use crate::model::{AppMsg, FluxApp, SortBy};
use crate::ui::FileItem;
use gtk::prelude::*;
use relm4::prelude::*;
use relm4::typed_view::grid::TypedGridView;
use std::collections::{HashSet, VecDeque};
use std::path::PathBuf;
use std::sync::atomic::AtomicU64;
use std::sync::Arc;

/// Isolated view, model, and navigation state for an individual tab.
#[derive(Debug)]
pub struct TabState {
    #[allow(dead_code)]
    pub id: u64,
    pub title: String,
    pub current_path: PathBuf,
    pub history: Vec<PathBuf>,
    pub forward_stack: Vec<PathBuf>,
    pub recent_stack: VecDeque<PathBuf>,
    pub is_list_mode: bool,
    pub sort_by: SortBy,
    pub sort_ascending: bool,
    pub filter: String,
    pub selection_status: String,
    pub is_initialized: bool,
    pub scroll_offset: f64,
    pub files: TypedGridView<FileItem, gtk::MultiSelection>,
    pub scroller: gtk::ScrolledWindow,
    #[allow(dead_code)]
    pub load_id: Arc<AtomicU64>,
    pub pending_thumbnails: HashSet<u32>,
    pub last_thumb_scroll_idx: usize,
    pub thumbs_ready: bool,
}

impl TabState {
    pub fn new(id: u64, initial_path: PathBuf, is_list_mode: bool, sort_by: SortBy) -> Self {
        let title = initial_path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| initial_path.to_string_lossy().to_string());

        let files = TypedGridView::<FileItem, gtk::MultiSelection>::new();
        files.view.set_enable_rubberband(true);
        if is_list_mode {
            files.view.set_min_columns(1);
            files.view.set_max_columns(1);
        } else {
            files.view.set_min_columns(1);
            files.view.set_max_columns(20);
        }
        files.view.set_halign(gtk::Align::Fill);
        files.view.set_hexpand(true);

        let scroller = gtk::ScrolledWindow::builder()
            .vexpand(true)
            .hexpand(true)
            .halign(gtk::Align::Fill)
            .hscrollbar_policy(gtk::PolicyType::Never)
            .vscrollbar_policy(gtk::PolicyType::Automatic)
            .propagate_natural_width(false)
            .child(&files.view)
            .build();

        Self {
            id,
            title,
            current_path: initial_path,
            history: Vec::new(),
            forward_stack: Vec::new(),
            recent_stack: VecDeque::with_capacity(30),
            is_list_mode,
            sort_by,
            sort_ascending: true,
            filter: String::new(),
            selection_status: String::new(),
            is_initialized: false,
            scroll_offset: 0.0,
            files,
            scroller,
            load_id: Arc::new(AtomicU64::new(1)),
            pending_thumbnails: HashSet::new(),
            last_thumb_scroll_idx: 0,
            thumbs_ready: false,
        }
    }
}

impl FluxApp {
    pub fn handle_new_tab(
        &mut self,
        target_path: Option<PathBuf>,
        sender: &AsyncComponentSender<Self>,
    ) {
        let path = target_path.unwrap_or_else(|| self.current_path.clone());
        self.next_tab_id += 1;
        let tab_id = self.next_tab_id;

        let mut new_tab = TabState::new(
            tab_id,
            path.clone(),
            self.is_list_mode,
            self.config.ui.default_sort,
        );

        // Do not mark initialized yet, let load_path or SwitchTab do it
        new_tab.is_initialized = true;

        self.wire_tab_signals(&mut new_tab, sender);

        let title = new_tab.title.clone();
        let page = self.tab_view.append(&new_tab.scroller);
        page.set_title(&title);

        self.tabs.push(new_tab);
        self.active_tab_index = self.tabs.len() - 1;

        if self.tab_view.selected_page().as_ref() != Some(&page) {
            self.tab_view.set_selected_page(&page);
        }

        self.load_path(path, sender);
    }

    pub fn handle_open_tabs(&mut self, targets: Vec<PathBuf>, sender: &AsyncComponentSender<Self>) {
        if targets.is_empty() {
            return;
        }

        let total = targets.len();
        for (i, path) in targets.into_iter().enumerate() {
            let is_last = i == total - 1;
            self.next_tab_id += 1;
            let tab_id = self.next_tab_id;

            let mut new_tab = TabState::new(
                tab_id,
                path.clone(),
                self.is_list_mode,
                self.config.ui.default_sort,
            );

            // Background tabs stay uninitialized until selected
            new_tab.is_initialized = is_last;

            self.wire_tab_signals(&mut new_tab, sender);

            let title = new_tab.title.clone();
            let page = self.tab_view.append(&new_tab.scroller);
            page.set_title(&title);

            self.tabs.push(new_tab);

            // Only activate and load the last tab
            if is_last {
                self.active_tab_index = self.tabs.len() - 1;
                self.tab_view.set_selected_page(&page);
                self.load_path(path, sender);
            }
        }
    }

    pub fn handle_switch_tab(&mut self, index: usize, sender: &AsyncComponentSender<Self>) {
        if index >= self.tabs.len() || index == self.active_tab_index {
            return;
        }

        let prev_idx = self.active_tab_index;
        if let Some(prev_tab) = self.tabs.get_mut(prev_idx) {
            prev_tab.current_path = self.current_path.clone();
            prev_tab.history = self.history.clone();
            prev_tab.forward_stack = self.forward_stack.clone();
            prev_tab.recent_stack = self.recent_stack.clone();
            prev_tab.filter = self.filter.clone();
            prev_tab.sort_by = self.sort_by;
            prev_tab.sort_ascending = self.sort_ascending;
            prev_tab.is_list_mode = self.is_list_mode;
            prev_tab.selection_status = self.selection_status.clone();
            prev_tab.scroll_offset = prev_tab.scroller.vadjustment().value();
        }

        self.active_tab_index = index;

        let (target_path, needs_initial_load, thumbs_ready) = {
            let tab = &mut self.tabs[index];
            let needs_load = !tab.is_initialized;
            tab.is_initialized = true;

            self.current_path = tab.current_path.clone();
            self.history = tab.history.clone();
            self.forward_stack = tab.forward_stack.clone();
            self.recent_stack = tab.recent_stack.clone();
            self.filter = tab.filter.clone();
            self.sort_by = tab.sort_by;
            self.sort_ascending = tab.sort_ascending;
            self.is_list_mode = tab.is_list_mode;
            self.selection_status = tab.selection_status.clone();

            (tab.current_path.clone(), needs_load, tab.thumbs_ready)
        };

        // Only update the selected page if it's not already active to avoid recursive notification cycles
        if (index as i32) < self.tab_view.n_pages() {
            let page = self.tab_view.nth_page(index as i32);
            if self.tab_view.selected_page().as_ref() != Some(&page) {
                self.tab_view.set_selected_page(&page);
            }
        }

        self.update_breadcrumbs();
        self.sync_sidebar_selection();

        // If the tab was interrupted or hasn't finished its thumbnails, kick it back into gear!
        if !thumbs_ready {
            let sender_clone = sender.clone();
            glib::timeout_add_local_once(std::time::Duration::from_millis(50), move || {
                sender_clone.input(AppMsg::CheckVisibleThumbnails);
            });
        }

        if let Some(entry) = self.header_path_entry.upgrade() {
            entry.set_text(&self.current_path.to_string_lossy());
            entry.set_position(entry.text_length() as i32);
        }

        if needs_initial_load {
            self.load_path(target_path, sender);
        } else {
            // Immediately evaluate visible items and trigger pending thumbnail tasks for the active tab
            self.check_visible_thumbnails(sender);
        }
    }

    pub fn handle_close_tab(
        &mut self,
        index_opt: Option<usize>,
        sender: &AsyncComponentSender<Self>,
    ) {
        match index_opt {
            None => {
                if self.tabs.len() <= 1 {
                    let app = gtk::Application::default();
                    if let Some(win) = app.active_window() {
                        win.close();
                    }
                    return;
                }

                if (self.active_tab_index as i32) < self.tab_view.n_pages() {
                    let page = self.tab_view.nth_page(self.active_tab_index as i32);
                    self.tab_view.close_page(&page);
                }
            }

            Some(idx) => {
                if idx < self.tabs.len() {
                    self.tabs.remove(idx);
                }

                let total_tabs = self.tabs.len();
                if total_tabs == 0 {
                    return;
                }

                // AdwTabView already adjusted its selected page.
                // Sync active_tab_index to AdwTabView's actual selected position.
                let current_page_pos = self
                    .tab_view
                    .selected_page()
                    .map(|p| self.tab_view.page_position(&p) as usize)
                    .unwrap_or(0);

                self.active_tab_index = current_page_pos.min(total_tabs.saturating_sub(1));

                let (target_path, needs_initial_load) = {
                    let tab = &mut self.tabs[self.active_tab_index];
                    let needs_load = !tab.is_initialized;
                    tab.is_initialized = true;

                    self.current_path = tab.current_path.clone();
                    self.history = tab.history.clone();
                    self.forward_stack = tab.forward_stack.clone();
                    self.recent_stack = tab.recent_stack.clone();
                    self.filter = tab.filter.clone();
                    self.sort_by = tab.sort_by;
                    self.sort_ascending = tab.sort_ascending;
                    self.is_list_mode = tab.is_list_mode;
                    self.selection_status = tab.selection_status.clone();

                    (tab.current_path.clone(), needs_load)
                };

                self.update_breadcrumbs();
                self.sync_sidebar_selection();

                if let Some(entry) = self.header_path_entry.upgrade() {
                    entry.set_text(&self.current_path.to_string_lossy());
                    entry.set_position(entry.text_length() as i32);
                }

                if needs_initial_load {
                    self.load_path(target_path, sender);
                }
            }
        }
    }

    pub fn handle_next_tab(&mut self) {
        if self.tabs.len() > 1 {
            let next = (self.active_tab_index + 1) % self.tabs.len();
            if (next as i32) < self.tab_view.n_pages() {
                let page = self.tab_view.nth_page(next as i32);
                self.tab_view.set_selected_page(&page);
            }
        }
    }

    pub fn handle_prev_tab(&mut self) {
        if self.tabs.len() > 1 {
            let prev = if self.active_tab_index == 0 {
                self.tabs.len() - 1
            } else {
                self.active_tab_index - 1
            };
            if (prev as i32) < self.tab_view.n_pages() {
                let page = self.tab_view.nth_page(prev as i32);
                self.tab_view.set_selected_page(&page);
            }
        }
    }

    fn wire_tab_signals(&self, tab: &mut TabState, sender: &AsyncComponentSender<Self>) {
        tab.files
            .view
            .set_single_click_activate(self.config.ui.single_click);

        let s_open = sender.clone();
        tab.files.view.connect_activate(move |_, pos| {
            s_open.input(AppMsg::Open(Some(pos)));
        });

        let s_sel = sender.clone();
        if let Some(selection_model) = tab.files.view.model().and_downcast::<gtk::MultiSelection>()
        {
            selection_model.connect_selection_changed(move |_, _, _| {
                s_sel.input(AppMsg::SelectionChanged);
            });
        }

        Self::connect_tab_scroller(&tab.scroller, sender);
    }
}
