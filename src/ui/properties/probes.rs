use crate::i18n::tr;
use std::fs;
use std::io::Read;
use std::path::Path;
use std::process::Command;

/// Formats a byte count as a compact human-readable string (`1.50 MB`).
pub(super) fn format_size(size: u64) -> String {
    if size == 0 {
        return "0B".to_string();
    }
    let units = ["B", "KB", "MB", "GB", "TB", "PB"];
    let i = (size as f64).log(1024.0).floor() as usize;
    let i = i.min(units.len() - 1);
    let s = size as f64 / 1024.0f64.powi(i as i32);
    format!("{:.2} {}", s, units[i])
}

/// Shannon entropy of `data`, in bits per byte, rounded to four decimals.
pub(super) fn get_shannon_entropy(data: &[u8]) -> f64 {
    if data.is_empty() {
        return 0.0;
    }
    let mut counts = [0usize; 256];
    for &b in data {
        counts[b as usize] += 1;
    }
    let len = data.len() as f64;
    let mut entropy = 0.0;
    for &count in &counts {
        if count > 0 {
            let p = count as f64 / len;
            entropy -= p * p.log2();
        }
    }
    (entropy * 10000.0).round() / 10000.0
}

/// Runs `git log -1` for `path` and returns `(label, value)` rows.
pub(super) fn get_git_info(path: &Path) -> Option<Vec<(String, String)>> {
    let output = Command::new("git")
        .args([
            "log",
            "-1",
            "--format=%h|%ai|%s",
            "--",
            &path.to_string_lossy(),
        ])
        .output()
        .ok()?;

    let out_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if out_str.is_empty() {
        return None;
    }

    let parts: Vec<&str> = out_str.split('|').collect();
    if parts.len() >= 3 {
        Some(vec![
            (tr("Hash"), parts[0].to_string()),
            (tr("Date"), parts[1].to_string()),
            (tr("Subject"), parts[2].to_string()),
        ])
    } else {
        None
    }
}

/// Reads the first 20 bytes of `path` and decodes the ELF header if present.
pub(super) fn get_elf_info(path: &Path) -> Option<Vec<(String, String)>> {
    let mut f = fs::File::open(path).ok()?;
    let mut buffer = [0u8; 20];
    if f.read_exact(&mut buffer).is_err() {
        return None;
    }

    if &buffer[0..4] != b"\x7fELF" {
        return None;
    }

    let is_little = buffer[5] == 1;
    let endian = if is_little { "Little" } else { "Big" };

    let read_u16 = |idx: usize| -> u16 {
        if is_little {
            u16::from_le_bytes([buffer[idx], buffer[idx + 1]])
        } else {
            u16::from_be_bytes([buffer[idx], buffer[idx + 1]])
        }
    };

    let e_type = read_u16(16);
    let e_machine = read_u16(18);

    let type_str = match e_type {
        1 => "Relocatable",
        2 => "Executable",
        3 => "Shared",
        4 => "Core",
        _ => "Unknown",
    };

    let mach_str = match e_machine {
        0x3E => "x86_64",
        0x03 => "x86",
        0x28 => "ARM",
        0xB7 => "AArch64",
        _ => "Unknown",
    };

    Some(vec![
        (tr("Type"), type_str.to_string()),
        (tr("Arch"), mach_str.to_string()),
        (tr("Endian"), endian.to_string()),
    ])
}

/// Counts lines, words, TODOs, and detects CRLF vs LF endings in a UTF-8 body.
pub(super) fn get_text_metrics(raw: &[u8]) -> Option<Vec<(String, String)>> {
    if let Ok(content) = std::str::from_utf8(raw) {
        let lines: Vec<&str> = content.lines().collect();
        let word_count: usize = lines.iter().map(|l| l.split_whitespace().count()).sum();
        let todo_count = lines
            .iter()
            .filter(|l| l.to_uppercase().contains("TODO"))
            .count();
        let line_endings = if content.contains("\r\n") {
            "CRLF"
        } else {
            "LF"
        };

        Some(vec![
            (tr("Line Count"), lines.len().to_string()),
            (tr("Word Count"), word_count.to_string()),
            (tr("TODOs"), todo_count.to_string()),
            (tr("Line Endings"), line_endings.to_string()),
        ])
    } else {
        None
    }
}
