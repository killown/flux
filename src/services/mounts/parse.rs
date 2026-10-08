//! Pure parsers for `/proc/self/mountinfo` and `/etc/fstab` fields.

/// Decodes octal escapes (`\040` etc.) that the kernel and fstab use to
/// encode spaces and other special characters in path fields.
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

/// Extracts the mount-point field (5th whitespace-separated column) from
/// `/proc/self/mountinfo`.
pub fn parse_mountinfo(content: &str) -> Vec<String> {
    content
        .lines()
        .filter_map(|line| line.split_whitespace().nth(4))
        .map(unescape_mount_field)
        .collect()
}

/// Extracts mount points from `/etc/fstab`, skipping comments and blank lines.
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
