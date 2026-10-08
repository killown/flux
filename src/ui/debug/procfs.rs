use std::fs;

pub(super) fn page_size() -> u64 {
    unsafe { libc::sysconf(libc::_SC_PAGESIZE) as u64 }
}

pub(super) fn read_rss_kb() -> u64 {
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

pub(super) fn read_vm_peak_kb() -> u64 {
    parse_status_kb("VmPeak:")
}

pub(super) fn read_vm_data_kb() -> u64 {
    parse_status_kb("VmData:")
}

pub(super) fn read_vm_stk_kb() -> u64 {
    parse_status_kb("VmStk:")
}

pub(super) fn read_vm_swap_kb() -> u64 {
    parse_status_kb("VmSwap:")
}

pub(super) fn read_fd_count() -> usize {
    fs::read_dir("/proc/self/fd")
        .map(|entries| entries.count())
        .unwrap_or(0)
}

pub(super) fn read_thread_count() -> u64 {
    fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|body| {
            body.lines()
                .find(|entry| entry.starts_with("Threads:"))
                .and_then(|entry| entry.split_whitespace().nth(1)?.parse().ok())
        })
        .unwrap_or(0)
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
