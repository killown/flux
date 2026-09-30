use std::path::Path;

const VOLATILE_ROOTS: &[&str] = &[
    "/media",
    "/run/media",
    "/var/run/media",
    "/mnt",
    "/net",
    "/run/user",
];

const ALWAYS_INCONCLUSIVE_ROOTS: &[&str] = &["/run/user"];

pub fn unescape_mount_field(field: &str) -> String {
    let bytes = field.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'\\'
            && i + 3 < bytes.len()
            && bytes[i + 1..i + 4]
                .iter()
                .all(|b| (b'0'..=b'7').contains(b))
        {
            let v = (bytes[i + 1] - b'0') as u32 * 64
                + (bytes[i + 2] - b'0') as u32 * 8
                + (bytes[i + 3] - b'0') as u32;
            if v <= 0xFF {
                out.push(v as u8);
                i += 4;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

pub fn parse_mountinfo(content: &str) -> Vec<String> {
    content
        .lines()
        .filter_map(|line| line.split_whitespace().nth(4))
        .map(unescape_mount_field)
        .collect()
}

pub fn parse_fstab(content: &str) -> Vec<String> {
    content
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .filter_map(|l| l.split_whitespace().nth(1))
        .filter(|mp| mp.starts_with('/'))
        .map(unescape_mount_field)
        .collect()
}

#[derive(Debug, Default, Clone)]
pub struct MountTable {
    mounted: Vec<String>,
    fstab: Vec<String>,
}

impl MountTable {
    #[allow(dead_code)]
    pub fn from_parts(mounted: Vec<String>, fstab: Vec<String>) -> Self {
        Self { mounted, fstab }
    }

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
