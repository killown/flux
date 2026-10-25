use flux::model::FluxApp;
use flux::ui::undo_redo::{FileOp, FileOpHistory, TrashedItem};
use std::fs;
use std::path::{Path, PathBuf};

#[test]
fn test_history_starts_empty() {
    let mut history = FileOpHistory::new();
    assert!(!history.can_undo());
    assert!(!history.can_redo());
    assert!(history.pop_undo().is_none());
    assert!(history.pop_redo().is_none());
}

#[test]
fn test_push_undo_clears_redo_stack() {
    let mut history = FileOpHistory::new();
    let op1 = FileOp::Trash {
        items: vec![TrashedItem::untracked(PathBuf::from("/tmp/test1.txt"))],
    };
    let op2 = FileOp::Trash {
        items: vec![TrashedItem::untracked(PathBuf::from("/tmp/test2.txt"))],
    };

    history.push_undo(op1);
    let popped = history.pop_undo().unwrap();
    history.push_redo(popped);
    assert!(history.can_redo());

    history.push_undo(op2);
    assert!(!history.can_redo());
    assert!(history.can_undo());
}

#[test]
fn test_undo_stack_pop_order() {
    let mut history = FileOpHistory::new();

    for i in 0..10 {
        history.push_undo(FileOp::Trash {
            items: vec![TrashedItem::untracked(PathBuf::from(format!(
                "/tmp/file_{}.txt",
                i
            )))],
        });
    }

    assert!(history.can_undo());
    let mut count = 0;
    while let Some(op) = history.pop_undo() {
        if let FileOp::Trash { items } = op {
            assert_eq!(
                items[0].path,
                PathBuf::from(format!("/tmp/file_{}.txt", 9 - count))
            );
        }
        count += 1;
    }
    assert_eq!(count, 10);
    assert!(!history.can_undo());
}

/// Renames `from` to `to` inside `dir` and records it the way
/// `handle_perform_rename` does: canonical paths, op pushed on the undo stack.
fn rename_and_record(dir: &Path, from: &str, to: &str, history: &mut FileOpHistory) {
    let old_path = dir.join(from);
    let new_path = FluxApp::rename_path(&old_path, to).unwrap();
    history.push_undo(FileOp::Rename {
        old_path: old_path.canonicalize().unwrap_or_else(|_| old_path.clone()),
        new_path: new_path.canonicalize().unwrap_or_else(|_| new_path.clone()),
        old_name: from.to_string(),
        new_name: to.to_string(),
    });
}

fn undo_once(history: &mut FileOpHistory) -> std::io::Result<()> {
    let op = history.pop_undo().expect("nothing to undo");
    FluxApp::undo_rename(history, op)
}

fn redo_once(history: &mut FileOpHistory) -> std::io::Result<()> {
    let op = history.pop_redo().expect("nothing to redo");
    FluxApp::redo_rename(history, op)
}

#[test]
fn rename_redo_after_undo_renames_again() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().canonicalize().unwrap();
    fs::write(dir.join("a.txt"), "payload").unwrap();
    let mut history = FileOpHistory::new();
    rename_and_record(&dir, "a.txt", "b.txt", &mut history);

    undo_once(&mut history).unwrap();
    assert!(dir.join("a.txt").exists());
    assert!(!dir.join("b.txt").exists());
    assert!(history.can_redo());
    assert!(!history.can_undo());

    redo_once(&mut history).unwrap();
    assert!(!dir.join("a.txt").exists());
    assert_eq!(fs::read_to_string(dir.join("b.txt")).unwrap(), "payload");
    assert!(history.can_undo());
    assert!(!history.can_redo());
}

#[test]
fn rename_undo_redo_undo_round_trip() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().canonicalize().unwrap();
    fs::write(dir.join("a.txt"), "payload").unwrap();
    let mut history = FileOpHistory::new();
    rename_and_record(&dir, "a.txt", "b.txt", &mut history);

    for _ in 0..3 {
        undo_once(&mut history).unwrap();
        assert!(dir.join("a.txt").exists() && !dir.join("b.txt").exists());
        redo_once(&mut history).unwrap();
        assert!(dir.join("b.txt").exists() && !dir.join("a.txt").exists());
    }
    undo_once(&mut history).unwrap();
    assert!(dir.join("a.txt").exists() && !dir.join("b.txt").exists());
    assert_eq!(
        history.redo_label().unwrap(),
        "Rename \"a.txt\" → \"b.txt\""
    );
}

#[test]
fn rename_directory_survives_undo_redo() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().canonicalize().unwrap();
    fs::create_dir(dir.join("old")).unwrap();
    fs::write(dir.join("old").join("child.txt"), "x").unwrap();
    let mut history = FileOpHistory::new();
    rename_and_record(&dir, "old", "new", &mut history);

    undo_once(&mut history).unwrap();
    assert!(dir.join("old").join("child.txt").exists());
    redo_once(&mut history).unwrap();
    assert!(dir.join("new").join("child.txt").exists());
    assert!(!dir.join("old").exists());
}

#[test]
fn rename_undo_blocked_by_existing_name_pushes_nothing() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().canonicalize().unwrap();
    fs::write(dir.join("a.txt"), "orig").unwrap();
    let mut history = FileOpHistory::new();
    rename_and_record(&dir, "a.txt", "b.txt", &mut history);
    fs::write(dir.join("a.txt"), "squatter").unwrap();

    assert!(undo_once(&mut history).is_err());
    assert_eq!(fs::read_to_string(dir.join("a.txt")).unwrap(), "squatter");
    assert_eq!(fs::read_to_string(dir.join("b.txt")).unwrap(), "orig");
    assert!(!history.can_redo());
}

#[test]
fn undo_and_redo_rename_reject_other_ops() {
    let mut history = FileOpHistory::new();
    let op = || FileOp::Trash { items: vec![] };
    assert!(FluxApp::undo_rename(&mut history, op()).is_err());
    assert!(FluxApp::redo_rename(&mut history, op()).is_err());
    assert!(!history.can_undo() && !history.can_redo());
}

#[test]
fn history_limit_evicts_oldest() {
    let mut h = FileOpHistory::new();
    for i in 0..100 {
        h.push_undo(FileOp::Trash {
            items: vec![TrashedItem::untracked(PathBuf::from(format!(
                "/tmp/f{}",
                i
            )))],
        });
    }
    // Undo stack is capped, popping all should not return 100 entries.
    let mut count = 0;
    while h.pop_undo().is_some() {
        count += 1;
    }
    assert!(count <= 64, "history must be capped, got {}", count);
}

#[test]
fn can_undo_false_after_pop_all() {
    let mut h = FileOpHistory::new();
    h.push_undo(FileOp::Trash {
        items: vec![TrashedItem::untracked(PathBuf::from("/tmp/a"))],
    });
    h.pop_undo();
    assert!(!h.can_undo());
}

#[test]
fn can_redo_false_after_pop_all() {
    let mut h = FileOpHistory::new();
    let op = FileOp::Trash {
        items: vec![TrashedItem::untracked(PathBuf::from("/tmp/a"))],
    };
    h.push_redo(op);
    h.pop_redo();
    assert!(!h.can_redo());
}

#[test]
fn clear_empties_both_stacks() {
    let mut h = FileOpHistory::new();
    h.push_undo(FileOp::Trash { items: vec![] });
    h.push_redo(FileOp::Trash { items: vec![] });
    h.clear();
    assert!(!h.can_undo());
    assert!(!h.can_redo());
}

#[test]
fn move_op_label_plural() {
    let op = FileOp::Move {
        items: vec![
            (PathBuf::from("/a"), PathBuf::from("/b")),
            (PathBuf::from("/c"), PathBuf::from("/d")),
        ],
        dest_dir: PathBuf::from("/dest"),
    };
    assert!(op.label().contains("2"));
}

#[test]
fn copy_op_label_singular() {
    let op = FileOp::Copy {
        copies: vec![PathBuf::from("/a")],
        dest_dir: PathBuf::from("/dest"),
    };
    assert!(op.label().contains("Copy"));
}

#[test]
fn trash_op_label_plural() {
    let op = FileOp::Trash {
        items: vec![
            TrashedItem::untracked(PathBuf::from("/a")),
            TrashedItem::untracked(PathBuf::from("/b")),
            TrashedItem::untracked(PathBuf::from("/c")),
        ],
    };
    assert!(op.label().contains("3"));
}

#[test]
fn rename_op_label_includes_both_names() {
    let op = FileOp::Rename {
        old_path: PathBuf::from("/a"),
        new_path: PathBuf::from("/b"),
        old_name: "old.txt".into(),
        new_name: "new.txt".into(),
    };
    let label = op.label();
    assert!(label.contains("old.txt"));
    assert!(label.contains("new.txt"));
}

#[test]
fn push_redo_preserves_order() {
    let mut h = FileOpHistory::new();
    h.push_redo(FileOp::Trash {
        items: vec![TrashedItem::untracked(PathBuf::from("/a"))],
    });
    h.push_redo(FileOp::Trash {
        items: vec![TrashedItem::untracked(PathBuf::from("/b"))],
    });
    let first = h.pop_redo().unwrap();
    if let FileOp::Trash { items } = first {
        assert_eq!(items[0].path, PathBuf::from("/b"));
    }
}

#[test]
fn undo_label_reflects_top_of_stack() {
    let mut h = FileOpHistory::new();
    h.push_undo(FileOp::Trash {
        items: vec![TrashedItem::untracked(PathBuf::from("/old"))],
    });
    h.push_undo(FileOp::Trash {
        items: vec![TrashedItem::untracked(PathBuf::from("/newest"))],
    });
    let label = h.undo_label().unwrap();
    assert!(label.contains("newest"));
}

#[test]
fn trashed_items_with_same_path_stay_distinct() {
    let first = TrashedItem {
        path: PathBuf::from("/home/u/report.txt"),
        trash_name: Some("report.txt".into()),
        deletion_date: Some("2026-10-25T10:00:00".into()),
    };
    let second = TrashedItem {
        path: PathBuf::from("/home/u/report.txt"),
        trash_name: Some("report.2.txt".into()),
        deletion_date: Some("2026-10-25T10:05:00".into()),
    };
    assert_ne!(first, second);

    let op = FileOp::Trash {
        items: vec![first.clone(), second.clone()],
    };
    if let FileOp::Trash { items } = op {
        assert_eq!(items[0].trash_name.as_deref(), Some("report.txt"));
        assert_eq!(items[1].trash_name.as_deref(), Some("report.2.txt"));
    }
}

#[test]
fn untracked_item_has_no_trash_identity() {
    let item = TrashedItem::untracked(PathBuf::from("/a"));
    assert!(item.trash_name.is_none());
    assert!(item.deletion_date.is_none());
}

#[test]
fn trash_op_label_singular_uses_file_name() {
    let op = FileOp::Trash {
        items: vec![TrashedItem::untracked(PathBuf::from("/x/notes.md"))],
    };
    assert!(op.label().contains("notes.md"));
}
