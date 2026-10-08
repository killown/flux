pub(super) fn fmt_kb(kb: u64) -> String {
    if kb >= 1_048_576 {
        format!("{:.1} GB", kb as f64 / 1_048_576.0)
    } else if kb >= 1_024 {
        format!("{:.1} MB", kb as f64 / 1_024.0)
    } else {
        format!("{} KB", kb)
    }
}

pub(super) fn fmt_bytes(total: u64) -> String {
    fmt_kb(total / 1024)
}
