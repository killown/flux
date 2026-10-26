use std::collections::VecDeque;
use std::path::PathBuf;

#[test]
fn test_history_navigation_back_and_forward() {
    let mut history: Vec<PathBuf> = vec![PathBuf::from("/home"), PathBuf::from("/home/user")];
    let mut forward_stack: Vec<PathBuf> = Vec::new();
    let mut current_path = PathBuf::from("/home/user/Documents");

    // Go Back
    if let Some(prev) = history.pop() {
        forward_stack.push(current_path.clone());
        current_path = prev;
    }

    assert_eq!(current_path, PathBuf::from("/home/user"));
    assert_eq!(forward_stack.len(), 1);
    assert_eq!(forward_stack[0], PathBuf::from("/home/user/Documents"));

    // Go Forward
    if let Some(next) = forward_stack.pop() {
        history.push(current_path.clone());
        current_path = next;
    }

    assert_eq!(current_path, PathBuf::from("/home/user/Documents"));
    assert_eq!(history.len(), 2);
    assert_eq!(history[1], PathBuf::from("/home/user"));
}

#[test]
fn test_recent_stack_deduplication_and_truncation() {
    let max_recent = 5;
    let mut recent_stack: VecDeque<PathBuf> = VecDeque::new();

    let paths = vec![
        PathBuf::from("/dir1"),
        PathBuf::from("/dir2"),
        PathBuf::from("/dir3"),
        PathBuf::from("/dir4"),
        PathBuf::from("/dir5"),
        PathBuf::from("/dir2"), // Duplicate access
    ];

    for path in paths {
        let old_path = PathBuf::from("/current");
        recent_stack.retain(|p| p != &path && p != &old_path);
        recent_stack.push_front(path);
        recent_stack.truncate(max_recent);
    }

    assert_eq!(recent_stack.len(), 5);
    assert_eq!(recent_stack[0], PathBuf::from("/dir2"));
    assert_eq!(recent_stack[1], PathBuf::from("/dir5"));
}

#[test]
fn test_quick_panel_add_remove_and_cycle() {
    let mut exclusive_list: Vec<PathBuf> = Vec::new();

    // Verify initial unassigned state
    let mut exclusive_index: Option<usize> = None;
    assert!(exclusive_index.is_none());

    let p1 = PathBuf::from("/tmp/folder1");
    let p2 = PathBuf::from("/tmp/folder2");

    // Add items
    exclusive_list.push(p1.clone());
    exclusive_index = Some(0);
    assert_eq!(exclusive_index, Some(0));

    exclusive_list.push(p2.clone());

    assert_eq!(exclusive_list.len(), 2);

    // Cycle Next
    let new_idx = match exclusive_index {
        Some(i) => (i + 1) % exclusive_list.len(),
        None => 0,
    };
    exclusive_index = Some(new_idx);
    assert_eq!(exclusive_index, Some(1));

    // Remove Item
    if let Some(pos) = exclusive_list.iter().position(|p| p == &p1) {
        exclusive_list.remove(pos);
        exclusive_index = if exclusive_list.is_empty() {
            None
        } else {
            Some(pos.saturating_sub(1).min(exclusive_list.len() - 1))
        };
    }

    assert_eq!(exclusive_list.len(), 1);
    assert_eq!(exclusive_list[0], p2);
    assert_eq!(exclusive_index, Some(0));
}
