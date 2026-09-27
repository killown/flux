use flux::ui::undo_redo::{FileOp, FileOpHistory};
use std::path::PathBuf;

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
        paths: vec![PathBuf::from("/tmp/test1.txt")],
    };
    let op2 = FileOp::Trash {
        paths: vec![PathBuf::from("/tmp/test2.txt")],
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
            paths: vec![PathBuf::from(format!("/tmp/file_{}.txt", i))],
        });
    }

    assert!(history.can_undo());
    let mut count = 0;
    while let Some(op) = history.pop_undo() {
        if let FileOp::Trash { paths } = op {
            assert_eq!(
                paths[0],
                PathBuf::from(format!("/tmp/file_{}.txt", 9 - count))
            );
        }
        count += 1;
    }
    assert_eq!(count, 10);
    assert!(!history.can_undo());
}

#[test]
fn test_rename_operation_symmetry() {
    let op = FileOp::Rename {
        old_path: PathBuf::from("/tmp/old_name.txt"),
        new_path: PathBuf::from("/tmp/new_name.txt"),
        old_name: "old_name.txt".to_string(),
        new_name: "new_name.txt".to_string(),
    };

    let inverse = match op {
        FileOp::Rename {
            old_path,
            new_path,
            old_name,
            new_name,
        } => FileOp::Rename {
            old_path: new_path,
            new_path: old_path,
            old_name: new_name,
            new_name: old_name,
        },
        _ => unreachable!(),
    };

    if let FileOp::Rename {
        old_path,
        new_path,
        old_name,
        new_name,
    } = inverse
    {
        assert_eq!(old_path, PathBuf::from("/tmp/new_name.txt"));
        assert_eq!(new_path, PathBuf::from("/tmp/old_name.txt"));
        assert_eq!(old_name, "new_name.txt");
        assert_eq!(new_name, "old_name.txt");
    }
}

#[test]
fn history_limit_evicts_oldest() {
    let mut h = FileOpHistory::new();
    for i in 0..100 {
        h.push_undo(FileOp::Trash {
            paths: vec![PathBuf::from(format!("/tmp/f{}", i))],
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
        paths: vec![PathBuf::from("/tmp/a")],
    });
    h.pop_undo();
    assert!(!h.can_undo());
}

#[test]
fn can_redo_false_after_pop_all() {
    let mut h = FileOpHistory::new();
    let op = FileOp::Trash {
        paths: vec![PathBuf::from("/tmp/a")],
    };
    h.push_redo(op);
    h.pop_redo();
    assert!(!h.can_redo());
}

#[test]
fn clear_empties_both_stacks() {
    let mut h = FileOpHistory::new();
    h.push_undo(FileOp::Trash { paths: vec![] });
    h.push_redo(FileOp::Trash { paths: vec![] });
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
        paths: vec![
            PathBuf::from("/a"),
            PathBuf::from("/b"),
            PathBuf::from("/c"),
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
        paths: vec![PathBuf::from("/a")],
    });
    h.push_redo(FileOp::Trash {
        paths: vec![PathBuf::from("/b")],
    });
    let first = h.pop_redo().unwrap();
    if let FileOp::Trash { paths } = first {
        assert_eq!(paths[0], PathBuf::from("/b"));
    }
}

#[test]
fn undo_label_reflects_top_of_stack() {
    let mut h = FileOpHistory::new();
    h.push_undo(FileOp::Trash {
        paths: vec![PathBuf::from("/old")],
    });
    h.push_undo(FileOp::Trash {
        paths: vec![PathBuf::from("/newest")],
    });
    let label = h.undo_label().unwrap();
    assert!(label.contains("newest"));
}
