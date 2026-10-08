use super::super::mallinfo::MallinfoSnap;
use super::super::smaps::SmapsRegion;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub(crate) struct GridItemSnap {
    pub idx: u32,
    pub name: String,
    pub path: PathBuf,
    pub size: u64,
    pub mtime: i64,
    pub is_dir: bool,
    pub icon_size: i32,
    pub is_list_mode: bool,
    pub is_editing: bool,
    pub is_foreign: bool,
    pub is_custom_icon: bool,
    pub expand_labels: bool,
    pub has_texture: bool,
    pub max_width_chars: i32,
    pub grid_spacing: i32,
    pub rc_strong: usize,
    pub rc_weak: usize,
    pub active_path_ok: bool,
    pub active_path_val: Option<PathBuf>,
    pub icon_type: String,
}

#[derive(Debug, Clone)]
pub(crate) struct CacheSnap {
    pub path: PathBuf,
    pub item_count: usize,
    pub thumb_count: usize,
    pub media_task_count: usize,
    pub age_secs: u64,
    pub heap_approx: usize,
    pub head: Vec<String>,
    pub tail: Vec<String>,
    pub thumb_keys: Vec<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct CategorySummary {
    pub category: String,
    pub count: usize,
    pub size_kb: u64,
    pub rss_kb: u64,
    pub dirty_kb: u64,
}

#[derive(Debug, Clone)]
pub(crate) struct FontMap {
    pub name: String,
    pub bytes: u64,
    pub count: usize,
}

#[derive(Debug, Clone)]
pub(crate) struct AppStateSnap {
    pub current_path: PathBuf,
    pub is_loading: bool,
    pub is_list_mode: bool,
    pub show_hidden: bool,
    pub sort_by: String,
    pub sort_ascending: bool,
    pub filter: String,
    pub extension_filter: Option<Vec<String>>,
    pub current_icon_size: i32,
    pub current_list_icon_size: i32,
    pub load_id: u64,
    pub history_depth: usize,
    pub forward_stack_depth: usize,
    pub recent_stack: Vec<PathBuf>,
    pub pending_thumbnails: Vec<u32>,
    pub sidebar_count: usize,
    pub breadcrumb_count: usize,
    pub exclusive_list: Vec<PathBuf>,
    pub exclusive_index: Option<usize>,
    pub task_queue_len: usize,
    pub has_directory_monitor: bool,
    pub header_view: String,
    pub config_max_width_chars: i32,
    pub config_grid_spacing: i32,
    pub config_lazy_thumbnails: bool,
    pub config_show_thumbnails: bool,
    pub config_folders_first: bool,
    pub config_expand_labels: bool,
    pub folder_cache_capacity: usize,
    pub folder_icons_count: usize,
    pub file_icons_count: usize,
}

#[derive(Debug, Clone)]
pub struct DebugSnapshot {
    pub(crate) rss_kb: u64,
    pub(crate) vm_peak_kb: u64,
    pub(crate) vm_data_kb: u64,
    pub(crate) vm_stk_kb: u64,
    pub(crate) vm_swap_kb: u64,
    pub(crate) fd_count: usize,
    pub(crate) thread_count: u64,
    pub(crate) grid_len: usize,
    pub(crate) grid_items: Vec<GridItemSnap>,
    pub(crate) leaked_rcs: Vec<(u32, usize, String)>,
    pub(crate) active_path_mismatches: Vec<(u32, String, Option<PathBuf>)>,
    pub(crate) items_with_texture: usize,
    pub(crate) items_editing: usize,
    pub(crate) items_foreign: usize,
    pub(crate) items_custom_icon: usize,
    pub(crate) app: AppStateSnap,
    pub(crate) caches: Vec<CacheSnap>,
    pub(crate) categories: Vec<CategorySummary>,
    pub(crate) largest_anon_regions: Vec<SmapsRegion>,
    pub(crate) largest_file_regions: Vec<SmapsRegion>,
    pub(crate) mallinfo: MallinfoSnap,
    pub(crate) mimalloc_stats: String,
    pub(crate) font_maps: Vec<FontMap>,
    pub(crate) font_total_bytes: u64,
    pub(crate) pending_thumb_paths: Vec<u32>,
    pub(crate) hwga: Vec<(&'static str, crate::utils::hwga::FunctionStats)>,
}
