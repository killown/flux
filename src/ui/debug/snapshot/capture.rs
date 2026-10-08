use super::super::ffi::read_mimalloc_stats;
use super::super::mallinfo::read_mallinfo;
use super::super::procfs::{
    read_fd_count, read_rss_kb, read_thread_count, read_vm_data_kb, read_vm_peak_kb,
    read_vm_stk_kb, read_vm_swap_kb,
};
use super::super::smaps::{read_detailed_smaps, SmapsRegion};
use super::model::*;
use crate::model::FluxApp;
use std::rc::Rc;

impl DebugSnapshot {
    pub(crate) fn capture(app: &FluxApp) -> Self {
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
            folder_cache_capacity: app.config.ui.folder_cache_capacity,
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
            .iter()
            .filter(|r| {
                r.category.contains("anon")
                    || r.category.contains("heap")
                    || r.category.contains("shared")
            })
            .cloned()
            .collect();
        anon_regions.sort_by_key(|a| std::cmp::Reverse(a.rss_kb));
        anon_regions.truncate(10);

        let mut file_regions: Vec<SmapsRegion> = regions
            .into_iter()
            .filter(|r| {
                !r.path.is_empty() && !r.path.starts_with('[') && !r.category.contains("shared")
            })
            .collect();
        file_regions.sort_by_key(|a| std::cmp::Reverse(a.rss_kb));
        file_regions.truncate(10);

        let mallinfo = read_mallinfo();
        let mimalloc_stats = read_mimalloc_stats();

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

        DebugSnapshot {
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
            largest_file_regions: file_regions,
            mallinfo,
            mimalloc_stats,
            font_maps,
            font_total_bytes,
            pending_thumb_paths,
            hwga: crate::utils::hwga::CallMonitor::snapshot(),
        }
    }
}
