#![allow(dead_code)]

use crate::model::FluxApp;
use adw::prelude::*;
use std::fmt::Write as FmtWrite;
use std::fs;
use std::path::PathBuf;
use std::rc::Rc;

fn page_size() -> u64 {
    unsafe { libc::sysconf(libc::_SC_PAGESIZE) as u64 }
}

fn read_rss_kb() -> u64 {
    fs::read_to_string("/proc/self/statm")
        .ok()
        .and_then(|raw| {
            let mut parts = raw.split_whitespace();
            let _ = parts.next();
            let pages: u64 = parts.next()?.parse().ok()?;
            Some(pages * page_size() / 1024)
        })
        .unwrap_or(0)
}

fn read_vm_peak_kb() -> u64 {
    parse_status_kb("VmPeak:")
}

fn read_vm_rss_kb() -> u64 {
    parse_status_kb("VmRSS:")
}

fn read_vm_data_kb() -> u64 {
    parse_status_kb("VmData:")
}

fn read_vm_stk_kb() -> u64 {
    parse_status_kb("VmStk:")
}

fn read_vm_swap_kb() -> u64 {
    parse_status_kb("VmSwap:")
}

fn parse_status_kb(key: &str) -> u64 {
    fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|body| {
            body.lines()
                .find(|entry| entry.starts_with(key))
                .and_then(|entry| entry.split_whitespace().nth(1)?.parse().ok())
        })
        .unwrap_or(0)
}

fn read_fd_count() -> usize {
    fs::read_dir("/proc/self/fd")
        .map(|entries| entries.count())
        .unwrap_or(0)
}

fn read_thread_count() -> u64 {
    fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|body| {
            body.lines()
                .find(|entry| entry.starts_with("Threads:"))
                .and_then(|entry| entry.split_whitespace().nth(1)?.parse().ok())
        })
        .unwrap_or(0)
}

#[derive(Debug, Clone, Default)]
struct SmapsRegion {
    addr: String,
    perms: String,
    path: String,
    size_kb: u64,
    rss_kb: u64,
    pss_kb: u64,
    private_dirty_kb: u64,
    category: String,
}

fn classify_path(path: &str) -> String {
    let raw = path.trim();
    if raw == "[heap]" {
        return "heap (brk)".to_string();
    }
    if raw.starts_with("[stack") {
        return "thread stack".to_string();
    }
    if raw.is_empty() || raw == "[anon]" {
        return "anon mmap (glibc arena / raw buf)".to_string();
    }
    if raw.starts_with("/dev/shm") || raw.contains("memfd") {
        return "shared mem (wayland/shm)".to_string();
    }
    let lower = raw.to_lowercase();
    if lower.ends_with(".ttf")
        || lower.ends_with(".otf")
        || lower.ends_with(".woff")
        || lower.contains("font")
        || lower.contains("pango")
    {
        return "font / pango".to_string();
    }
    if lower.contains("vulkan")
        || lower.contains("libgl")
        || lower.contains("mesa")
        || lower.contains("nvidia")
        || lower.contains("radeon")
    {
        return "gpu driver / gl / vulkan".to_string();
    }
    if lower.contains("libgtk")
        || lower.contains("libadw")
        || lower.contains("libgsk")
        || lower.contains("libgdk")
        || lower.contains("libglib")
    {
        return "gtk / adw libs".to_string();
    }
    if lower.contains("libgio") || lower.contains("gvfs") {
        return "gio / gvfs".to_string();
    }
    if lower.contains("/flux") {
        return "flux binary".to_string();
    }
    "other mapped files".to_string()
}

fn read_detailed_smaps() -> Vec<SmapsRegion> {
    let content = fs::read_to_string("/proc/self/smaps").unwrap_or_default();
    let mut mapped_regions = Vec::with_capacity(512);
    let mut current_region = SmapsRegion::default();
    let mut parsing_region = false;

    for line in content.lines() {
        if line.contains('-') && line.contains(' ') && !line.starts_with(' ') {
            if parsing_region {
                current_region.category = classify_path(&current_region.path);
                mapped_regions.push(current_region.clone());
            }
            current_region = SmapsRegion::default();
            parsing_region = true;

            let mut tokens = line.splitn(6, ' ');
            current_region.addr = tokens.next().unwrap_or("").to_string();
            current_region.perms = tokens.next().unwrap_or("").to_string();
            let _ = tokens.next();
            let _ = tokens.next();
            let _ = tokens.next();
            current_region.path = tokens.next().unwrap_or("").trim().to_string();
            continue;
        }

        if !parsing_region {
            continue;
        }

        if let Some(val) = line.strip_prefix("Size:") {
            current_region.size_kb = val
                .split_whitespace()
                .next()
                .and_then(|num| num.parse().ok())
                .unwrap_or(0);
        } else if let Some(val) = line.strip_prefix("Rss:") {
            current_region.rss_kb = val
                .split_whitespace()
                .next()
                .and_then(|num| num.parse().ok())
                .unwrap_or(0);
        } else if let Some(val) = line.strip_prefix("Pss:") {
            current_region.pss_kb = val
                .split_whitespace()
                .next()
                .and_then(|num| num.parse().ok())
                .unwrap_or(0);
        } else if let Some(val) = line.strip_prefix("Private_Dirty:") {
            current_region.private_dirty_kb = val
                .split_whitespace()
                .next()
                .and_then(|num| num.parse().ok())
                .unwrap_or(0);
        }
    }

    if parsing_region {
        current_region.category = classify_path(&current_region.path);
        mapped_regions.push(current_region);
    }

    mapped_regions
}

#[derive(Debug, Clone, Default)]
struct MallinfoSnap {
    arena_bytes: usize,
    ordblks: usize,
    uordblks_bytes: usize,
    fordblks_bytes: usize,
    hblkhd_bytes: usize,
}

fn read_mallinfo() -> MallinfoSnap {
    #[cfg(target_os = "linux")]
    {
        #[repr(C)]
        struct StructMallinfo2 {
            arena: usize,
            ordblks: usize,
            smblks: usize,
            hblks: usize,
            hblkhd: usize,
            usmblks: usize,
            fsmblks: usize,
            uordblks: usize,
            fordblks: usize,
            keepcost: usize,
        }

        extern "C" {
            fn mallinfo2() -> StructMallinfo2;
        }

        let raw_info = unsafe { mallinfo2() };
        MallinfoSnap {
            arena_bytes: raw_info.arena,
            ordblks: raw_info.ordblks,
            uordblks_bytes: raw_info.uordblks,
            fordblks_bytes: raw_info.fordblks,
            hblkhd_bytes: raw_info.hblkhd,
        }
    }
    #[cfg(not(target_os = "linux"))]
    {
        MallinfoSnap::default()
    }
}

fn fmt_kb(kb: u64) -> String {
    if kb >= 1_048_576 {
        format!("{:.1} GB", kb as f64 / 1_048_576.0)
    } else if kb >= 1_024 {
        format!("{:.1} MB", kb as f64 / 1_024.0)
    } else {
        format!("{} KB", kb)
    }
}

fn fmt_bytes(total: u64) -> String {
    fmt_kb(total / 1024)
}

#[derive(Debug, Clone)]
struct GridItemSnap {
    idx: u32,
    name: String,
    path: PathBuf,
    size: u64,
    mtime: i64,
    is_dir: bool,
    icon_size: i32,
    is_list_mode: bool,
    is_editing: bool,
    is_foreign: bool,
    is_custom_icon: bool,
    expand_labels: bool,
    has_texture: bool,
    max_width_chars: i32,
    grid_spacing: i32,
    rc_strong: usize,
    rc_weak: usize,
    active_path_ok: bool,
    active_path_val: Option<PathBuf>,
    icon_type: String,
}

#[derive(Debug, Clone)]
struct CacheSnap {
    path: PathBuf,
    item_count: usize,
    thumb_count: usize,
    media_task_count: usize,
    age_secs: u64,
    heap_approx: usize,
    head: Vec<String>,
    tail: Vec<String>,
    thumb_keys: Vec<String>,
}

#[derive(Debug, Clone)]
struct CategorySummary {
    category: String,
    count: usize,
    size_kb: u64,
    rss_kb: u64,
    dirty_kb: u64,
}

#[derive(Debug, Clone)]
struct FontMap {
    name: String,
    bytes: u64,
    count: usize,
}

#[derive(Debug, Clone)]
struct AppStateSnap {
    current_path: PathBuf,
    is_loading: bool,
    is_list_mode: bool,
    show_hidden: bool,
    sort_by: String,
    sort_ascending: bool,
    filter: String,
    extension_filter: Option<Vec<String>>,
    current_icon_size: i32,
    current_list_icon_size: i32,
    load_id: u64,
    history_depth: usize,
    forward_stack_depth: usize,
    recent_stack: Vec<PathBuf>,
    pending_thumbnails: Vec<u32>,
    sidebar_count: usize,
    breadcrumb_count: usize,
    exclusive_list: Vec<PathBuf>,
    exclusive_index: Option<usize>,
    task_queue_len: usize,
    has_directory_monitor: bool,
    header_view: String,
    config_max_width_chars: i32,
    config_grid_spacing: i32,
    config_lazy_thumbnails: bool,
    config_show_thumbnails: bool,
    config_folders_first: bool,
    config_expand_labels: bool,
    folder_icons_count: usize,
    file_icons_count: usize,
}

#[derive(Debug, Clone)]
struct DebugSnapshot {
    rss_kb: u64,
    vm_peak_kb: u64,
    vm_data_kb: u64,
    vm_stk_kb: u64,
    vm_swap_kb: u64,
    fd_count: usize,
    thread_count: u64,
    grid_len: usize,
    grid_items: Vec<GridItemSnap>,
    leaked_rcs: Vec<(u32, usize, String)>,
    active_path_mismatches: Vec<(u32, String, Option<PathBuf>)>,
    items_with_texture: usize,
    items_editing: usize,
    items_foreign: usize,
    items_custom_icon: usize,
    app: AppStateSnap,
    caches: Vec<CacheSnap>,
    categories: Vec<CategorySummary>,
    largest_anon_regions: Vec<SmapsRegion>,
    mallinfo: MallinfoSnap,
    font_maps: Vec<FontMap>,
    font_total_bytes: u64,
    pending_thumb_paths: Vec<u32>,
}

impl DebugSnapshot {
    fn capture(app: &FluxApp) -> Self {
        let mut grid_items = Vec::with_capacity(app.files.len() as usize);
        let mut leaked_rcs = Vec::new();
        let mut active_path_mismatches = Vec::new();

        for i in 0..app.files.len() {
            if let Some(guard) = app.files.get(i) {
                let item = guard.borrow();
                let strong = Rc::strong_count(&item.active_path);
                let weak = Rc::weak_count(&item.active_path);
                let ap_val = item.active_path.borrow().clone();
                let ap_ok = ap_val.as_ref().map(|p| *p == item.path).unwrap_or(false);

                if strong > 1 {
                    leaked_rcs.push((item.grid_idx, strong, item.name.clone()));
                }
                if !ap_ok {
                    active_path_mismatches.push((item.grid_idx, item.name.clone(), ap_val.clone()));
                }

                let icon_type = format!("{:?}", item.icon)
                    .split('(')
                    .next()
                    .unwrap_or("?")
                    .to_string();

                grid_items.push(GridItemSnap {
                    idx: item.grid_idx,
                    name: item.name.clone(),
                    path: item.path.clone(),
                    size: item.size,
                    mtime: item.mtime,
                    is_dir: item.is_dir,
                    icon_size: item.icon_size,
                    is_list_mode: item.is_list_mode,
                    is_editing: item.is_editing,
                    is_foreign: item.is_foreign_owner,
                    is_custom_icon: item.is_custom_icon,
                    expand_labels: item.expand_labels,
                    has_texture: item.thumbnail.is_some(),
                    max_width_chars: item.max_width_chars,
                    grid_spacing: item.grid_spacing,
                    rc_strong: strong,
                    rc_weak: weak,
                    active_path_ok: ap_ok,
                    active_path_val: ap_val,
                    icon_type,
                });
            }
        }

        let items_with_texture = grid_items.iter().filter(|i| i.has_texture).count();
        let items_editing = grid_items.iter().filter(|i| i.is_editing).count();
        let items_foreign = grid_items.iter().filter(|i| i.is_foreign).count();
        let items_custom_icon = grid_items.iter().filter(|i| i.is_custom_icon).count();

        let app_snap = AppStateSnap {
            current_path: app.current_path.clone(),
            is_loading: app.is_loading,
            is_list_mode: app.is_list_mode,
            show_hidden: app.show_hidden,
            sort_by: format!("{:?}", app.sort_by),
            sort_ascending: app.sort_ascending,
            filter: app.filter.clone(),
            extension_filter: app.extension_filter.clone(),
            current_icon_size: app.current_icon_size,
            current_list_icon_size: app.current_list_icon_size,
            load_id: app.load_id.load(std::sync::atomic::Ordering::SeqCst),
            history_depth: app.history.len(),
            forward_stack_depth: app.forward_stack.len(),
            recent_stack: app.recent_stack.iter().cloned().collect(),
            pending_thumbnails: app.pending_thumbnails.iter().cloned().collect(),
            sidebar_count: app.sidebar.len(),
            breadcrumb_count: app.breadcrumbs.len(),
            exclusive_list: app.exclusive_list.clone(),
            exclusive_index: app.exclusive_index,
            task_queue_len: app.task_queue.len(),
            has_directory_monitor: app.directory_monitor.is_some(),
            header_view: app.header_view.clone(),
            config_max_width_chars: app.config.ui.max_width_chars,
            config_grid_spacing: app.config.ui.grid_spacing,
            config_lazy_thumbnails: app.config.ui.lazy_thumbnails,
            config_show_thumbnails: app.config.ui.show_thumbnails,
            config_folders_first: app.config.ui.folders_first,
            config_expand_labels: app.config.ui.expand_labels,
            folder_icons_count: app.config.ui.folder_icons.len(),
            file_icons_count: app.config.ui.file_icons.len(),
        };

        let start_time = std::time::Instant::now();
        let caches = app
            .folder_cache
            .iter()
            .map(|(path, cached)| {
                let heap: usize = cached
                    .items
                    .iter()
                    .map(|ctx| {
                        ctx.display_name.len()
                            + ctx.target_path.as_os_str().len()
                            + ctx.sort_name.len()
                            + ctx.sort_ext.len()
                            + ctx
                                .thumbnail_path
                                .as_ref()
                                .map(|p| p.as_os_str().len())
                                .unwrap_or(0)
                            + ctx.custom_icon.as_ref().map(|s| s.len()).unwrap_or(0)
                            + std::mem::size_of::<crate::model::FileLoadContext>()
                    })
                    .sum();

                let names: Vec<String> = cached
                    .items
                    .iter()
                    .map(|i| i.display_name.clone())
                    .collect();
                let head = names.iter().take(5).cloned().collect();
                let tail = names
                    .iter()
                    .rev()
                    .take(5)
                    .cloned()
                    .collect::<Vec<_>>()
                    .into_iter()
                    .rev()
                    .collect();

                let thumb_keys: Vec<String> = cached
                    .thumbnails
                    .keys()
                    .map(|p| {
                        p.file_name()
                            .map(|n| n.to_string_lossy().to_string())
                            .unwrap_or_default()
                    })
                    .collect();

                CacheSnap {
                    path: path.clone(),
                    item_count: cached.items.len(),
                    thumb_count: cached.thumbnails.len(),
                    media_task_count: cached.media_tasks.len(),
                    age_secs: start_time.duration_since(cached.last_visited).as_secs(),
                    heap_approx: heap,
                    head,
                    tail,
                    thumb_keys,
                }
            })
            .collect();

        let regions = read_detailed_smaps();

        let mut cat_map: std::collections::HashMap<String, CategorySummary> =
            std::collections::HashMap::new();
        for r in &regions {
            let entry = cat_map
                .entry(r.category.clone())
                .or_insert_with(|| CategorySummary {
                    category: r.category.clone(),
                    count: 0,
                    size_kb: 0,
                    rss_kb: 0,
                    dirty_kb: 0,
                });
            entry.count += 1;
            entry.size_kb += r.size_kb;
            entry.rss_kb += r.rss_kb;
            entry.dirty_kb += r.private_dirty_kb;
        }
        let mut categories: Vec<CategorySummary> = cat_map.into_values().collect();
        categories.sort_by_key(|a| std::cmp::Reverse(a.rss_kb));

        let mut anon_regions: Vec<SmapsRegion> = regions
            .into_iter()
            .filter(|r| {
                r.category.contains("anon")
                    || r.category.contains("heap")
                    || r.category.contains("shared")
            })
            .collect();
        anon_regions.sort_by_key(|a| std::cmp::Reverse(a.rss_kb));
        anon_regions.truncate(12);

        let mallinfo = read_mallinfo();

        let maps = read_detailed_smaps();
        let mut font_dedup: std::collections::HashMap<String, (u64, usize)> =
            std::collections::HashMap::new();
        for r in &maps {
            let lower = r.path.to_lowercase();
            if lower.ends_with(".ttf")
                || lower.ends_with(".otf")
                || lower.ends_with(".woff")
                || lower.ends_with(".woff2")
                || lower.contains("font")
                || lower.contains("pango")
            {
                let entry = font_dedup.entry(r.path.clone()).or_insert((0, 0));
                entry.0 += r.size_kb * 1024;
                entry.1 += 1;
            }
        }
        let mut font_maps: Vec<FontMap> = font_dedup
            .into_iter()
            .map(|(path_key, (bytes_val, count_val))| FontMap {
                name: std::path::Path::new(&path_key)
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or(path_key),
                bytes: bytes_val,
                count: count_val,
            })
            .collect();
        font_maps.sort_by_key(|a| std::cmp::Reverse(a.bytes));
        let font_total_bytes: u64 = font_maps.iter().map(|f| f.bytes).sum();

        let pending_thumb_paths = app.pending_thumbnails.iter().cloned().collect();

        Self {
            rss_kb: read_rss_kb(),
            vm_peak_kb: read_vm_peak_kb(),
            vm_data_kb: read_vm_data_kb(),
            vm_stk_kb: read_vm_stk_kb(),
            vm_swap_kb: read_vm_swap_kb(),
            fd_count: read_fd_count(),
            thread_count: read_thread_count(),
            grid_len: grid_items.len(),
            leaked_rcs,
            active_path_mismatches,
            items_with_texture,
            items_editing,
            items_foreign,
            items_custom_icon,
            grid_items,
            app: app_snap,
            caches,
            categories,
            largest_anon_regions: anon_regions,
            mallinfo,
            font_maps,
            font_total_bytes,
            pending_thumb_paths,
        }
    }

    fn render(&self) -> String {
        let mut buffer = String::with_capacity(32_768);

        let _ = writeln!(
            buffer,
            "═══════════════════════════════════════════════════════════════"
        );
        let _ = writeln!(buffer, "  FLUX DEEP MEMORY & ALLOCATOR PROFILE");
        let _ = writeln!(
            buffer,
            "═══════════════════════════════════════════════════════════════\n"
        );

        let _ = writeln!(
            buffer,
            "── Process Memory & glibc mallinfo2 ────────────────────────────"
        );
        let _ = writeln!(
            buffer,
            "  RSS (resident):   {:<10}  VmPeak:         {}",
            fmt_kb(self.rss_kb),
            fmt_kb(self.vm_peak_kb)
        );
        let _ = writeln!(
            buffer,
            "  VmData (heap):    {:<10}  VmSwap:         {}",
            fmt_kb(self.vm_data_kb),
            fmt_kb(self.vm_swap_kb)
        );
        let _ = writeln!(
            buffer,
            "  Threads:          {:<10}  Open FDs:       {}",
            self.thread_count, self.fd_count
        );
        let _ = writeln!(
            buffer,
            "  glibc Arena:      {:<10} (total space allocated by sbrk/mmap)",
            fmt_bytes(self.mallinfo.arena_bytes as u64)
        );
        let _ = writeln!(
            buffer,
            "  glibc In-Use:     {:<10} (live user allocations)",
            fmt_bytes(self.mallinfo.uordblks_bytes as u64)
        );
        let _ = writeln!(
            buffer,
            "  glibc Free:       {:<10} (cached by glibc, not released to kernel)",
            fmt_bytes(self.mallinfo.fordblks_bytes as u64)
        );
        let _ = writeln!(
            buffer,
            "  glibc mmap HD:    {:<10} (large chunks allocated via direct mmap)",
            fmt_bytes(self.mallinfo.hblkhd_bytes as u64)
        );
        let _ = writeln!(buffer);

        let _ = writeln!(
            buffer,
            "── Memory Map Summary (/proc/self/smaps) ───────────────────────"
        );
        let _ = writeln!(
            buffer,
            "  {:<34} {:>5}  {:>10}  {:>10}  {:>10}",
            "Category", "Count", "Virt Size", "Rss", "Dirty"
        );
        let _ = writeln!(
            buffer,
            "  {:-<34} {:-<5}  {:-<10}  {:-<10}  {:-<10}",
            "", "", "", "", ""
        );
        for entry in &self.categories {
            let _ = writeln!(
                buffer,
                "  {:<34} {:>5}  {:>10}  {:>10}  {:>10}",
                entry.category,
                entry.count,
                fmt_kb(entry.size_kb),
                fmt_kb(entry.rss_kb),
                fmt_kb(entry.dirty_kb)
            );
        }
        let _ = writeln!(buffer);

        let _ = writeln!(
            buffer,
            "── Top Anonymous / Heap Mappings (Physical RSS) ────────────────"
        );
        let _ = writeln!(
            buffer,
            "  {:<24} {:<5} {:>10} {:>10} {:>10}  Path/Type",
            "Address Range", "Perms", "Size", "Rss", "Dirty"
        );
        let _ = writeln!(
            buffer,
            "  {:-<24} {:-<5} {:-<10} {:-<10} {:-<10}  {:-<20}",
            "", "", "", "", "", ""
        );
        for reg in &self.largest_anon_regions {
            let label = if reg.path.is_empty() {
                "[anon: mmap/arena]"
            } else {
                &reg.path
            };
            let _ = writeln!(
                buffer,
                "  {:<24} {:<5} {:>10} {:>10} {:>10}  {}",
                reg.addr,
                reg.perms,
                fmt_kb(reg.size_kb),
                fmt_kb(reg.rss_kb),
                fmt_kb(reg.private_dirty_kb),
                label
            );
        }
        let _ = writeln!(buffer);

        let app_state = &self.app;
        let _ = writeln!(
            buffer,
            "── App State ───────────────────────────────────────────────────"
        );
        let _ = writeln!(
            buffer,
            "  current_path:     {}",
            app_state.current_path.display()
        );
        let _ = writeln!(buffer, "  load_id:          {}", app_state.load_id);
        let _ = writeln!(buffer, "  is_loading:       {}", app_state.is_loading);
        let _ = writeln!(
            buffer,
            "  icon_size:        {}px (list: {}px)",
            app_state.current_icon_size, app_state.current_list_icon_size
        );
        let _ = writeln!(buffer, "  task_queue len:   {}", app_state.task_queue_len);
        let _ = writeln!(
            buffer,
            "  config: max_width_chars={} grid_spacing={} lazy_thumbs={} show_thumbs={}",
            app_state.config_max_width_chars,
            app_state.config_grid_spacing,
            app_state.config_lazy_thumbnails,
            app_state.config_show_thumbnails
        );
        let _ = writeln!(buffer);

        let _ = writeln!(
            buffer,
            "── Grid Overview ({} items) ─────────────────────────────────────",
            self.grid_len
        );
        let _ = writeln!(buffer, "  textures loaded:  {}", self.items_with_texture);
        let _ = writeln!(
            buffer,
            "  pending thumbs:   {}",
            self.pending_thumb_paths.len()
        );

        if self.leaked_rcs.is_empty() {
            let _ = writeln!(buffer, "  Rc anomalies:     none ✓");
        } else {
            let _ = writeln!(buffer, "  ⚠ Rc strong > 1: {} items", self.leaked_rcs.len());
        }
        let _ = writeln!(buffer);

        let _ = writeln!(
            buffer,
            "── Folder Cache ({} entries) ────────────────────────────────────",
            self.caches.len()
        );
        for item in &self.caches {
            let _ = writeln!(
                buffer,
                "  {:?} -> items={} thumbs={} age={}s",
                item.path, item.item_count, item.thumb_count, item.age_secs
            );
        }
        let _ = writeln!(buffer);

        let _ = writeln!(
            buffer,
            "── Font / Pango Maps ({} files, total {}) ───────────────────────",
            self.font_maps.len(),
            fmt_bytes(self.font_total_bytes)
        );
        for font in &self.font_maps {
            let _ = writeln!(
                buffer,
                "  {:>9}  x{}  {}",
                fmt_bytes(font.bytes),
                font.count,
                font.name
            );
        }
        let _ = writeln!(buffer);

        buffer
    }
}

pub fn show_debug_window(app: &FluxApp) {
    let snapshot = DebugSnapshot::capture(app);
    let output = snapshot.render();

    let window = adw::Window::builder()
        .title("Flux Memory & Allocator Profiler")
        .default_width(1050)
        .default_height(800)
        .build();

    let header_bar = adw::HeaderBar::new();

    let refresh_button = gtk::Button::builder()
        .icon_name("view-refresh-symbolic")
        .tooltip_text("Refresh (re-samples /proc/self/smaps)")
        .build();
    let trim_button = gtk::Button::builder()
        .label("malloc_trim(0)")
        .tooltip_text("Call libc::malloc_trim(0) to release free arena pages to kernel")
        .build();
    let export_button = gtk::Button::builder()
        .icon_name("document-save-symbolic")
        .tooltip_text("Save to /tmp/flux_debug.txt")
        .build();

    header_bar.pack_start(&refresh_button);
    header_bar.pack_start(&trim_button);
    header_bar.pack_end(&export_button);

    let view = gtk::TextView::builder()
        .editable(false)
        .monospace(true)
        .left_margin(12)
        .right_margin(12)
        .top_margin(8)
        .bottom_margin(8)
        .build();

    let doc_buffer = view.buffer();
    doc_buffer.set_text(&output);

    let scroll_container = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Automatic)
        .vscrollbar_policy(gtk::PolicyType::Automatic)
        .hexpand(true)
        .vexpand(true)
        .build();
    scroll_container.set_child(Some(&view));

    let status_bar = gtk::Label::builder()
        .label(format!(
            "RSS {}  │  Heap {}  │  {} threads  │  {} FDs",
            fmt_kb(snapshot.rss_kb),
            fmt_bytes(snapshot.mallinfo.uordblks_bytes as u64),
            snapshot.thread_count,
            snapshot.fd_count,
        ))
        .halign(gtk::Align::Start)
        .margin_start(12)
        .margin_end(12)
        .margin_top(4)
        .margin_bottom(4)
        .build();
    status_bar.add_css_class("caption");
    status_bar.add_css_class("dim-label");

    let layout_root = gtk::Box::new(gtk::Orientation::Vertical, 0);
    layout_root.append(&header_bar);
    layout_root.append(&scroll_container);
    layout_root.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
    layout_root.append(&status_bar);
    window.set_content(Some(&layout_root));

    trim_button.connect_clicked(|_| {
        unsafe {
            libc::malloc_trim(0);
        }
        if let Some(chan) = crate::model::SENDER.get() {
            let _ = chan.send(crate::model::AppMsg::OpenDebugWindow);
        }
    });

    {
        let report_payload = output.clone();
        export_button.connect_clicked(move |_| {
            let target_file = "/tmp/flux_debug.txt";
            if std::fs::write(target_file, report_payload.as_bytes()).is_ok() {
                eprintln!("[debug] saved to {}", target_file);
            }
        });
    }

    refresh_button.connect_clicked(move |_| {
        if let Some(chan) = crate::model::SENDER.get() {
            let _ = chan.send(crate::model::AppMsg::OpenDebugWindow);
        }
    });

    window.present();
}
