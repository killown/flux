//! Concurrency clamp based on the process's file-descriptor budget.

/// Clamp the thumbnail worker count so it can't eat the whole FD budget.
///
/// Each thumbnail holds a couple of fds while it runs, and on a folder with
/// thousands of images that adds up fast. Give thumbnails a quarter of
/// RLIMIT_NOFILE and leave the rest for GTK, GIO, SQLite and the terminal.
pub(super) fn effective_thumbnail_permits(configured: usize) -> usize {
    let mut lim = libc::rlimit {
        rlim_cur: 0,
        rlim_max: 0,
    };
    let soft = unsafe {
        if libc::getrlimit(libc::RLIMIT_NOFILE, &mut lim) == 0 {
            lim.rlim_cur as usize
        } else {
            1024
        }
    };

    // Below 2 the pipeline just serializes for no reason.
    let budget = (soft / 4).max(2);
    configured.max(1).min(budget)
}
