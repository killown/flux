use super::probes::{
    format_size, get_elf_info, get_git_info, get_shannon_entropy, get_text_metrics,
};
use crate::i18n::tr;
use crate::utils;
use chrono::{DateTime, Local};
use sha2::{Digest, Sha256};
use std::fs;
use std::os::unix::fs::MetadataExt;
use std::path::Path;

/// A section title plus its `(label, value)` rows.
pub(super) type SectionData = (String, Vec<(String, String)>);

/// Assembles every metadata section for `path`.
///
/// `mime_type` is pre-computed by the caller (it's used elsewhere too), and
/// `file_content` is the first 1 MiB of the file (or empty if unreadable).
pub(super) fn build_sections(
    path: &Path,
    mime_type: &str,
    file_content: &[u8],
) -> Vec<SectionData> {
    let filename = path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();

    let mut sections_data: Vec<SectionData> = Vec::new();

    sections_data.push((
        tr("File Identity"),
        vec![
            (tr("Filename"), filename),
            (
                tr("Extension"),
                path.extension()
                    .map(|e| e.to_string_lossy().to_string())
                    .unwrap_or(tr("None")),
            ),
            (tr("MIME Type"), mime_type.to_string()),
        ],
    ));

    // ── Symlink ─────────────────────────────────────────────────────────────
    if path.is_symlink() {
        if let Ok(target) = fs::read_link(path) {
            sections_data.push((
                tr("Symlink Info"),
                vec![
                    (tr("Target"), target.to_string_lossy().to_string()),
                    (tr("Broken"), (!target.exists()).to_string()),
                    (
                        tr("Canonical"),
                        path.canonicalize()
                            .unwrap_or_default()
                            .to_string_lossy()
                            .to_string(),
                    ),
                ],
            ));
        }
    }

    // ── Extended attributes ────────────────────────────────────────────────
    if let Ok(attrs) = xattr::list(path) {
        let mut xattrs_list = Vec::new();
        for attr in attrs {
            if let Ok(Some(val)) = xattr::get(path, &attr) {
                xattrs_list.push((
                    attr.to_string_lossy().to_string(),
                    String::from_utf8_lossy(&val).to_string(),
                ));
            }
        }
        if !xattrs_list.is_empty() {
            sections_data.push((tr("Extended Attributes"), xattrs_list));
        }
    }

    // ── System & disk, temporal, security ──────────────────────────────────
    if let Ok(meta) = path.metadata() {
        let mut system_disk = vec![
            (tr("Full Path"), path.to_string_lossy().to_string()),
            (
                tr("Size"),
                format!("{} ({} B)", format_size(meta.len()), meta.len()),
            ),
            (tr("Disk Usage"), format_size(meta.blocks() * 512)),
            (tr("Inode"), meta.ino().to_string()),
            (tr("Device ID"), meta.dev().to_string()),
        ];

        if let Ok(mounts) = fs::read_to_string("/proc/self/mounts") {
            let mount_info = mounts
                .lines()
                .rfind(|l| path.starts_with(l.split_whitespace().nth(1).unwrap_or("")));
            if let Some(line) = mount_info {
                let parts: Vec<&str> = line.split_whitespace().collect();
                system_disk.push((tr("Mount Source"), parts[0].to_string()));
                system_disk.push((tr("FS Type"), parts[2].to_string()));
            }
        }
        sections_data.push((tr("System & Disk"), system_disk));

        let fmt_time = |t: std::time::SystemTime| -> String {
            let dt: DateTime<Local> = t.into();
            dt.format("%Y-%m-%d %H:%M:%S").to_string()
        };

        sections_data.push((
            tr("Temporal"),
            vec![
                (
                    tr("Created"),
                    meta.created().map(fmt_time).unwrap_or_default(),
                ),
                (
                    tr("Modified"),
                    meta.modified().map(fmt_time).unwrap_or_default(),
                ),
                (
                    tr("Accessed"),
                    meta.accessed().map(fmt_time).unwrap_or_default(),
                ),
            ],
        ));

        let mut security = vec![
            (tr("Permissions"), format!("{:03o}", meta.mode() & 0o777)),
            (tr("UID/GID"), format!("{}:{}", meta.uid(), meta.gid())),
        ];
        if !file_content.is_empty() {
            security.push((
                tr("Entropy"),
                format!("{:.4}", get_shannon_entropy(file_content)),
            ));
            let hash_hex = Sha256::digest(file_content)
                .iter()
                .map(|b| format!("{:02x}", b))
                .collect::<String>();

            sections_data.push((tr("Security"), vec![(tr("SHA256"), hash_hex)]));
        }
        sections_data.push((tr("Security"), security));
    }

    // ── ELF analysis ────────────────────────────────────────────────────────
    if mime_type.contains("executable") || mime_type.contains("sharedlib") {
        if let Some(elf) = get_elf_info(path) {
            sections_data.push((tr("Executable Analysis"), elf));
        }
    }

    // ── Text content metrics ───────────────────────────────────────────────
    let is_text = mime_type.contains("text")
        || mime_type.contains("javascript")
        || mime_type.contains("json")
        || mime_type.contains("xml");
    let text_exts = [".py", ".rs", ".toml", ".yaml", ".sh", ".md", ".txt"];
    let has_text_ext = path
        .extension()
        .map(|e| text_exts.contains(&e.to_string_lossy().as_ref()))
        .unwrap_or(false);
    if (is_text || has_text_ext) && !file_content.is_empty() {
        if let Some(metrics) = get_text_metrics(file_content) {
            sections_data.push((tr("Content Metrics"), metrics));
        }
    }

    // ── Image dimensions ────────────────────────────────────────────────────
    if mime_type.starts_with("image/") {
        if let Ok(dim) = imagesize::size(path) {
            sections_data.push((
                tr("Visual Media"),
                vec![
                    (tr("Dimensions"), format!("{}x{}", dim.width, dim.height)),
                    (
                        tr("Aspect Ratio"),
                        format!("{:.2}:1", dim.width as f64 / dim.height as f64),
                    ),
                ],
            ));
        }
    }

    // ── Git history ─────────────────────────────────────────────────────────
    if let Some(git) = get_git_info(path) {
        sections_data.push((tr("Git History"), git));
    }

    let _ = utils::media::get_mime_type; // keep the `utils` import live for callers

    sections_data
}
