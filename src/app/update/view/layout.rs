use crate::app::state::FluxApp;
use crate::domain::model::constants;
use crate::domain::model::{AppMsg, NavigationMsg, SortBy, ViewMsg};
use crate::ui::FileItem;
use crate::utils;
use gtk::prelude::{Cast, ObjectExt, WidgetExt};
use relm4::prelude::*;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::Duration;

/// Quiet period after the last zoom step before the new size is written to disk.
const ZOOM_PERSIST_DELAY: Duration = Duration::from_millis(400);

/// True while a `ViewMsg::ZoomApply` timer is pending.
static ZOOM_APPLY_QUEUED: AtomicBool = AtomicBool::new(false);

/// Bumped on every zoom step. A persist timer only fires if no newer step arrived.
static ZOOM_PERSIST_GEN: AtomicU64 = AtomicU64::new(0);

/// Queues one grid update for after the next paint and one debounced save.
///
/// Called on the GTK thread for every zoom step. The grid update is an idle
/// source below the frame clock's redraw priority, so it never runs ahead of a
/// pending paint: a burst of scroll events costs at most one update per frame,
/// and a slow update delays the next one instead of starving the paint. The
/// sources send messages through `infra::SENDER`, so the work runs in
/// `handle_zoom_apply` and `handle_zoom_persist` and not inside the scroll
/// controller.
fn schedule_zoom_work() {
    if !ZOOM_APPLY_QUEUED.swap(true, Ordering::Relaxed) {
        gtk::glib::idle_add_local_full(gtk::glib::Priority::DEFAULT_IDLE, || {
            ZOOM_APPLY_QUEUED.store(false, Ordering::Relaxed);
            if let Some(sender) = crate::infra::SENDER.get() {
                let _ = sender.send(AppMsg::View(ViewMsg::ZoomApply));
            }
            gtk::glib::ControlFlow::Break
        });
    }

    let generation = ZOOM_PERSIST_GEN.fetch_add(1, Ordering::Relaxed) + 1;
    gtk::glib::timeout_add_local_once(ZOOM_PERSIST_DELAY, move || {
        if ZOOM_PERSIST_GEN.load(Ordering::Relaxed) == generation {
            if let Some(sender) = crate::infra::SENDER.get() {
                let _ = sender.send(AppMsg::View(ViewMsg::ZoomPersist));
            }
        }
    });
}

/// A live grid cell: its root widget and the model position `bind` published.
struct BoundCell {
    root: gtk::Overlay,
    /// Only a hint, the model can have been sorted or filtered since the item was built.
    index: Option<u32>,
}

/// The live cells of `view`, keyed by the path each one shows.
///
/// Reads the `flux_path` and `grid_item_index` data that `bind` stores on each
/// cell root, so it is a walk over the handful of live cells and never touches
/// the model.
fn bound_cells(view: &gtk::GridView) -> HashMap<PathBuf, BoundCell> {
    let mut cells = HashMap::new();
    let mut child = view.first_child();
    while let Some(cell) = child {
        let root = cell.first_child().unwrap_or_else(|| cell.clone());
        let path = unsafe {
            root.data::<PathBuf>("flux_path")
                .map(|p| p.as_ref().clone())
        };
        let index = unsafe { root.data::<u32>("grid_item_index").map(|i| *i.as_ref()) };
        if let (Some(path), Ok(root)) = (path, root.downcast::<gtk::Overlay>()) {
            cells.insert(path, BoundCell { root, index });
        }
        child = cell.next_sibling();
    }
    cells
}

impl FluxApp {
    /// Toggles between grid card layout and compact list view.
    pub fn handle_toggle_list_mode(&mut self) {
        crate::hit!("handle_toggle_list_mode");
        // The shortcut can fire from any focus (sidebar, panel, terminal).
        // Pull focus back to the main grid first, then switch. Skipped while
        // the full-window terminal is up, since the grid is hidden then.
        if !self.term.fullscreen {
            if let Some(tab) = self.tabs.get(self.active_tab_index) {
                tab.files.view.grab_focus();
            }
        }

        self.is_list_mode = !self.is_list_mode;
        self.search.saved_list_mode = self.is_list_mode;
        self.config.default_list_mode = self.is_list_mode;
        utils::save_config(&self.config);

        let is_list = self.is_list_mode;
        let new_icon_size = if is_list {
            self.current_list_icon_size
        } else {
            self.current_icon_size
        };

        let active_tab = match self.tabs.get_mut(self.active_tab_index) {
            Some(t) => t,
            None => return,
        };

        let max_columns = if is_list { 1 } else { 20 };
        active_tab.files.view.set_min_columns(1);
        active_tab.files.view.set_max_columns(max_columns);
        self.search.saved_max_columns = max_columns;

        for i in 0..active_tab.files.len() {
            if let Some(item_wrapper) = active_tab.files.get(i) {
                let mut item = item_wrapper.borrow().clone();
                item.is_list_mode = is_list;
                item.icon_size = new_icon_size;
                active_tab.files.remove(i);
                active_tab.files.insert(i, item);
            }
        }

        self.sync_list_mode();
        self.refresh_wide_preview_layout();
    }

    /// Toggles between ascending and descending sort order.
    pub fn handle_toggle_sort_order(&mut self, sender: &AsyncComponentSender<Self>) {
        crate::hit!("handle_toggle_sort_order");
        self.sort_ascending = !self.sort_ascending;
        let _ = self.state_db.save_view(
            &self.current_path,
            &format!("{:?}", self.sort_by),
            !self.sort_ascending,
            self.current_icon_size as u32,
            self.config.ui.folders_first,
        );
        sender.input(AppMsg::Navigation(NavigationMsg::Refresh));
    }

    /// Cycles through available sorting criteria (Name -> Date -> Size -> Type).
    pub fn handle_cycle_sort(&mut self, sender: &AsyncComponentSender<Self>) {
        crate::hit!("handle_cycle_sort");
        self.sort_by = match self.sort_by {
            SortBy::Name => SortBy::Date,
            SortBy::Date => SortBy::Size,
            SortBy::Size => SortBy::Type,
            SortBy::Type => SortBy::Name,
        };
        let _ = self.state_db.save_view(
            &self.current_path,
            &format!("{:?}", self.sort_by),
            !self.sort_ascending,
            self.current_icon_size as u32,
            self.config.ui.folders_first,
        );
        sender.input(AppMsg::Navigation(NavigationMsg::Refresh));
    }

    /// Toggles "Folders First" sorting priority for the current directory view.
    pub fn handle_cycle_folder_priority(&mut self, sender: &AsyncComponentSender<Self>) {
        crate::hit!("handle_cycle_folder_priority");
        let path = self.current_path.clone();
        let current_state = if let Ok(Some((_, _, _, ff))) = self.state_db.get_view(&path) {
            ff
        } else {
            self.config.ui.folders_first
        };
        let new_state = !current_state;

        let _ = self.state_db.save_view(
            &path,
            &format!("{:?}", self.sort_by),
            false,
            self.current_icon_size as u32,
            new_state,
        );
        self.load_path(path, sender);
    }

    /// Adjusts icon scale on scroll, targeting only the active view mode.
    ///
    /// In grid mode `current_icon_size` is updated. In list mode `current_list_icon_size`
    /// and `config.ui.list_icon_size` are updated instead. Only the in-memory size
    /// changes here. The live cells are resized once per frame by
    /// [`Self::handle_zoom_apply`] and the size is saved after scrolling pauses by
    /// [`Self::handle_zoom_persist`], so a burst of scroll events does no per-event
    /// model change, SQLite write or `config.toml` write.
    pub fn handle_zoom(&mut self, delta: f64) {
        crate::hit!("handle_zoom");
        if delta == 0.0 {
            return;
        }

        let direction = if delta > 0.0 { -1 } else { 1 };

        let current_size = if self.is_list_mode {
            self.current_list_icon_size
        } else {
            self.current_icon_size
        };
        let current_idx = constants::CRISP_ICON_SIZES
            .iter()
            .position(|&s| s >= current_size)
            .unwrap_or(constants::CRISP_ICON_SIZES.len() - 1);

        let new_idx = if direction > 0 {
            (current_idx + 1).min(constants::CRISP_ICON_SIZES.len() - 1)
        } else {
            current_idx.saturating_sub(1)
        };

        let new_size = constants::CRISP_ICON_SIZES[new_idx];
        if new_size == current_size {
            return;
        }

        if self.is_list_mode {
            self.current_list_icon_size = new_size;
            self.config.ui.list_icon_size = new_size;
        } else {
            self.current_icon_size = new_size;
            // Cells realized before the burst settles must bind at the new size.
            FileItem::set_live_grid_icon_size(Some(new_size));
        }

        schedule_zoom_work();
    }

    /// Applies the current icon size to the live cells of the active tab.
    ///
    /// Grid cells are resized in place ([`FileItem::resize_in_place`]): no unbind, no
    /// model change and no new cell, so the selection and the thumbnails stay as
    /// they are and the cost follows the live cell count, not the folder size. Only
    /// a cell that cannot be resized that way is replaced in the model, which is
    /// what rebinds it. The items without a live cell keep their old size until
    /// [`Self::handle_zoom_persist`] syncs them, and bind covers the cells that
    /// GTK realizes in between.
    pub fn handle_zoom_apply(&mut self) {
        crate::hit!("handle_zoom_apply");
        self.sync_icon_sizes(false);
    }

    /// Sets the icon size of the active tab's items to the current size.
    ///
    /// With `full` set it also updates every item without a live cell, which is a
    /// walk over the whole model and so only runs once a burst has settled.
    fn sync_icon_sizes(&mut self, full: bool) {
        let is_list = self.is_list_mode;
        let size = if is_list {
            self.current_list_icon_size
        } else {
            self.current_icon_size
        };

        let Some(tab) = self.tabs.get_mut(self.active_tab_index) else {
            return;
        };

        let cells = bound_cells(&tab.files.view);
        let mut unresolved: HashSet<PathBuf> = HashSet::new();
        for (path, cell) in &cells {
            // The published index is a hint: the item found there must be this cell's.
            let resolved = cell
                .index
                .and_then(|index| tab.files.get(index))
                .is_some_and(|wrapper| {
                    let mut item = wrapper.borrow_mut();
                    if item.path != *path || item.is_list_mode != is_list {
                        return false;
                    }
                    if item.icon_size == size {
                        return true;
                    }
                    let previous = item.icon_size;
                    item.icon_size = size;
                    if item.resize_in_place(&cell.root) {
                        true
                    } else {
                        item.icon_size = previous;
                        false
                    }
                });
            if !resolved {
                unresolved.insert(path.clone());
            }
        }

        // No live cell found means the walk failed, so replace everything.
        let replace_all = cells.is_empty();
        if !(full || replace_all || !unresolved.is_empty()) {
            return;
        }
        for i in 0..tab.files.len() {
            let stale = tab.files.get(i).and_then(|wrapper| {
                let mut item = wrapper.borrow_mut();
                if item.is_list_mode != is_list || item.icon_size == size {
                    return None;
                }
                if replace_all || unresolved.contains(&item.path) {
                    Some(item.clone())
                } else {
                    item.icon_size = size;
                    None
                }
            });
            if let Some(mut item) = stale {
                item.icon_size = size;
                tab.files.remove(i);
                tab.files.insert(i, item);
            }
        }
    }

    /// Finishes a zoom burst once scrolling has paused: syncs every item to the
    /// final size, then saves it.
    ///
    /// Grid mode stores the size for the current folder in the state database,
    /// list mode stores `config.ui.list_icon_size` in `config.toml`. Both run on the
    /// blocking pool.
    pub fn handle_zoom_persist(&mut self) {
        crate::hit!("handle_zoom_persist");
        self.sync_icon_sizes(true);
        FileItem::set_live_grid_icon_size(None);
        if self.is_list_mode {
            let config = self.config.clone();
            relm4::spawn_blocking(move || utils::save_config(&config));
        } else {
            let state_db = self.state_db.clone();
            let path = self.current_path.clone();
            let sort = format!("{:?}", self.sort_by);
            let size = self.current_icon_size as u32;
            let folders_first = self.config.ui.folders_first;
            relm4::spawn_blocking(move || {
                let _ = state_db.save_view(&path, &sort, false, size, folders_first);
            });
        }
    }
}
