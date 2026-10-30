//! Folder preview: the interactive icon grid, its chunked fill and Ctrl+scroll
//! zoom. Lazy thumbnails live in `folder_thumbs`.

use super::content::FolderEntry;
use super::handle::PreviewPanelHandle;
use super::registry;
use crate::domain::model::{AppMsg, FluxApp, NavigationMsg, PreviewMsg};
use crate::ui::FileItem;
use adw::prelude::*;
use relm4::typed_view::grid::TypedGridView;
use relm4::AsyncComponentSender;
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Duration;

/// Default icon size of the folder grid cells.
pub(super) const FOLDER_GRID_ICON_SIZE: i32 = 96;
/// Side room the folder grid leaves for the nav arrows (8px margin + button width).
pub(super) const FOLDER_GRID_NAV_GUTTER: i32 = 88;
/// Smallest and largest icon size Ctrl+scroll can reach in the folder grid.
const FOLDER_GRID_MIN_ICON: i32 = 32;
const FOLDER_GRID_MAX_ICON: i32 = 256;
/// Time spent creating cells synchronously when a folder is shown (fills the viewport).
const FOLDER_FIRST_BUDGET: Duration = Duration::from_millis(16);
/// Time spent creating cells per idle callback afterwards, well under one frame.
const FOLDER_CHUNK_BUDGET: Duration = Duration::from_millis(6);

/// A folder grid fill in progress, see `fill_folder_grid`.
struct FolderFill {
    icon_size: i32,
    entries: Vec<FolderEntry>,
    cfg: crate::domain::model::Config,
    hook: Option<crate::ui::ThumbHook>,
    next: Cell<usize>,
}

/// Builds the interactive grid shown when the selected item is a directory.
pub(super) fn build_folder_grid(
    sender: &AsyncComponentSender<FluxApp>,
) -> Rc<RefCell<TypedGridView<FileItem, gtk::MultiSelection>>> {
    crate::hit!("build_folder_grid");
    let single_click = crate::utils::load_config().ui.single_click;
    let grid = Rc::new(RefCell::new(
        TypedGridView::<FileItem, gtk::MultiSelection>::new(),
    ));
    let view = grid.borrow().view.clone();
    view.set_enable_rubberband(true);
    view.set_single_click_activate(single_click);
    view.set_min_columns(1);
    view.set_max_columns(20);
    view.set_hexpand(true);
    view.set_vexpand(true);

    // Enter or click: folders open in the main grid, files open with the default app.
    // Files inside an archive go through `ActivateItems` so they are extracted first.
    {
        let weak = Rc::downgrade(&grid);
        let s = sender.clone();
        view.connect_activate(move |_, position| {
            let Some(grid) = weak.upgrade() else {
                return;
            };
            let target = grid.borrow().get(position).map(|w| {
                let item = w.borrow();
                (item.path.clone(), item.is_dir)
            });
            match target {
                Some((path, true)) => s.input(AppMsg::Navigation(NavigationMsg::Navigate(path))),
                Some((path, false))
                    if path
                        .to_string_lossy()
                        .starts_with(crate::infra::archive::ARCHIVE_URI) =>
                {
                    s.input(AppMsg::Navigation(NavigationMsg::ActivateItems(vec![(
                        path, false,
                    )])))
                }
                Some((path, false)) => FluxApp::open_file(path),
                None => {}
            }
        });
    }

    // Esc hands the keyboard back to the main grid.
    {
        let s = sender.clone();
        let keys = gtk::EventControllerKey::new();
        keys.connect_key_pressed(move |_, key, _, _| {
            if key == gtk::gdk::Key::Escape || key == gtk::gdk::Key::Tab {
                s.input(AppMsg::Preview(PreviewMsg::TogglePreviewGridFocus));
                return gtk::glib::Propagation::Stop;
            }
            glib::Propagation::Proceed
        });
        view.add_controller(keys);
    }

    // Read-only for now: the cell context menu acts on the main tab, so claim right-click here.
    let block_menu = gtk::GestureClick::new();
    block_menu.set_button(gtk::gdk::BUTTON_SECONDARY);
    block_menu.set_propagation_phase(gtk::PropagationPhase::Capture);
    block_menu.connect_pressed(|g, _, _, _| {
        g.set_state(gtk::EventSequenceState::Claimed);
    });
    view.add_controller(block_menu);

    grid
}

pub(super) fn attach_folder_zoom_scroll(sw: &gtk::ScrolledWindow) {
    let ctl = gtk::EventControllerScroll::new(gtk::EventControllerScrollFlags::VERTICAL);
    ctl.set_propagation_phase(gtk::PropagationPhase::Capture);
    ctl.connect_scroll(|c, _dx, dy| {
        if !c
            .current_event_state()
            .contains(gtk::gdk::ModifierType::CONTROL_MASK)
        {
            return glib::Propagation::Proceed;
        }
        registry::with(|h| h.zoom_folder_grid(dy < 0.0));
        glib::Propagation::Stop
    });
    sw.add_controller(ctl);
}

impl PreviewPanelHandle {
    /// Empties the folder grid and cancels its thumbnail loads, so nothing of a folder
    /// the preview has left stays in memory or keeps a worker busy.
    pub(crate) fn release_folder_grid(&self) {
        self.state.fill_gen.set(self.state.fill_gen.get() + 1);
        self.state.folder_thumbs.stop();
        let mut grid = self.folder_grid.borrow_mut();
        if !grid.is_empty() {
            grid.clear();
        }
    }

    /// Replaces the folder grid contents with `entries`.
    ///
    /// Building a cell costs an icon lookup, so thousands of them in one go would freeze
    /// the panel. The first chunk fills the viewport right away, the rest is added between
    /// frames and abandoned as soon as the preview moves on to something else.
    pub(super) fn fill_folder_grid(&self, entries: &[FolderEntry]) {
        crate::hit!("fill_folder_grid");
        let cfg = crate::utils::load_config();
        let st = &self.state;
        let fill_gen = st.fill_gen.get() + 1;
        st.fill_gen.set(fill_gen);
        st.folder_thumbs.stop();
        self.folder_grid.borrow_mut().clear();
        // Image thumbnails load lazily as cells bind, `None` when switched off.
        let hook = st.folder_thumbs.begin(&cfg);

        *st.folder_entries.borrow_mut() = entries.to_vec();
        let fill = Rc::new(FolderFill {
            icon_size: st.folder_icon_size.get(),
            entries: entries.to_vec(),
            cfg,
            hook,
            next: Cell::new(0),
        });
        self.append_folder_chunk(&fill, FOLDER_FIRST_BUDGET);
        if fill.next.get() >= fill.entries.len() {
            return;
        }

        let h = self.clone();
        gtk::glib::idle_add_local(move || {
            if h.state.fill_gen.get() != fill_gen {
                return gtk::glib::ControlFlow::Break;
            }
            h.append_folder_chunk(&fill, FOLDER_CHUNK_BUDGET);
            if fill.next.get() >= fill.entries.len() {
                gtk::glib::ControlFlow::Break
            } else {
                gtk::glib::ControlFlow::Continue
            }
        });
    }

    /// Appends entries of `fill` to the folder grid until `budget` is used up
    /// (at least one), so a chunk never holds the main loop for a whole frame.
    fn append_folder_chunk(&self, fill: &FolderFill, budget: Duration) {
        crate::hit!("append_folder_chunk");
        let began = std::time::Instant::now();
        let start = fill.next.get();
        let cfg = &fill.cfg;
        let mut grid = self.folder_grid.borrow_mut();
        let mut end = start;
        for (idx, e) in (start as u32..).zip(&fill.entries[start..]) {
            if end > start && began.elapsed() >= budget {
                break;
            }
            end += 1;
            let icon = crate::utils::icon::get_icon_for_path(&e.path, e.is_dir);
            let label = crate::utils::helpers::format_display_label(
                &e.name,
                e.is_dir,
                &cfg.ui.hidden_extensions,
            );
            grid.append(
                FileItem::builder(e.name.clone(), e.path.clone(), icon)
                    .display_label(label)
                    .is_dir(e.is_dir)
                    .size(e.size)
                    .mtime(e.mtime)
                    .icon_size(fill.icon_size)
                    .grid_idx(idx)
                    .max_width_chars(cfg.ui.max_width_chars)
                    .grid_spacing(cfg.ui.grid_spacing)
                    .show_symlink_emblem(cfg.ui.show_symlink_emblem)
                    .scale_font_with_icons(cfg.ui.scale_font_with_icons)
                    .default_icon_size(cfg.ui.default_icon_size)
                    .disable_drag_and_drop(true)
                    .thumb_hook(fill.hook.clone())
                    .build(),
            );
        }
        fill.next.set(end);
    }

    /// Steps the folder grid icon size up or down and refills the grid, keeping the scroll position.
    fn zoom_folder_grid(&self, bigger: bool) {
        let st = &self.state;
        let old = st.folder_icon_size.get();
        let sizes: Vec<i32> = crate::ui::constants::CRISP_ICON_SIZES
            .iter()
            .copied()
            .filter(|s| (FOLDER_GRID_MIN_ICON..=FOLDER_GRID_MAX_ICON).contains(s))
            .collect();
        let idx = sizes
            .iter()
            .position(|&s| s >= old)
            .unwrap_or(sizes.len() - 1);
        let new = if bigger {
            sizes[(idx + 1).min(sizes.len() - 1)]
        } else {
            sizes[idx.saturating_sub(1)]
        };
        if new == old || st.folder_entries.borrow().is_empty() {
            return;
        }
        st.folder_icon_size.set(new);

        let scroller = self
            .stack
            .child_by_name("folder")
            .and_then(|w| w.downcast::<gtk::ScrolledWindow>().ok());
        let adj = scroller.map(|sw| sw.vadjustment());
        let scaled = adj
            .as_ref()
            .map(|a| a.value() * f64::from(new) / f64::from(old));

        let entries = st.folder_entries.borrow().clone();
        self.fill_folder_grid(&entries);

        if let (Some(adj), Some(value)) = (adj, scaled) {
            gtk::glib::timeout_add_local_once(Duration::from_millis(60), move || {
                adj.set_value(value.min((adj.upper() - adj.page_size()).max(0.0)));
            });
        }
    }

    /// Focuses the folder grid and selects its first item when nothing is selected.
    pub fn focus_folder_grid(&self) -> bool {
        if self.stack.visible_child_name().as_deref() != Some("folder") {
            return false;
        }
        let view = self.folder_grid.borrow().view.clone();
        if let Some(model) = view
            .model()
            .and_then(|m| m.downcast::<gtk::MultiSelection>().ok())
        {
            if model.n_items() > 0 && model.selection().is_empty() {
                model.select_item(0, true);
            }
        }
        view.grab_focus();
        view.has_focus()
    }

    /// True when keyboard focus is inside the folder grid.
    pub fn folder_grid_has_focus(&self) -> bool {
        let view = self.folder_grid.borrow().view.clone();
        view.root()
            .and_then(|r| r.focus())
            .is_some_and(|f| f == *view.upcast_ref::<gtk::Widget>() || f.is_ancestor(&view))
    }
}
