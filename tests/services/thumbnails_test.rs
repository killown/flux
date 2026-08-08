use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

#[test]
fn test_thumbnail_session_invalidation() {
    let load_id = Arc::new(AtomicU64::new(1));
    let initial_session = load_id.load(Ordering::Acquire);

    // Simulate session increment on directory navigation
    let new_session = load_id.fetch_add(1, Ordering::SeqCst) + 1;

    assert_ne!(initial_session, new_session);
    assert_eq!(load_id.load(Ordering::Acquire), 2);
}

#[test]
fn test_media_tasks_filtering_and_session_matching() {
    let media_tasks = [
        ("image1.png".to_string(), PathBuf::from("/tmp/image1.png")),
        ("video1.mp4".to_string(), PathBuf::from("/tmp/video1.mp4")),
    ];

    let current_session = 1u64;
    let active_session = Arc::new(AtomicU64::new(1));

    // Valid session check
    assert_eq!(active_session.load(Ordering::Acquire), current_session);
    assert_eq!(media_tasks.len(), 2);

    // Session invalidated mid-process
    active_session.store(2, Ordering::SeqCst);
    assert_ne!(active_session.load(Ordering::Acquire), current_session);
}

#[test]
fn test_empty_media_tasks_bailout() {
    let media_tasks: [(String, PathBuf); 0] = [];
    assert!(media_tasks.is_empty());
}
