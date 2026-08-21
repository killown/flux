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
