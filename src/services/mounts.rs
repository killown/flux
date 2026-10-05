use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;

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

fn resolve_luks_loop_name(dev_node: &str) -> Option<String> {
    let dev_path = Path::new(dev_node);
    let dev_name = dev_path.file_name()?.to_str()?;
    if !dev_name.starts_with("dm-") && !dev_name.starts_with("mapper/") {
        return None;
    }
    let sys_path = format!("/sys/block/{}/slaves", dev_name);
    if let Ok(entries) = fs::read_dir(sys_path) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with("loop") {
                let backing = format!("/sys/block/{}/loop/backing_file", name);
                if let Ok(backing_path) = fs::read_to_string(backing) {
                    let backing_path = backing_path.trim();
                    if let Some(file_name) = Path::new(backing_path).file_stem() {
                        if let Some(s) = file_name.to_str() {
                            return Some(s.to_string());
                        }
                    }
                }
            }
        }
    }
    None
}

fn get_fs_label(dev_node: &str) -> Option<String> {
    let output = Command::new("lsblk")
        .args(["-no", "LABEL", dev_node])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let label = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if label.is_empty() {
        None
    } else {
        Some(label)
    }
}

fn resolve_mount_name(path: &Path, dev_node: Option<&str>) -> String {
    let user_name = std::env::var("USER").unwrap_or_default();

    if let Some(dev) = dev_node {
        if let Some(luks) = resolve_luks_loop_name(dev) {
            return luks;
        }
        if let Some(label) = get_fs_label(dev) {
            return label;
        }
    }

    let components: Vec<&str> = path
        .components()
        .filter_map(|c| c.as_os_str().to_str())
        .collect();

    for &comp in components.iter().rev() {
        if !comp.eq_ignore_ascii_case("home") && !comp.eq_ignore_ascii_case(&user_name) {
            return comp.trim_start_matches('.').to_string();
        }
    }

    if let Some(dev) = dev_node {
        if let Some(base) = Path::new(dev).file_name().and_then(|n| n.to_str()) {
            return base.to_string();
        }
    }
    "Mount".to_string()
}

pub fn get_system_mounts() -> Vec<(String, PathBuf)> {
    let mut mounts = Vec::new();
    let home_dir = dirs::home_dir().unwrap_or_default();

    let mut media_roots = vec![PathBuf::from("/media")];
    if let Ok(user) = std::env::var("USER") {
        media_roots.push(PathBuf::from(format!("/run/media/{}", user)));
    }
    if let Ok(entries) = fs::read_dir("/run/media") {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_dir() && !media_roots.contains(&p) {
                media_roots.push(p);
            }
        }
    }

    for root in media_roots {
        if let Ok(entries) = fs::read_dir(&root) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() && path != home_dir {
                    let name = resolve_mount_name(&path, None);
                    if !mounts.iter().any(|(_, p)| p == &path) {
                        mounts.push((name, path));
                    }
                }
            }
        }
    }

    if let Ok(content) = fs::read_to_string("/proc/self/mounts") {
        for line in content.lines() {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() < 3 {
                continue;
            }
            let dev_node = parts[0];
            let path_str = parts[1];
            let fs_type = parts[2];
            let path = PathBuf::from(path_str);

            if path_str == "/"
                || path_str == "/home"
                || path_str == "/var/home"
                || path == home_dir
                || (path_str.starts_with("/home/") && path.components().count() <= 3)
                || path_str.starts_with("/app")
                || path_str.starts_with("/run/flatpak")
            {
                continue;
            }

            let is_mnt = path_str.starts_with("/mnt/") && path_str != "/mnt";
            let is_user_fuse =
                fs_type.contains("fuse") && path.starts_with(&home_dir) && path != home_dir;

            if is_mnt || is_user_fuse {
                let name = resolve_mount_name(&path, Some(dev_node));
                if !mounts.iter().any(|(_, p)| p == &path) {
                    mounts.push((name, path));
                }
            }
        }
    }

    for (uri, name, _icon) in crate::services::network::active_mounts() {
        let path = PathBuf::from(uri);
        if path.is_absolute() && !mounts.iter().any(|(_, p)| p == &path) {
            mounts.push((name, path));
        }
    }

    mounts
}
