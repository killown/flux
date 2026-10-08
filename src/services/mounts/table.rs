use super::parse::{parse_fstab, parse_mountinfo};
use std::path::Path;

/// Mount points that only appear when a filesystem is mounted, so a missing
/// path under them can never be treated as "definitely gone".
const VOLATILE_ROOTS: &[&str] = &[
    "/media",
    "/run/media",
    "/var/run/media",
    "/mnt",
    "/net",
    "/run/user",
];

/// Volatile roots where we never have enough info to conclude anything.
const ALWAYS_INCONCLUSIVE_ROOTS: &[&str] = &["/run/user"];

#[derive(Debug, Default, Clone)]
pub struct MountTable {
    mounted: Vec<String>,
    fstab: Vec<String>,
}

impl MountTable {
    /// Builds a table from pre-parsed parts. Mostly for tests.
    #[allow(dead_code)]
    pub fn from_parts(mounted: Vec<String>, fstab: Vec<String>) -> Self {
        Self { mounted, fstab }
    }

    /// Reads `/proc/self/mountinfo` and `/etc/fstab`.
    pub fn load() -> Self {
        let mounted = std::fs::read_to_string("/proc/self/mountinfo")
            .map(|c| parse_mountinfo(&c))
            .unwrap_or_default();
        let fstab = std::fs::read_to_string("/etc/fstab")
            .map(|c| parse_fstab(&c))
            .unwrap_or_default();
        Self { mounted, fstab }
    }

    fn is_mounted(&self, mount_point: &str) -> bool {
        self.mounted.iter().any(|m| m == mount_point)
    }

    fn deepest_mount_for(&self, path: &Path) -> Option<&str> {
        self.mounted
            .iter()
            .filter(|m| path.starts_with(m.as_str()))
            .max_by_key(|m| m.len())
            .map(String::as_str)
    }

    /// Returns `true` when the state of `path` cannot be decided from the
    /// current mount table - a missing directory under such a path is
    /// considered "possibly just unmounted" rather than "deleted".
    pub fn is_inconclusive(&self, path: &Path) -> bool {
        for root in ALWAYS_INCONCLUSIVE_ROOTS {
            if path.starts_with(root) {
                return true;
            }
        }
        for root in VOLATILE_ROOTS {
            if path.starts_with(root) {
                let covered = self
                    .deepest_mount_for(path)
                    .map(|m| m.len() > root.len() && Path::new(m).starts_with(root))
                    .unwrap_or(false);
                if !covered {
                    return true;
                }
            }
        }

        self.fstab
            .iter()
            .any(|f| f != "/" && path.starts_with(f.as_str()) && !self.is_mounted(f))
    }
}

/// Returns `true` when `path` is missing on disk *and* the mount table gives
/// us enough information to conclude it was actually deleted (rather than
/// hidden behind an unmounted filesystem).
pub fn is_confirmed_missing(path: &Path, mounts: &MountTable) -> bool {
    match path.symlink_metadata() {
        Ok(_) => false,
        Err(e) => {
            let gone =
                e.kind() == std::io::ErrorKind::NotFound || e.raw_os_error() == Some(libc::ENOTDIR);
            gone && !mounts.is_inconclusive(path)
        }
    }
}
