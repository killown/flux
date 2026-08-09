use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use tempfile::tempdir;

// ─── Helpers & Utilities ───────────────────────────────────────────────────

/// Replicates the tmp basename cleaning logic in paste_ops.rs:
/// Strips the `.tmpXyZ.` prefix that archive extraction occasionally adds.
fn clean_tmp_basename(name: &str) -> String {
    if name.starts_with(".tmp") {
        name.split_once('.')
            .and_then(|(_, rest)| rest.split_once('.'))
            .map(|(_, real)| real.to_string())
            .unwrap_or_else(|| name.to_string())
    } else {
        name.to_string()
    }
}

/// Replicates scan_total_bytes logic for pre-scanning copy sizes.
fn scan_total_bytes(path: &Path) -> u64 {
    if path.is_file() {
        return fs::metadata(path).map(|m| m.len()).unwrap_or(0);
    }
    if path.is_dir() {
        let mut total = 0u64;
        if let Ok(entries) = fs::read_dir(path) {
            for entry in entries.flatten() {
                total += scan_total_bytes(&entry.path());
            }
        }
        return total;
    }
    0
}

/// Checks if pasting `src` into `dest_dir` would cause a recursive infinite loop.
fn is_recursive_paste(src: &Path, dest_dir: &Path) -> bool {
    dest_dir.starts_with(src)
}

// ─── Tests ─────────────────────────────────────────────────────────────────

#[test]
fn test_clean_tmp_basename() {
    assert_eq!(clean_tmp_basename(".tmpAbCd.favicon.svg"), "favicon.svg");
    assert_eq!(clean_tmp_basename(".tmp123.notes.txt"), "notes.txt");
    assert_eq!(clean_tmp_basename("regular_file.png"), "regular_file.png");
    assert_eq!(clean_tmp_basename(".gitignore"), ".gitignore");
}

#[test]
fn test_copy_name_collision_formatting() {
    let original_name = "document.pdf".to_string();
    let target_dir = PathBuf::from("/tmp/destination");

    // Simulates the while dest.exists() loop in dispatch_paste_ops
    let format_copy_name = |orig: &str, num: usize| -> String {
        match orig.rfind('.') {
            Some(idx) if idx > 0 => {
                let (name, ext) = orig.split_at(idx);
                format!("{} (copy {}){}", name, num, ext)
            }
            _ => format!("{} (copy {})", orig, num),
        }
    };

    assert_eq!(format_copy_name(&original_name, 1), "document (copy 1).pdf");
    assert_eq!(format_copy_name(&original_name, 2), "document (copy 2).pdf");

    let no_ext = "Makefile".to_string();
    assert_eq!(format_copy_name(&no_ext, 1), "Makefile (copy 1)");
}

#[test]
fn test_scan_total_bytes_directory_tree() {
    let dir = tempdir().unwrap();
    let root = dir.path();

    // Create a 100-byte file
    let file1_path = root.join("file1.bin");
    let mut file1 = File::create(&file1_path).unwrap();
    file1.write_all(&[0u8; 100]).unwrap();

    // Create a nested directory with a 250-byte file
    let sub_dir = root.join("sub");
    fs::create_dir(&sub_dir).unwrap();
    let file2_path = sub_dir.join("file2.bin");
    let mut file2 = File::create(&file2_path).unwrap();
    file2.write_all(&[0u8; 250]).unwrap();

    let total_bytes = scan_total_bytes(root);
    assert_eq!(total_bytes, 350);
}

#[test]
fn test_recursive_paste_prevention() {
    let parent = Path::new("/home/user/Documents");
    let child = Path::new("/home/user/Documents/Projects/Flux");
    let unrelated = Path::new("/home/user/Downloads");

    assert!(is_recursive_paste(parent, child));
    assert!(!is_recursive_paste(child, parent));
    assert!(!is_recursive_paste(parent, unrelated));
}

#[test]
fn test_conflict_detection_logic() {
    let dir = tempdir().unwrap();
    let target_dir = dir.path();

    // Create an existing folder in the destination
    let existing_folder = target_dir.join("ExistingFolder");
    fs::create_dir(&existing_folder).unwrap();

    let incoming_items = vec![
        ("ExistingFolder".to_string(), true),
        ("NewFolder".to_string(), true),
        ("existing_file.txt".to_string(), false),
    ];

    let mut conflicts = Vec::new();

    for (name, is_dir) in incoming_items {
        if is_dir {
            let dest = target_dir.join(&name);
            if dest.exists() && dest.is_dir() {
                conflicts.push(name);
            }
        }
    }

    assert_eq!(conflicts.len(), 1);
    assert_eq!(conflicts[0], "ExistingFolder");
}
