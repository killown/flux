use flux::ui::conflict_policy::{
    auto_rename_dest, ConflictChoice, ConflictContext, ConflictPolicy,
};
use std::fs::File;
use std::path::PathBuf;
use tempfile::tempdir;

#[test]
fn test_auto_rename_dest_creates_suffixed_filename() {
    let tmp = tempdir().unwrap();
    let existing_file = tmp.path().join("document.pdf");
    File::create(&existing_file).unwrap();

    let renamed = auto_rename_dest(&existing_file);
    assert_eq!(renamed, tmp.path().join("document (1).pdf"));
}

#[test]
fn test_auto_rename_dest_increments_existing_suffixes() {
    let tmp = tempdir().unwrap();
    let file0 = tmp.path().join("image.png");
    let file1 = tmp.path().join("image (1).png");
    let file2 = tmp.path().join("image (2).png");

    File::create(&file0).unwrap();
    File::create(&file1).unwrap();
    File::create(&file2).unwrap();

    let renamed = auto_rename_dest(&file0);
    assert_eq!(renamed, tmp.path().join("image (3).png"));
}

#[test]
fn test_auto_rename_dest_handles_files_without_extension() {
    let tmp = tempdir().unwrap();
    let no_ext = tmp.path().join("LICENSE");
    File::create(&no_ext).unwrap();

    let renamed = auto_rename_dest(&no_ext);
    assert_eq!(renamed, tmp.path().join("LICENSE (1)"));
}

#[test]
fn test_auto_rename_dest_handles_directories() {
    let tmp = tempdir().unwrap();
    let dir = tmp.path().join("my_folder");
    std::fs::create_dir(&dir).unwrap();

    let renamed = auto_rename_dest(&dir);
    assert_eq!(renamed, tmp.path().join("my_folder (1)"));
}

#[test]
fn test_conflict_policy_default_is_ask() {
    assert_eq!(ConflictPolicy::default(), ConflictPolicy::Ask);
}

#[test]
fn test_conflict_choice_to_policy_mapping() {
    let map_choice = |choice: &ConflictChoice| match choice {
        ConflictChoice::Replace => ConflictPolicy::ReplaceAll,
        ConflictChoice::Skip => ConflictPolicy::SkipAll,
        ConflictChoice::AutoRename => ConflictPolicy::AutoRenameAll,
        ConflictChoice::Cancel => ConflictPolicy::Ask,
    };

    assert_eq!(
        map_choice(&ConflictChoice::Replace),
        ConflictPolicy::ReplaceAll
    );
    assert_eq!(map_choice(&ConflictChoice::Skip), ConflictPolicy::SkipAll);
    assert_eq!(
        map_choice(&ConflictChoice::AutoRename),
        ConflictPolicy::AutoRenameAll
    );
    assert_eq!(map_choice(&ConflictChoice::Cancel), ConflictPolicy::Ask);
}

#[test]
fn test_conflict_context_batch_subtitles() {
    let ctx = ConflictContext {
        src: PathBuf::from("/tmp/src.txt"),
        dest: PathBuf::from("/tmp/dest.txt"),
        is_cut: true,
        batch_total: 5,
        batch_index: 2,
    };

    let op_word = if ctx.is_cut { "move" } else { "copy" };
    assert_eq!(op_word, "move");
    assert_eq!(ctx.batch_total, 5);
    assert_eq!(ctx.batch_index, 2);
}

#[test]
fn auto_rename_single_digit_boundary() {
    let tmp = tempdir().unwrap();
    let base = tmp.path().join("f.txt");
    File::create(&base).unwrap();
    for i in 1..10 {
        File::create(tmp.path().join(format!("f ({}).txt", i))).unwrap();
    }
    let next = auto_rename_dest(&base);
    assert_eq!(next.file_name().unwrap(), "f (10).txt");
}

#[test]
fn auto_rename_double_digit_boundary() {
    let tmp = tempdir().unwrap();
    let base = tmp.path().join("f.txt");
    File::create(&base).unwrap();
    for i in 1..100 {
        File::create(tmp.path().join(format!("f ({}).txt", i))).unwrap();
    }
    let next = auto_rename_dest(&base);
    assert_eq!(next.file_name().unwrap(), "f (100).txt");
}

#[test]
fn auto_rename_with_leading_dot_name() {
    let tmp = tempdir().unwrap();
    let f = tmp.path().join(".hidden");
    File::create(&f).unwrap();
    let renamed = auto_rename_dest(&f);
    assert!(renamed
        .file_name()
        .unwrap()
        .to_string_lossy()
        .starts_with(".hidden"));
}

#[test]
fn auto_rename_preserves_parent_dir() {
    let tmp = tempdir().unwrap();
    let sub = tmp.path().join("sub");
    std::fs::create_dir(&sub).unwrap();
    let f = sub.join("x.txt");
    File::create(&f).unwrap();
    let renamed = auto_rename_dest(&f);
    assert_eq!(renamed.parent().unwrap(), sub);
}

#[test]
fn auto_rename_multiple_extensions() {
    let tmp = tempdir().unwrap();
    let f = tmp.path().join("archive.tar.gz");
    File::create(&f).unwrap();
    let renamed = auto_rename_dest(&f);
    assert_eq!(renamed.file_name().unwrap(), "archive.tar (1).gz");
}

#[test]
fn conflict_choice_equality() {
    assert_eq!(ConflictChoice::Replace, ConflictChoice::Replace);
    assert_ne!(ConflictChoice::Replace, ConflictChoice::Skip);
}

#[test]
fn conflict_policy_all_variants_distinct() {
    use std::mem::discriminant;
    let variants = [
        ConflictPolicy::Ask,
        ConflictPolicy::ReplaceAll,
        ConflictPolicy::SkipAll,
        ConflictPolicy::AutoRenameAll,
    ];
    let discriminants: std::collections::HashSet<_> = variants.iter().map(discriminant).collect();
    assert_eq!(discriminants.len(), 4);
}

#[test]
fn conflict_context_fields_preserved() {
    let ctx = ConflictContext {
        src: PathBuf::from("/a"),
        dest: PathBuf::from("/b"),
        is_cut: false,
        batch_total: 100,
        batch_index: 99,
    };
    assert_eq!(ctx.src, PathBuf::from("/a"));
    assert_eq!(ctx.dest, PathBuf::from("/b"));
    assert!(!ctx.is_cut);
    assert_eq!(ctx.batch_total, 100);
    assert_eq!(ctx.batch_index, 99);
}
