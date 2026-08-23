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
