//! `spawn_batch` and `spawn_single`: when a thumbnail task gives up before doing
//! any work.

use flux::infra::thumbnails::{
    spawn_batch, spawn_single, BatchRequest, SingleRequest, ThumbnailTaskManager,
};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Semaphore;

/// Long enough for a task that is not blocked to have finished, short enough to keep the suite fast.
const SETTLE: Duration = Duration::from_millis(300);
/// Upper bound for a task that is expected to finish.
const DONE: Duration = Duration::from_secs(10);

#[derive(Debug, PartialEq)]
enum Outcome {
    /// The task ended and never delivered a thumbnail.
    Finished,
    /// The task is still running.
    StillRunning,
    /// The task delivered a thumbnail.
    Delivered,
}

fn wait(rx: &Receiver<u32>, within: Duration) -> Outcome {
    match rx.recv_timeout(within) {
        Ok(_) => Outcome::Delivered,
        Err(RecvTimeoutError::Disconnected) => Outcome::Finished,
        Err(RecvTimeoutError::Timeout) => Outcome::StillRunning,
    }
}

fn no_permits() -> Arc<Semaphore> {
    Arc::new(Semaphore::new(0))
}

fn missing(name: &str) -> PathBuf {
    PathBuf::from("/nonexistent-flux-test-dir").join(name)
}

fn run_batch(
    tasks: Vec<(u32, PathBuf)>,
    session: &Arc<AtomicU64>,
    expected_session: u64,
    semaphore: &Arc<Semaphore>,
    skip_active_downloads: bool,
) -> Receiver<u32> {
    let (tx, rx) = mpsc::channel();
    spawn_batch(
        BatchRequest {
            tasks,
            expected_session,
            session: session.clone(),
            semaphore: semaphore.clone(),
            max_concurrency: 4,
            skip_active_downloads,
        },
        move |grid_idx, _image| {
            let _ = tx.send(grid_idx);
        },
    );
    rx
}

fn run_single(
    manager: &Arc<ThumbnailTaskManager>,
    grid_idx: u32,
    session: &Arc<AtomicU64>,
    expected_session: u64,
    cancel_flag: Arc<AtomicBool>,
    semaphore: &Arc<Semaphore>,
) -> Receiver<u32> {
    let (tx, rx) = mpsc::channel();
    spawn_single(
        SingleRequest {
            grid_idx,
            media_path: missing("single.png"),
            expected_session,
            session: session.clone(),
            cancel_flag,
            semaphore: semaphore.clone(),
            manager: manager.clone(),
        },
        move |grid_idx, _image| {
            let _ = tx.send(grid_idx);
        },
    );
    rx
}

// ─── spawn_batch ──────────────────────────────────────────────────────────────

#[test]
fn an_empty_batch_finishes_at_once() {
    let session = Arc::new(AtomicU64::new(1));
    let rx = run_batch(Vec::new(), &session, 1, &no_permits(), false);

    assert_eq!(wait(&rx, DONE), Outcome::Finished);
}

#[test]
fn a_batch_from_a_superseded_session_never_reaches_the_semaphore() {
    let session = Arc::new(AtomicU64::new(2));
    let tasks = vec![(0, missing("a.png")), (1, missing("b.png"))];
    let rx = run_batch(tasks, &session, 1, &no_permits(), false);

    assert_eq!(wait(&rx, DONE), Outcome::Finished);
}

#[test]
fn a_current_batch_waits_for_a_permit() {
    let session = Arc::new(AtomicU64::new(1));
    let semaphore = no_permits();
    let rx = run_batch(vec![(0, missing("a.png"))], &session, 1, &semaphore, false);

    assert_eq!(wait(&rx, SETTLE), Outcome::StillRunning);

    semaphore.close();
    assert_eq!(wait(&rx, DONE), Outcome::Finished);
}

#[test]
fn a_batch_superseded_while_waiting_for_a_permit_does_no_work() {
    let session = Arc::new(AtomicU64::new(1));
    let semaphore = no_permits();
    let rx = run_batch(vec![(0, missing("a.png"))], &session, 1, &semaphore, false);
    assert_eq!(wait(&rx, SETTLE), Outcome::StillRunning);

    session.store(2, Ordering::Release);
    semaphore.add_permits(1);

    assert_eq!(wait(&rx, DONE), Outcome::Finished);
}

#[test]
fn a_closed_semaphore_ends_the_batch_without_delivering() {
    let session = Arc::new(AtomicU64::new(1));
    let semaphore = no_permits();
    semaphore.close();
    let rx = run_batch(vec![(0, missing("a.png"))], &session, 1, &semaphore, false);

    assert_eq!(wait(&rx, DONE), Outcome::Finished);
}

#[test]
fn a_file_still_being_downloaded_is_skipped_before_the_semaphore() {
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("a.png");
    std::fs::write(&target, "x").unwrap();
    std::fs::write(dir.path().join("a.png.part"), "x").unwrap();
    let session = Arc::new(AtomicU64::new(1));

    let rx = run_batch(vec![(0, target)], &session, 1, &no_permits(), true);

    assert_eq!(wait(&rx, DONE), Outcome::Finished);
}

#[test]
fn the_download_check_only_applies_when_asked_for() {
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("a.png");
    std::fs::write(&target, "x").unwrap();
    std::fs::write(dir.path().join("a.png.part"), "x").unwrap();
    let session = Arc::new(AtomicU64::new(1));
    let semaphore = no_permits();

    let rx = run_batch(vec![(0, target)], &session, 1, &semaphore, false);

    assert_eq!(wait(&rx, SETTLE), Outcome::StillRunning);
    semaphore.close();
    assert_eq!(wait(&rx, DONE), Outcome::Finished);
}

#[test]
fn a_finished_download_is_not_skipped() {
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("a.png");
    std::fs::write(&target, "x").unwrap();
    let session = Arc::new(AtomicU64::new(1));
    let semaphore = no_permits();

    let rx = run_batch(vec![(0, target)], &session, 1, &semaphore, true);

    assert_eq!(wait(&rx, SETTLE), Outcome::StillRunning);
    semaphore.close();
    assert_eq!(wait(&rx, DONE), Outcome::Finished);
}

// ─── spawn_single ─────────────────────────────────────────────────────────────

#[test]
fn a_superseded_single_task_stops_tracking_itself() {
    let manager = Arc::new(ThumbnailTaskManager::default());
    let flag = manager.register_token(5);
    let session = Arc::new(AtomicU64::new(2));

    let rx = run_single(&manager, 5, &session, 1, flag, &no_permits());

    assert_eq!(wait(&rx, DONE), Outcome::Finished);
    assert!(manager.cancel_out_of_viewport(100, 200).is_empty());
}

#[test]
fn a_cancelled_single_task_stops_before_the_semaphore() {
    let manager = Arc::new(ThumbnailTaskManager::default());
    let flag = manager.register_token(6);
    flag.store(true, Ordering::Release);
    let session = Arc::new(AtomicU64::new(1));

    let rx = run_single(&manager, 6, &session, 1, flag, &no_permits());

    assert_eq!(wait(&rx, DONE), Outcome::Finished);
    assert!(manager.cancel_out_of_viewport(100, 200).is_empty());
}

#[test]
fn a_closed_semaphore_ends_a_single_task_and_clears_its_token() {
    let manager = Arc::new(ThumbnailTaskManager::default());
    let flag = manager.register_token(8);
    let session = Arc::new(AtomicU64::new(1));
    let semaphore = no_permits();
    semaphore.close();

    let rx = run_single(&manager, 8, &session, 1, flag, &semaphore);

    assert_eq!(wait(&rx, DONE), Outcome::Finished);
    assert!(manager.cancel_out_of_viewport(100, 200).is_empty());
}

#[test]
fn a_waiting_single_task_stays_tracked_and_is_cancelled_by_scrolling() {
    let manager = Arc::new(ThumbnailTaskManager::default());
    let flag = manager.register_token(9);
    let session = Arc::new(AtomicU64::new(1));
    let semaphore = no_permits();

    let rx = run_single(&manager, 9, &session, 1, flag, &semaphore);

    assert_eq!(wait(&rx, SETTLE), Outcome::StillRunning);
    assert_eq!(manager.cancel_out_of_viewport(100, 200), vec![9]);

    semaphore.add_permits(1);
    assert_eq!(wait(&rx, DONE), Outcome::Finished);
}
