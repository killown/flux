use crate::model::SortBy;
use crate::ui::FileItem;
use gtk::prelude::*;
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
