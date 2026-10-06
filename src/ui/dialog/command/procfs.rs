use std::fs;

// ─── libc helpers ────────────────────────────────────────────────────────────

fn page_size() -> usize {
    unsafe { libc::sysconf(libc::_SC_PAGESIZE) as usize }
}

fn clock_tick() -> f64 {
    unsafe { libc::sysconf(libc::_SC_CLK_TCK) as f64 }
}

// ─── /proc parsing helpers ──────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct ProcStat {
    pub pid: u32,
    pub ppid: u32,
    pub state: char,
    pub utime: f64,
    pub stime: f64,
    #[allow(dead_code)]
    pub nice: i32,
    pub num_threads: u64,
    pub rss: u64,
    #[allow(dead_code)]
    pub vms: u64,
}

pub(super) fn parse_stat(pid: u32) -> Option<ProcStat> {
    let path = format!("/proc/{}/stat", pid);
    let content = fs::read_to_string(&path).ok()?;
    let _open_paren = content.find('(')?;
    let close_paren = content.rfind(')')?;
    let after_comm = &content[close_paren + 2..];
    let parts: Vec<&str> = after_comm.split_whitespace().collect();
    if parts.len() < 22 {
        return None;
    }
    let state = parts[0].chars().next()?;
    let ppid: u32 = parts[1].parse().ok()?;
    let utime: u64 = parts[11].parse().ok()?;
    let stime: u64 = parts[12].parse().ok()?;
    let nice: i32 = parts[16].parse().ok()?;
    let num_threads: u64 = parts[17].parse().ok()?;
    let rss_pages: u64 = parts[21].parse().ok()?;

    let tick = clock_tick();
    let page_sz = page_size() as u64;
    Some(ProcStat {
        pid,
        ppid,
        state,
        utime: utime as f64 / tick,
        stime: stime as f64 / tick,
        nice,
        num_threads,
        rss: rss_pages * page_sz,
        vms: 0,
    })
}

pub(super) fn parse_statm(pid: u32) -> Option<u64> {
    let path = format!("/proc/{}/statm", pid);
    let content = fs::read_to_string(&path).ok()?;
    let mut parts = content.split_whitespace();
    let vms_pages: u64 = parts.next()?.parse().ok()?;
    let page_sz = page_size() as u64;
    Some(vms_pages * page_sz)
}

pub(super) fn get_cmdline(pid: u32) -> Option<String> {
    let path = format!("/proc/{}/cmdline", pid);
    fs::read_to_string(&path).ok().and_then(|s| {
        let parts: Vec<&str> = s.split('\0').filter(|p| !p.is_empty()).collect();
        if parts.is_empty() {
            None
        } else {
            Some(parts.join(" "))
        }
    })
}

pub(super) fn count_fds(pid: u32) -> Option<usize> {
    let path = format!("/proc/{}/fd", pid);
    fs::read_dir(&path).ok().map(|entries| entries.count())
}

pub(super) fn get_cwd(pid: u32) -> Option<String> {
    let path = format!("/proc/{}/cwd", pid);
    fs::read_link(&path)
        .ok()
        .and_then(|p| p.to_str().map(|s| s.to_owned()))
}

pub(super) fn state_name(ch: char) -> String {
    match ch {
        'R' => crate::i18n::tr("Running"),
        'S' => crate::i18n::tr("Sleeping"),
        'D' => crate::i18n::tr("Uninterruptible Sleep"),
        'Z' => crate::i18n::tr("Zombie"),
        'T' => crate::i18n::tr("Stopped"),
        't' => crate::i18n::tr("Tracing Stop"),
        'X' | 'x' => crate::i18n::tr("Dead"),
        _ => crate::i18n::tr("Unknown"),
    }
}

// ─── Formatting helpers ─────────────────────────────────────────────────────

pub(super) fn format_duration(secs: f64) -> String {
    let total = secs as u64;
    let h = total / 3600;
    let m = (total % 3600) / 60;
    let s = total % 60;
    if h > 0 {
        format!("{:02}:{:02}:{:02}", h, m, s)
    } else {
        format!("{:02}:{:02}", m, s)
    }
}

pub(super) fn format_bytes(bytes: u64) -> String {
    const KB: u64 = 1_024;
    const MB: u64 = 1_024 * KB;
    const GB: u64 = 1_024 * MB;
    match bytes {
        b if b >= GB => format!("{:.2} GB", b as f64 / GB as f64),
        b if b >= MB => format!("{:.2} MB", b as f64 / MB as f64),
        b if b >= KB => format!("{:.2} KB", b as f64 / KB as f64),
        b => format!("{} B", b),
    }
}

pub(super) fn get_io_stats(pid: u32) -> Option<String> {
    let path = format!("/proc/{}/io", pid);
    let content = fs::read_to_string(&path).ok()?;
    let mut stats = Vec::new();
    for line in content.lines() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 2 {
            continue;
        }
        let key = parts[0];
        let value: u64 = parts[1].parse().ok()?;
        match key {
            "rchar:" => stats.push(("rchar", value, "chars read")),
            "wchar:" => stats.push(("wchar", value, "chars written")),
            "syscr:" => stats.push(("syscr", value, "read syscalls")),
            "syscw:" => stats.push(("syscw", value, "write syscalls")),
            "read_bytes:" => stats.push(("read_bytes", value, "bytes read")),
            "write_bytes:" => stats.push(("write_bytes", value, "bytes written")),
            "cancelled_write_bytes:" => {
                stats.push(("cancelled_write_bytes", value, "cancelled writes"))
            }
            _ => {}
        }
    }
    if stats.is_empty() {
        return None;
    }
    let lines: Vec<String> = stats
        .into_iter()
        .map(|(key, val, _)| {
            let disp = if key == "rchar"
                || key == "wchar"
                || key == "read_bytes"
                || key == "write_bytes"
                || key == "cancelled_write_bytes"
            {
                format_bytes(val)
            } else {
                val.to_string()
            };
            format!("{}: {}", key, disp)
        })
        .collect();
    Some(lines.join("  │  "))
}
