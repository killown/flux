//! Lazy thumbnails for the folder grid in the peek-preview panel.
//!
//! The grid cells tell us when they get `Bound` or `Unbound` through a [`ThumbHook`]
//! (see `ui/components/bind.rs`). That means only the cells actually visible on screen
//! ever ask for a thumbnail, we never walk the whole folder.
//!
//! * **Debounce**: we collect binds and flush them 150 ms after the first one, so if
//!   you're arrowing quickly through folders the grid gets replaced before any load
//!   even starts.
//! * **Cancellation**: each request gets its own cancel flag (set when the cell
//!   scrolls away) plus a generation token (bumped whenever the grid is refilled or
//!   released). Stale workers just drop their result and never touch the UI.
//! * **Images and videos**: video frames come from ffmpeg, for plain folders and
//!   `archive://` entries alike. No audio, PDF or font thumbnails here.
//! * **Disk cache**: loads go through `utils::media::get_or_create_thumbnail`, the same
//!   XDG cache the main grid uses, so revisiting a folder is a cache hit.
//! * **Switch**: `ui.folder_grid_thumbnails` (on by default), also gated by the global
//!   `show_thumbnails` and `thumbnail_types.images` / `thumbnail_types.videos`.

use crate::model::Config;
use crate::ui::{FileItem, ThumbEvent, ThumbHook};
use adw::prelude::*;
use futures::channel::mpsc::{unbounded, UnboundedSender};
use futures::StreamExt;
use relm4::typed_view::grid::TypedGridView;
use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::rc::{Rc, Weak};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Semaphore;

/// How long to wait after the first bind before we actually start loading.
const DEBOUNCE: Duration = Duration::from_millis(150);
/// Cap on concurrent loads, this is a side panel, not the main view.
const MAX_PARALLEL: usize = 4;

type FolderGrid = Rc<RefCell<TypedGridView<FileItem, gtk::MultiSelection>>>;

/// A finished thumbnail on its way from a worker back to the GTK thread.
struct Done {
    generation: u64,
    idx: u32,
    path: PathBuf,
    texture: gtk::gdk::Texture,
}

struct Inner {
    grid: Weak<RefCell<TypedGridView<FileItem, gtk::MultiSelection>>>,
    /// False while the grid is empty/released or the feature is switched off.
    enabled: Cell<bool>,
    /// Whether image cells get thumbnails (`thumbnail_types.images`).
    images: Cell<bool>,
    /// Whether video cells get thumbnails (`thumbnail_types.videos`).
    videos: Cell<bool>,
    /// Bumped on every refill and release, workers check against it.
    generation: Arc<AtomicU64>,
    /// Bound cells we haven't handed to a worker yet.
    waiting: RefCell<HashMap<u32, PathBuf>>,
    /// Indices already requested in this generation (in flight or done), so a
    /// recycled cell re-binding the same item doesn't kick off a second load.
    requested: RefCell<HashSet<u32>>,
    /// Cancel flags for the loads currently in flight.
    tokens: RefCell<HashMap<u32, Arc<AtomicBool>>>,
    /// The image widget of the cell currently showing each index, lets us paint
    /// results in place.
    cells: RefCell<HashMap<u32, gtk::glib::WeakRef<gtk::Image>>>,
    debounce_armed: Cell<bool>,
    permits: RefCell<Arc<Semaphore>>,
    tx: UnboundedSender<Done>,
}

/// Owns the lazy thumbnail loading for the folder grid. Cheap to clone and share
/// through the panel state.
pub(crate) struct FolderThumbs(Rc<Inner>);

impl FolderThumbs {
    pub(crate) fn new(grid: &FolderGrid) -> Self {
        let (tx, mut rx) = unbounded::<Done>();
        let inner = Rc::new(Inner {
            grid: Rc::downgrade(grid),
            enabled: Cell::new(false),
            images: Cell::new(false),
            videos: Cell::new(false),
            generation: Arc::new(AtomicU64::new(0)),
            waiting: RefCell::new(HashMap::new()),
            requested: RefCell::new(HashSet::new()),
            tokens: RefCell::new(HashMap::new()),
            cells: RefCell::new(HashMap::new()),
            debounce_armed: Cell::new(false),
            permits: RefCell::new(Arc::new(Semaphore::new(1))),
            tx,
        });

        let weak = Rc::downgrade(&inner);
        gtk::glib::spawn_future_local(async move {
            while let Some(done) = rx.next().await {
                match weak.upgrade() {
                    Some(inner) => inner.apply(done),
                    None => break,
                }
            }
        });

        Self(inner)
    }

    /// Drops every pending and in-flight load. Leaves the grid contents alone.
    pub(crate) fn stop(&self) {
        self.0.cancel_all();
    }

    /// Starts a new generation for a freshly filled grid and returns the hook the
    /// new items should carry, or `None` if thumbnails are off for this grid.
    pub(crate) fn begin(&self, cfg: &Config) -> Option<ThumbHook> {
        self.0.cancel_all();

        let types = &cfg.ui.thumbnail_types;
        let on = cfg.ui.folder_grid_thumbnails
            && cfg.ui.show_thumbnails
            && (types.images || types.videos);
        if !on {
            return None;
        }

        self.0.images.set(types.images);
        self.0.videos.set(types.videos);
        self.0.enabled.set(true);
        *self.0.permits.borrow_mut() = Arc::new(Semaphore::new(
            cfg.ui.thumbnail_threads.clamp(1, MAX_PARALLEL),
        ));

        let weak = Rc::downgrade(&self.0);
        Some(ThumbHook(Rc::new(move |ev| {
            if let Some(inner) = weak.upgrade() {
                inner.on_event(&inner, ev);
            }
        })))
    }
}

/// Images and videos, each gated by its own `thumbnail_types` switch.
fn wants_thumbnail(path: &Path, images: bool, videos: bool) -> bool {
    let (is_img, is_vid) = crate::utils::media::is_visual_media(path);
    (is_img && images) || (is_vid && videos)
}

impl Inner {
    fn cancel_all(&self) {
        self.enabled.set(false);
        self.generation.fetch_add(1, Ordering::AcqRel);
        for (_, flag) in self.tokens.borrow_mut().drain() {
            flag.store(true, Ordering::Release);
        }
        self.waiting.borrow_mut().clear();
        self.requested.borrow_mut().clear();
        self.cells.borrow_mut().clear();
        self.debounce_armed.set(false);
    }

    fn on_event(&self, this: &Rc<Inner>, ev: ThumbEvent) {
        if !self.enabled.get() {
            return;
        }
        match ev {
            ThumbEvent::Bound { idx, path, icon } => {
                if self.requested.borrow().contains(&idx) {
                    return;
                }
                if !wants_thumbnail(&path, self.images.get(), self.videos.get()) {
                    // Remember the verdict so recycled cells skip the check.
                    self.requested.borrow_mut().insert(idx);
                    return;
                }
                self.cells.borrow_mut().insert(idx, icon.downgrade());
                self.waiting.borrow_mut().insert(idx, path);
                self.arm(this);
            }
            ThumbEvent::Unbound { idx } => {
                self.waiting.borrow_mut().remove(&idx);
                self.cells.borrow_mut().remove(&idx);
                // Scrolled away before the result arrived: cancel it, and let a
                // later bind of the same item ask again.
                if let Some(flag) = self.tokens.borrow_mut().remove(&idx) {
                    flag.store(true, Ordering::Release);
                    self.requested.borrow_mut().remove(&idx);
                }
            }
        }
    }

    fn arm(&self, this: &Rc<Inner>) {
        if self.debounce_armed.replace(true) {
            return;
        }
        let weak = Rc::downgrade(this);
        let generation = self.generation.load(Ordering::Acquire);
        gtk::glib::timeout_add_local_once(DEBOUNCE, move || {
            let Some(inner) = weak.upgrade() else {
                return;
            };
            // The grid was refilled or released in the meantime, the new
            // generation will arm itself.
            if inner.generation.load(Ordering::Acquire) != generation {
                return;
            }
            inner.debounce_armed.set(false);
            inner.flush(generation);
        });
    }

    /// Hands every still-bound cell to a worker, top-left first.
    fn flush(&self, generation: u64) {
        let mut batch: Vec<(u32, PathBuf)> = self.waiting.borrow_mut().drain().collect();
        batch.sort_by_key(|(idx, _)| *idx);

        for (idx, path) in batch {
            if !self.requested.borrow_mut().insert(idx) {
                continue;
            }
            let cancel = Arc::new(AtomicBool::new(false));
            self.tokens.borrow_mut().insert(idx, cancel.clone());

            let current = self.generation.clone();
            let permits = self.permits.borrow().clone();
            let tx = self.tx.clone();

            relm4::spawn(async move {
                let stale = || {
                    cancel.load(Ordering::Acquire) || current.load(Ordering::Acquire) != generation
                };
                if stale() {
                    return;
                }
                let Ok(_permit) = permits.acquire().await else {
                    return;
                };
                if stale() {
                    return;
                }
                let texture = crate::utils::media::get_or_create_thumbnail(&path).await;
                if stale() {
                    return;
                }
                if let Some(texture) = texture {
                    let _ = tx.unbounded_send(Done {
                        generation,
                        idx,
                        path,
                        texture,
                    });
                }
            });
        }
    }

    /// GTK thread: stashes the texture on the item and paints it into the cell that
    /// shows it. We update the cell in place, replacing the item (`remove` + `insert`)
    /// would recreate the widget and restart its CSS hover transition on every result.
    fn apply(&self, done: Done) {
        if done.generation != self.generation.load(Ordering::Acquire) {
            return;
        }
        self.tokens.borrow_mut().remove(&done.idx);

        let Some(grid_rc) = self.grid.upgrade() else {
            return;
        };
        let Ok(grid) = grid_rc.try_borrow() else {
            return;
        };
        let Some(wrapper) = grid.get(done.idx) else {
            return;
        };

        let icon_size = {
            let mut item = wrapper.borrow_mut();
            // The slot may hold a different item by now.
            if item.path != done.path || item.thumbnail.is_some() {
                return;
            }
            item.thumbnail = Some(done.texture.clone());
            item.icon_size
        };

        // Not bound any more, the texture lives on the item and will show up
        // on the next bind.
        let cell = self
            .cells
            .borrow()
            .get(&done.idx)
            .and_then(|weak| weak.upgrade());
        if let Some(image) = cell {
            image.set_pixel_size(icon_size);
            image.set_paintable(Some(&done.texture));
        }
    }
}
