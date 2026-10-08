use super::format::{fmt_bytes, fmt_kb};
use super::snapshot::DebugSnapshot;
use std::fmt::Write as FmtWrite;

impl DebugSnapshot {
    pub(crate) fn render(&self) -> String {
        let mut buffer = String::with_capacity(65_536);

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
            "── Kernel Process Memory (/proc/self/status & statm) ──────────"
        );
        let _ = writeln!(
            buffer,
            "  RSS (Physical):   {:<10}  VmPeak (Virtual): {}",
            fmt_kb(self.rss_kb),
            fmt_kb(self.vm_peak_kb)
        );
        let _ = writeln!(
            buffer,
            "  VmData (Heap):    {:<10}  VmSwap:           {}",
            fmt_kb(self.vm_data_kb),
            fmt_kb(self.vm_swap_kb)
        );
        let _ = writeln!(
            buffer,
            "  Threads:          {:<10}  Open FDs:         {}",
            self.thread_count, self.fd_count
        );
        let _ = writeln!(buffer);

        let _ = writeln!(
            buffer,
            "── Allocators (mimalloc & glibc mallinfo2) ─────────────────────"
        );
        let _ = writeln!(
            buffer,
            "  [Note: Flux compiles with mimalloc as #[global_allocator]. Rust heap"
        );
        let _ = writeln!(
            buffer,
            "   allocations route via mimalloc; glibc mallinfo2 below reflects only C"
        );
        let _ = writeln!(
            buffer,
            "   libraries such as GObject, GTK4 internals, and SQLite.]\n"
        );
        let _ = writeln!(
            buffer,
            "  glibc Arena:      {:<10} (total space allocated by sbrk/mmap)",
            fmt_bytes(self.mallinfo.arena_bytes as u64)
        );
        let _ = writeln!(
            buffer,
            "  glibc In-Use:     {:<10} (live C user allocations)",
            fmt_bytes(self.mallinfo.uordblks_bytes as u64)
        );
        let _ = writeln!(
            buffer,
            "  glibc Free:       {:<10} (cached by glibc allocator pool)",
            fmt_bytes(self.mallinfo.fordblks_bytes as u64)
        );
        let _ = writeln!(
            buffer,
            "  glibc mmap HD:    {:<10} (large chunks allocated via direct mmap)",
            fmt_bytes(self.mallinfo.hblkhd_bytes as u64)
        );
        let _ = writeln!(buffer);

        if !self.mimalloc_stats.trim().is_empty() {
            let _ = writeln!(
                buffer,
                "── mimalloc Internal Runtime Stats ─────────────────────────────"
            );
            for line in self.mimalloc_stats.lines() {
                let _ = writeln!(buffer, "  {}", line);
            }
            let _ = writeln!(buffer);
        }

        let _ = writeln!(
            buffer,
            "── Memory Map Summary (/proc/self/smaps) ───────────────────────"
        );
        let _ = writeln!(
            buffer,
            "  {:<42} {:>5}  {:>10}  {:>10}  {:>10}",
            "Category", "Count", "Virt Size", "Rss", "Dirty"
        );
        let _ = writeln!(
            buffer,
            "  {:-<42} {:-<5}  {:-<10}  {:-<10}  {:-<10}",
            "", "", "", "", ""
        );
        for entry in &self.categories {
            let _ = writeln!(
                buffer,
                "  {:<42} {:>5}  {:>10}  {:>10}  {:>10}",
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

        let _ = writeln!(
            buffer,
            "── Top Mapped Files (Physical RSS) ─────────────────────────────"
        );
        let _ = writeln!(
            buffer,
            "  {:<24} {:<5} {:>10} {:>10} {:>10}  File Path",
            "Address Range", "Perms", "Size", "Rss", "Dirty"
        );
        let _ = writeln!(
            buffer,
            "  {:-<24} {:-<5} {:-<10} {:-<10} {:-<10}  {:-<20}",
            "", "", "", "", "", ""
        );
        for reg in &self.largest_file_regions {
            let _ = writeln!(
                buffer,
                "  {:<24} {:<5} {:>10} {:>10} {:>10}  {}",
                reg.addr,
                reg.perms,
                fmt_kb(reg.size_kb),
                fmt_kb(reg.rss_kb),
                fmt_kb(reg.private_dirty_kb),
                reg.path
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
            "  current_path:           {}",
            app_state.current_path.display()
        );
        let _ = writeln!(buffer, "  load_id:                {}", app_state.load_id);
        let _ = writeln!(buffer, "  is_loading:             {}", app_state.is_loading);
        let _ = writeln!(
            buffer,
            "  icon_size:              {}px (list: {}px)",
            app_state.current_icon_size, app_state.current_list_icon_size
        );
        let _ = writeln!(
            buffer,
            "  task_queue len:         {}",
            app_state.task_queue_len
        );
        let _ = writeln!(
            buffer,
            "  folder_cache_capacity:  {}",
            app_state.folder_cache_capacity
        );
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
        let _ = writeln!(
            buffer,
            "  textures loaded:        {}",
            self.items_with_texture
        );
        let _ = writeln!(
            buffer,
            "  pending thumbs:         {}",
            self.pending_thumb_paths.len()
        );
        let _ = writeln!(
            buffer,
            "  custom icons active:    {}",
            self.items_custom_icon
        );

        if self.leaked_rcs.is_empty() {
            let _ = writeln!(buffer, "  Rc anomalies:           none ✓");
        } else {
            let _ = writeln!(
                buffer,
                "  ⚠ Rc strong > 1:        {} items",
                self.leaked_rcs.len()
            );
        }
        let _ = writeln!(buffer);

        let _ = writeln!(
            buffer,
            "── Folder Cache ({} entries) ────────────────────────────────────",
            self.caches.len()
        );
        if self.caches.is_empty() {
            let _ = writeln!(
                buffer,
                "  (empty - folder_cache_capacity = 0 or no cached folders)"
            );
        } else {
            for item in &self.caches {
                let _ = writeln!(
                    buffer,
                    "  {:?} -> items={} thumbs={} age={}s approx_heap={}",
                    item.path,
                    item.item_count,
                    item.thumb_count,
                    item.age_secs,
                    fmt_bytes(item.heap_approx as u64)
                );
            }
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

        let _ = writeln!(
            buffer,
            "── HWGA Call Frequency & Timing ({} probes) ─────────────────────",
            self.hwga.len()
        );
        if self.hwga.is_empty() {
            let _ = writeln!(
                buffer,
                "  (no data - release build or no instrumented calls yet)"
            );
        } else {
            let _ = writeln!(
                buffer,
                "  {:>7}  {:>10}  {:>10}  {:>10}  {:>10}  {:>9}  Function",
                "Calls", "Total", "Self", "p95", "Max", "Main>16ms"
            );
            let _ = writeln!(
                buffer,
                "  {:-<7}  {:-<10}  {:-<10}  {:-<10}  {:-<10}  {:-<9}  {:-<20}",
                "", "", "", "", "", "", ""
            );
            for (name, stats) in &self.hwga {
                let main = if stats.main_count > 0 {
                    format!("{}/{}", stats.main_over_budget, stats.main_count)
                } else {
                    "-".to_string()
                };
                let _ = writeln!(
                    buffer,
                    "  {:>7}  {:>10}  {:>10}  {:>10}  {:>10}  {:>9}  {}",
                    stats.count,
                    format!("{:.2?}", stats.total_time),
                    format!("{:.2?}", stats.self_time),
                    format!("{:.2?}", stats.p95),
                    format!("{:.2?}", stats.max_time),
                    main,
                    name
                );
            }
        }
        let _ = writeln!(buffer);

        buffer
    }
}
