use std::fs::Metadata;
use std::path::PathBuf;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::OnceLock;

/// Unix timestamp of "now", resolved once per load batch by the loader.
///
/// `FileLoadContext::labels()` reads this instead of calling
/// `LocalTime::now()` per item, which would be O(N) `localtime_r`
/// calls on a large directory. The loader stores a fresh value before
/// priming `labels()` (see `loader/directory.rs`, `loader/archive.rs`,
/// `loader/streaming.rs`, `search/content.rs`).
///
/// `0` means "not set", `compute_labels` falls back to a per-call
/// `LocalTime::now()` in that case (tests, direct callers).
pub static NOW_UNIX: AtomicI64 = AtomicI64::new(0);

/// Returns the shared "now" timestamp, or 0 if not primed.
#[inline]
pub fn now_unix() -> i64 {
    NOW_UNIX.load(Ordering::Relaxed)
}

/// Stores the shared "now" timestamp. Called once per load batch.
#[inline]
pub fn set_now_unix(unix: i64) {
    NOW_UNIX.store(unix, Ordering::Relaxed);
}

/// Pre-formatted strings for the grid/list cell labels.
///
/// Computed once at load time (in the Rayon pool), consumed by
/// `FileItem::bind_widgets` on the GTK main thread. Zero allocation and
/// zero syscalls are required to render a cell once this is filled in.
///
/// See `src/ui/components/bind.rs`, the "Contract" doc comment at the top
/// of `bind_widgets` forbids formatting or I/O on the GTK thread.
#[derive(Debug, Clone, Default)]
pub struct CellLabels {
    /// Human-readable size: `"1.4 MB"`, `"12 items"`, `""` for empty.
    #[allow(dead_code)]
    pub size: String,
    /// Short date/time: `"14:32"`, `"Yesterday"`, `"Mar 3"`, `"2023"`.
    #[allow(dead_code)]
    pub mtime_short: String,
    /// Full timestamp for tooltip: `"03/15/24 14:32:11"`.
    pub mtime_tooltip: String,
    /// Combined `size · mtime_short` string for `.flux-list-info`.
    pub info_combined: String,
}

impl CellLabels {
    /// Returns true if the info label should be visible (has any content).
    #[inline]
    pub fn has_info(&self) -> bool {
        !self.info_combined.is_empty()
    }
}

/// Internal metadata container for parallel directory processing.
#[derive(Debug, Clone)]
pub struct FileLoadContext {
    /// The sanitized name used for UI rendering and label display.
    pub display_name: String,
    /// The absolute filesystem path to the target file or directory.
    pub target_path: PathBuf,
    /// True if the item is a directory, false if it is a file or symlink.
    pub is_dir: bool,
    /// True when a browsable archive is drawn as a folder (`ui.archives_as_folders`).
    ///
    /// Visual folder, semantic file: `is_dir` stays `false`, so drag and drop,
    /// delete, terminal "Open Here" and `directory`-scoped menu actions treat the
    /// item as the file it is. Only the icon, label and badge change.
    pub is_virtual_dir: bool,
    /// Pre-processed string used for case-insensitive and natural sorting.
    pub sort_name: String,
    /// Pre-computed lowercase file extension for zero-alloc Type sorting.
    pub sort_ext: String,
    /// The path to a cached or generated image representing the file content.
    pub thumbnail_path: Option<PathBuf>,
    /// Whether the filename label should wrap across multiple lines.
    pub expand_labels: bool,
    /// Optional override icon name for the item.
    pub custom_icon: Option<String>,

    /// True if the target item is a symbolic link.
    pub is_symlink: bool,
    /// True if the symbolic link points to a non-existent target.
    pub is_broken_symlink: bool,
    /// Encrypted archive, per the ZIP local header or what this session has learned.
    pub is_encrypted_archive: bool,
    /// A password for this archive is in the session cache. Hint only.
    pub has_cached_password: bool,

    // Lazy metadata caches
    /// Cached emptiness flag determined lazily without consuming entire directory streams.
    is_empty: OnceLock<bool>,
    metadata: OnceLock<Option<Metadata>>,
    size: OnceLock<u64>,
    mtime: OnceLock<i64>,
    is_foreign_owner: OnceLock<bool>,
    /// Pre-formatted cell labels, resolved lazily from `size()` + `mtime()`.
    labels: OnceLock<CellLabels>,
}

impl FileLoadContext {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        display_name: String,
        target_path: PathBuf,
        is_dir: bool,
        sort_name: String,
        sort_ext: String,
        thumbnail_path: Option<PathBuf>,
        expand_labels: bool,
        custom_icon: Option<String>,
        is_symlink: bool,
        is_broken_symlink: bool,
    ) -> Self {
        Self {
            display_name,
            target_path,
            is_dir,
            is_virtual_dir: false,
            sort_name,
            sort_ext,
            thumbnail_path,
            expand_labels,
            custom_icon,
            is_symlink,
            is_broken_symlink,
            is_encrypted_archive: false,
            has_cached_password: false,
            is_empty: OnceLock::new(),
            metadata: OnceLock::new(),
            size: OnceLock::new(),
            mtime: OnceLock::new(),
            is_foreign_owner: OnceLock::new(),
            labels: OnceLock::new(),
        }
    }

    /// Resolves and caches file metadata on demand.
    #[inline]
    pub fn metadata(&self) -> Option<&Metadata> {
        self.metadata
            .get_or_init(|| {
                if self.is_symlink {
                    std::fs::metadata(&self.target_path)
                        .ok()
                        .or_else(|| self.target_path.symlink_metadata().ok())
                } else {
                    self.target_path.symlink_metadata().ok()
                }
            })
            .as_ref()
    }

    /// Pre-seeds size, modification time, and ownership for virtual/archive entries.
    #[allow(clippy::too_many_arguments)]
    pub fn with_stats(
        display_name: String,
        target_path: PathBuf,
        is_dir: bool,
        sort_name: String,
        sort_ext: String,
        size: u64,
        mtime: i64,
        thumbnail_path: Option<PathBuf>,
        expand_labels: bool,
        custom_icon: Option<String>,
    ) -> Self {
        Self {
            display_name,
            target_path,
            is_dir,
            is_virtual_dir: false,
            sort_name,
            sort_ext,
            thumbnail_path,
            expand_labels,
            custom_icon,
            is_symlink: false,
            is_broken_symlink: false,
            is_encrypted_archive: false,
            has_cached_password: false,
            is_empty: OnceLock::from(size == 0),
            metadata: OnceLock::new(),
            size: OnceLock::from(size),
            mtime: OnceLock::from(mtime),
            is_foreign_owner: OnceLock::from(false),
            labels: OnceLock::new(),
        }
    }

    /// True when the item is drawn and sorted together with the folders.
    ///
    /// Real directories and virtual-dir archives (`ui.archives_as_folders`)
    /// both qualify. This is a presentation helper for sorting only: it must
    /// never replace `is_dir` in drag and drop, delete, terminal "Open Here"
    /// or `directory`-scoped menu actions, because a virtual dir is still a file.
    #[inline]
    pub fn is_folder_like(&self) -> bool {
        self.is_dir || self.is_virtual_dir
    }

    /// Orders two items by folder grouping alone.
    ///
    /// Returns `Equal` when both sit in the same group, so the caller falls
    /// through to its Name / Size / Date / Type comparison and its ascending or
    /// descending direction. With `folders_first` the folder group leads,
    /// otherwise it trails. Items with `is_virtual_dir == false` group exactly
    /// as the old `is_dir` comparison did.
    #[inline]
    pub fn cmp_folder_group(&self, other: &Self, folders_first: bool) -> std::cmp::Ordering {
        let (a, b) = (self.is_folder_like(), other.is_folder_like());
        if folders_first {
            b.cmp(&a)
        } else {
            a.cmp(&b)
        }
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        *self.is_empty.get_or_init(|| {
            if self.is_dir {
                std::fs::read_dir(&self.target_path)
                    .map(|mut rd| rd.next().is_none())
                    .unwrap_or(false)
            } else {
                self.size() == 0
            }
        })
    }

    /// Resolves size lazily on first access.
    #[inline]
    pub fn size(&self) -> u64 {
        *self.size.get_or_init(|| {
            if self.is_dir {
                std::fs::read_dir(&self.target_path)
                    .map(|rd| rd.count())
                    .unwrap_or(0) as u64
            } else {
                self.metadata().map(|m| m.len()).unwrap_or(0)
            }
        })
    }

    /// Resolves modification time lazily on first access.
    #[inline]
    pub fn mtime(&self) -> i64 {
        *self.mtime.get_or_init(|| {
            self.metadata()
                .and_then(|m| m.modified().ok())
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs() as i64)
                .unwrap_or(0)
        })
    }

    /// Resolves foreign owner flag lazily on first access.
    #[inline]
    pub fn is_foreign_owner(&self, current_uid: u32) -> bool {
        *self.is_foreign_owner.get_or_init(|| {
            use std::os::unix::fs::MetadataExt;
            self.metadata()
                .map(|m| m.uid() != current_uid)
                .unwrap_or(false)
        })
    }

    // ── Pre-formatted cell labels ───────────────────────────────────────────

    /// Returns the pre-formatted cell labels, computing them on first access.
    ///
    /// **Contract:** callers on the GTK main thread (i.e. `FileItem::bind_widgets`)
    /// MUST treat this as a read-only cache. The expensive work (date formatting,
    /// size formatting) happens on the *first* call - which should be primed
    /// from the Rayon pool in the loader, alongside `size()` / `mtime()`.
    ///
    /// See `loader/directory.rs`, `loader/archive.rs`, `loader/streaming.rs`,
    /// and `search/content.rs` for the priming sites.
    #[inline]
    pub fn labels(&self) -> &CellLabels {
        crate::hit!("FileLoadContext::labels");
        self.labels.get_or_init(|| {
            let now = now_unix();
            self.compute_labels(now)
        })
    }

    /// Computes the label strings from `size()` and `mtime()`.
    ///
    /// `now_unix` is the shared "now" timestamp set once per load batch. If
    /// `0`, a fresh `LocalTime::now()` is resolved (test / direct
    /// caller fallback).
    ///
    /// Runs exactly once per item, do not call directly on the GTK thread -
    /// call `labels()` instead, which primes this in the loader.
    fn compute_labels(&self, now_unix: i64) -> CellLabels {
        crate::hit!("compute_labels");
        use crate::i18n::tr;

        let size = self.size();
        let mtime = self.mtime();

        let size_label = if self.is_dir {
            if size == 1 {
                "1 item".to_string()
            } else {
                format!("{} {}", size, tr("items"))
            }
        } else if size > 0 {
            format_size(size)
        } else {
            String::new()
        };

        let mut mtime_short = String::new();
        let mut mtime_tooltip = String::new();

        if mtime > 0 {
            // Resolve `now` once per batch when primed, fall back per-call
            // for tests / direct callers. The fallback is the only path that
            // calls `localtime_r`, and it does so at most once per item.
            let now_dt_owned = if now_unix > 0 {
                LocalTime::from_unix(now_unix)
            } else {
                LocalTime::now()
            };

            if let (Some(file_dt), Some(now_dt)) = (LocalTime::from_unix(mtime), now_dt_owned) {
                let (f_y, f_m, f_d) = (file_dt.year(), file_dt.month(), file_dt.day());
                let (n_y, n_m, n_d) = (now_dt.year(), now_dt.month(), now_dt.day());

                let time_str = file_dt.format("%H:%M").unwrap_or_default();

                if let Some(exact) = file_dt.format("%x %X") {
                    mtime_tooltip = exact;
                }

                if f_y == n_y && f_m == n_m && f_d == n_d {
                    mtime_short = format!("{}, {}", tr("Today"), time_str);
                } else {
                    // "Yesterday" check without `add_days(-1)`: subtract one
                    // day from `now_dt`'s Y/M/D fields directly. This avoids
                    // the per-item DST recalculation that `add_days` performs.
                    //
                    // DST edge cases (2 AM on a spring-forward day) are
                    // irrelevant for the "Yesterday" label - the worst outcome
                    // is mis-classifying a file from 23:00–23:59 as "today"
                    // or "yesterday", which is cosmetically fine.
                    let yesterday = prev_calendar_day(n_y, n_m, n_d);
                    let is_yesterday = (f_y, f_m, f_d) == yesterday;

                    if is_yesterday {
                        mtime_short = format!("{}, {}", tr("Yesterday"), time_str);
                    } else if let Some(formatted) = file_dt.format("%x %H:%M") {
                        mtime_short = formatted;
                    }
                }
            }
        }

        let info_combined = match (size_label.is_empty(), mtime_short.is_empty()) {
            (false, false) => format!("{} · {}", size_label, mtime_short),
            (false, true) => size_label.clone(),
            (true, false) => mtime_short.clone(),
            (true, true) => String::new(),
        };

        CellLabels {
            size: size_label,
            mtime_short,
            mtime_tooltip,
            info_combined,
        }
    }
}

/// A broken-down local time, resolved with the C library so the labels follow
/// the process locale and time zone (`%x` and `%X` are locale formats).
struct LocalTime {
    tm: libc::tm,
}

impl LocalTime {
    /// Local time for a unix timestamp, `None` when it is out of range.
    fn from_unix(unix: i64) -> Option<Self> {
        let t = libc::time_t::try_from(unix).ok()?;
        // SAFETY: `tm` is plain data and `localtime_r` fully initialises it when it
        // returns non-null. `t` outlives the call.
        let mut tm: libc::tm = unsafe { std::mem::zeroed() };
        let resolved = unsafe { !libc::localtime_r(&t, &mut tm).is_null() };
        resolved.then_some(Self { tm })
    }

    /// Local time right now.
    fn now() -> Option<Self> {
        let secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .ok()?
            .as_secs();
        Self::from_unix(i64::try_from(secs).ok()?)
    }

    fn year(&self) -> i32 {
        self.tm.tm_year + 1900
    }

    /// Month of the year, `1..=12`.
    fn month(&self) -> i32 {
        self.tm.tm_mon + 1
    }

    /// Day of the month, `1..=31`.
    fn day(&self) -> i32 {
        self.tm.tm_mday
    }

    /// Formats with `strftime` in the current locale, `None` when it does not fit.
    fn format(&self, fmt: &str) -> Option<String> {
        let fmt = std::ffi::CString::new(fmt).ok()?;
        let mut buf = [0u8; 128];
        // SAFETY: `buf` is valid for `buf.len()` bytes, `fmt` is NUL terminated and
        // `self.tm` was initialised by `localtime_r`.
        let n = unsafe {
            libc::strftime(
                buf.as_mut_ptr().cast::<libc::c_char>(),
                buf.len(),
                fmt.as_ptr(),
                &self.tm,
            )
        };
        (n > 0).then(|| String::from_utf8_lossy(&buf[..n]).into_owned())
    }
}

/// Returns `(year, month, day)` for the calendar day before `(y, m, d)`.
///
/// Pure field arithmetic - no `LocalTime` resolution, no DST, no
/// `localtime_r`. Called once per item that isn't "Today", so it must be
/// cheap.
#[inline]
fn prev_calendar_day(y: i32, m: i32, d: i32) -> (i32, i32, i32) {
    if d > 1 {
        return (y, m, d - 1);
    }
    // Last day of the previous month.
    let (py, pm) = if m > 1 { (y, m - 1) } else { (y - 1, 12) };
    let last = days_in_month(py, pm);
    (py, pm, last)
}

/// Number of days in `(year, month)`.
#[inline]
fn days_in_month(y: i32, m: i32) -> i32 {
    match m {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            if (y % 4 == 0 && y % 100 != 0) || y % 400 == 0 {
                29
            } else {
                28
            }
        }
        _ => 30,
    }
}

/// Formats a byte count into a human-readable string (B / KB / MB / GB).
///
/// Matches the formatting used by `FileItem::bind_widgets` previously:
/// `{} B`, `{:.0} KB`, `{:.1} MB`, `{:.1} GB`.
fn format_size(bytes: u64) -> String {
    const KB: u64 = 1_024;
    const MB: u64 = 1_024 * KB;
    const GB: u64 = 1_024 * MB;
    match bytes {
        b if b >= GB => format!("{:.1} GB", b as f64 / GB as f64),
        b if b >= MB => format!("{:.1} MB", b as f64 / MB as f64),
        b if b >= KB => format!("{:.0} KB", b as f64 / KB as f64),
        b => format!("{b} B"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cmp::Ordering as Ord3;

    /// Builds an item, then sets `is_virtual_dir` after construction because
    /// `FileLoadContext::new` does not take it.
    fn item(name: &str, is_dir: bool, is_virtual_dir: bool) -> FileLoadContext {
        let mut it = FileLoadContext::new(
            name.to_string(),
            PathBuf::from(format!("/t/{name}")),
            is_dir,
            name.to_lowercase(),
            String::new(),
            None,
            false,
            None,
            false,
            false,
        );
        it.is_virtual_dir = is_virtual_dir;
        it
    }

    /// Same shape as the loader comparators: group first, then Name, with the
    /// direction flipped after the group check.
    fn sorted(mut items: Vec<FileLoadContext>, folders_first: bool, asc: bool) -> Vec<String> {
        items.sort_by(|a, b| {
            let group = a.cmp_folder_group(b, folders_first);
            if group != Ord3::Equal {
                return group;
            }
            let by_name = a.sort_name.cmp(&b.sort_name);
            if asc {
                by_name
            } else {
                by_name.reverse()
            }
        });
        items.into_iter().map(|i| i.display_name).collect()
    }

    /// `archives_as_folders` on: the zip carries `is_virtual_dir`.
    fn mixed_on() -> Vec<FileLoadContext> {
        vec![
            item("z.txt", false, false),
            item("n.zip", false, true),
            item("a.txt", false, false),
            item("m_dir", true, false),
        ]
    }

    /// `archives_as_folders` off: the loader never sets the flag.
    fn mixed_off() -> Vec<FileLoadContext> {
        vec![
            item("z.txt", false, false),
            item("n.zip", false, false),
            item("a.txt", false, false),
            item("m_dir", true, false),
        ]
    }

    #[test]
    fn folder_like_covers_dirs_and_virtual_dirs_but_not_files() {
        assert!(item("d", true, false).is_folder_like());
        assert!(item("a.zip", false, true).is_folder_like());
        assert!(!item("f", false, false).is_folder_like());
    }

    #[test]
    fn virtual_dir_stays_a_semantic_file() {
        let zip = item("a.zip", false, true);
        assert!(!zip.is_dir);
        assert!(zip.is_virtual_dir);
    }

    #[test]
    fn option_on_folders_first_groups_zip_with_folder() {
        assert_eq!(
            sorted(mixed_on(), true, true),
            ["m_dir", "n.zip", "a.txt", "z.txt"]
        );
    }

    #[test]
    fn option_on_folders_last_groups_zip_with_folder_at_the_end() {
        assert_eq!(
            sorted(mixed_on(), false, true),
            ["a.txt", "z.txt", "m_dir", "n.zip"]
        );
    }

    #[test]
    fn option_off_zip_sorts_with_files() {
        assert_eq!(
            sorted(mixed_off(), true, true),
            ["m_dir", "a.txt", "n.zip", "z.txt"]
        );
        assert_eq!(
            sorted(mixed_off(), false, true),
            ["a.txt", "n.zip", "z.txt", "m_dir"]
        );
    }

    #[test]
    fn descending_reverses_inside_each_group_only() {
        assert_eq!(
            sorted(mixed_on(), true, false),
            ["n.zip", "m_dir", "z.txt", "a.txt"]
        );
        assert_eq!(
            sorted(mixed_on(), false, false),
            ["z.txt", "a.txt", "n.zip", "m_dir"]
        );
    }

    #[test]
    fn without_virtual_dirs_grouping_matches_the_old_is_dir_rule() {
        for folders_first in [true, false] {
            for a_dir in [true, false] {
                for b_dir in [true, false] {
                    let a = item("a", a_dir, false);
                    let b = item("b", b_dir, false);
                    let old = if a.is_dir != b.is_dir {
                        if folders_first {
                            b.is_dir.cmp(&a.is_dir)
                        } else {
                            a.is_dir.cmp(&b.is_dir)
                        }
                    } else {
                        Ord3::Equal
                    };
                    assert_eq!(a.cmp_folder_group(&b, folders_first), old);
                }
            }
        }
    }
}
