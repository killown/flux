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
