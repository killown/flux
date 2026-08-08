use flux::services::tasks::TaskQueue;
use gtk::gio::prelude::CancellableExt;

#[test]
fn test_summary_empty_queue_returns_none() {
    let q = TaskQueue::default();
    assert!(q.summary().is_none());
}

#[test]
fn test_update_and_summary_single_task() {
    let q = TaskQueue::default();
    let c = gtk::gio::Cancellable::new();
    q.update(1, "test".to_string(), 50, 100, 3, c);

    let (ops, items, avg) = q.summary().expect("queue must be non-empty");
    assert_eq!(ops, 1);
    assert_eq!(items, 3);
    assert!((avg - 0.5).abs() < f64::EPSILON);
}

#[test]
fn test_remove_empties_queue() {
    let q = TaskQueue::default();
    let c = gtk::gio::Cancellable::new();
    q.update(1, "test".to_string(), 10, 10, 1, c);
    q.remove(1);
    assert!(q.summary().is_none());
}

#[test]
fn test_cancel_all_clears_queue() {
    let q = TaskQueue::default();
    q.update(
        1,
        "test".to_string(),
        0,
        100,
        1,
        gtk::gio::Cancellable::new(),
    );
    q.update(
        2,
        "test".to_string(),
        0,
        200,
        2,
        gtk::gio::Cancellable::new(),
    );
    q.cancel_all();
    assert!(q.summary().is_none());
}

#[test]
fn test_summary_averages_multiple_tasks() {
    let q = TaskQueue::default();
    q.update(
        1,
        "test".to_string(),
        0,
        100,
        1,
        gtk::gio::Cancellable::new(),
    );
    q.update(
        2,
        "test".to_string(),
        200,
        200,
        1,
        gtk::gio::Cancellable::new(),
    );

    let (ops, items, avg) = q.summary().unwrap();
    assert_eq!(ops, 2);
    assert_eq!(items, 2);
    assert!((avg - 0.5).abs() < f64::EPSILON);
}

#[test]
fn test_summary_zero_total_contributes_zero_progress() {
    let q = TaskQueue::default();
    q.update(1, "test".to_string(), 0, 0, 1, gtk::gio::Cancellable::new());
    let (_, _, avg) = q.summary().unwrap();
    assert!((avg - 0.0).abs() < f64::EPSILON);
}

#[test]
fn test_remove_nonexistent_is_a_noop() {
    let q = TaskQueue::default();
    q.remove(999);
    assert!(q.summary().is_none());
}

#[test]
fn test_update_preserves_cancellable() {
    let q = TaskQueue::default();
    let c1 = gtk::gio::Cancellable::new();
    let c2 = gtk::gio::Cancellable::new();

    q.update(1, "file1".to_string(), 0, 100, 1, c1.clone());
    q.update(1, "file1".to_string(), 50, 100, 1, c2.clone());

    let snapshot = q.snapshot();
    let task = snapshot.iter().find(|(id, _)| *id == 1).unwrap().1.clone();
    assert!(!task.cancellable.is_cancelled());
    c1.cancel();
    assert!(task.cancellable.is_cancelled());
    assert!(!c2.is_cancelled());
}
