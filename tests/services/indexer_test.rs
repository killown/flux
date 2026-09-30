use flux::services::indexer::{
    apply_event, delta_scan, full_scan, now_ns, remove_tree, worker_loop, Job, SearchIndex, ENABLED,
};
use rusqlite::Connection;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
use std::sync::mpsc;
use std::time::Duration;

struct Fixture {
    _tmp: tempfile::TempDir,
    root: PathBuf,
    idx: SearchIndex,
}

fn touch(p: &Path) {
    fs::create_dir_all(p.parent().unwrap()).unwrap();
    fs::write(p, "x").unwrap();
}

fn fixture() -> Fixture {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("home");
    touch(&root.join("docs/report_final.txt"));
    touch(&root.join("docs/notes/ideas.md"));
    touch(&root.join("pics/holiday.jpg"));
    touch(&root.join("100%_done/a_b.txt"));
    touch(&root.join("100x_done/keep.txt"));
    touch(&root.join(".hidden/secret.txt"));
    touch(&root.join("proj/target/debug/junk.o"));
    touch(&root.join("proj/node_modules/pkg/index.js"));
    let idx = SearchIndex::open_at(&tmp.path().join("index.db")).unwrap();
    Fixture {
        root,
        idx,
        _tmp: tmp,
    }
}

fn never() -> bool {
    false
}

fn built() -> Fixture {
    let f = fixture();
    assert!(full_scan(&f.idx, &[f.root.clone()], &never).unwrap());
    f
}

fn names(f: &Fixture, q: &str) -> Vec<String> {
    let mut v: Vec<String> = f
        .idx
        .query(q, 100)
        .unwrap()
        .into_iter()
        .map(|p| {
            p.strip_prefix(&f.root)
                .unwrap()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    v.sort();
    v
}

#[test]
fn full_scan_indexes_and_respects_exclusions() {
    let f = built();
    assert!(f.idx.is_ready());
    assert_eq!(names(&f, "report"), vec!["docs/report_final.txt"]);
    assert_eq!(names(&f, "REPORT_FIN"), vec!["docs/report_final.txt"]);
    assert!(names(&f, "secret").is_empty());
    assert!(names(&f, "junk").is_empty());
    assert!(names(&f, "index.js").is_empty());
    assert_eq!(names(&f, "notes"), vec!["docs/notes"]);
}

#[test]
fn delta_scan_picks_up_new_files_and_folders() {
    let f = built();
    touch(&f.root.join("docs/brand_new_file.txt"));
    touch(&f.root.join("docs/notes/deep/er/nested_addition.rs"));
    touch(&f.root.join("fresh_top_level/inside_fresh.txt"));
    assert!(names(&f, "brand_new").is_empty());

    delta_scan(&f.idx, &[f.root.clone()], &never).unwrap();

    assert_eq!(names(&f, "brand_new"), vec!["docs/brand_new_file.txt"]);
    assert_eq!(
        names(&f, "nested_addition"),
        vec!["docs/notes/deep/er/nested_addition.rs"]
    );
    assert_eq!(
        names(&f, "inside_fresh"),
        vec!["fresh_top_level/inside_fresh.txt"]
    );
    assert_eq!(names(&f, "fresh_top"), vec!["fresh_top_level"]);
}

#[test]
fn delta_scan_removes_deleted_and_renamed() {
    let f = built();
    fs::remove_file(f.root.join("docs/report_final.txt")).unwrap();
    fs::remove_dir_all(f.root.join("pics")).unwrap();
    fs::rename(f.root.join("docs/notes"), f.root.join("docs/renamed_notes")).unwrap();

    delta_scan(&f.idx, &[f.root.clone()], &never).unwrap();

    assert!(names(&f, "report").is_empty());
    assert!(names(&f, "holiday").is_empty());
    assert!(names(&f, "ideas").contains(&"docs/renamed_notes/ideas.md".to_string()));
    assert_eq!(names(&f, "ideas").len(), 1);
}

#[test]
fn delta_scan_is_idempotent_and_unchanged_dirs_are_skipped() {
    let f = built();
    let before = f.idx.len();
    delta_scan(&f.idx, &[f.root.clone()], &never).unwrap();
    delta_scan(&f.idx, &[f.root.clone()], &never).unwrap();
    assert_eq!(f.idx.len(), before);
    assert_eq!(names(&f, "report").len(), 1);
}

#[test]
fn stable_dirs_record_their_mtime_and_are_not_resynced() {
    let f = built();
    let old = filetime_epoch(3600);
    for d in ["docs", "docs/notes", "pics"] {
        set_mtime(&f.root.join(d), old);
    }
    delta_scan(&f.idx, &[f.root.clone()], &never).unwrap();
    let stored: i64 = f
        .idx
        .conn()
        .query_row(
            "SELECT mtime FROM entries WHERE path = ?1",
            [f.root.join("docs").to_str().unwrap()],
            |r| r.get(0),
        )
        .unwrap();
    assert_ne!(stored, 0);

    touch(&f.root.join("docs/sneaky.txt"));
    set_mtime(&f.root.join("docs"), old);
    delta_scan(&f.idx, &[f.root.clone()], &never).unwrap();
    assert!(names(&f, "sneaky").is_empty());

    set_mtime(&f.root.join("docs"), old + 5_000_000_000);
    delta_scan(&f.idx, &[f.root.clone()], &never).unwrap();
    assert_eq!(names(&f, "sneaky"), vec!["docs/sneaky.txt"]);
}

fn filetime_epoch(secs_ago: i64) -> i64 {
    now_ns() - secs_ago * 1_000_000_000
}

fn set_mtime(p: &Path, ns: i64) {
    use std::ffi::CString;
    let c = CString::new(p.to_str().unwrap()).unwrap();
    let ts = libc::timespec {
        tv_sec: ns / 1_000_000_000,
        tv_nsec: ns % 1_000_000_000,
    };
    let times = [
        libc::timespec {
            tv_sec: 0,
            tv_nsec: libc::UTIME_OMIT,
        },
        ts,
    ];
    assert_eq!(
        unsafe { libc::utimensat(libc::AT_FDCWD, c.as_ptr(), times.as_ptr(), 0) },
        0
    );
}

#[test]
fn events_update_incrementally() {
    let f = built();
    let stop = never;
    let new_file = f.root.join("docs/event_added.txt");
    touch(&new_file);
    let new_dir = f.root.join("event_dir");
    touch(&new_dir.join("child/leaf_file.txt"));

    apply_event(
        &f.idx.conn(),
        &[f.root.clone()],
        &Job::Created(new_file.clone()),
        &stop,
    )
    .unwrap();
    apply_event(
        &f.idx.conn(),
        &[f.root.clone()],
        &Job::Created(new_dir.clone()),
        &stop,
    )
    .unwrap();
    assert_eq!(names(&f, "event_added"), vec!["docs/event_added.txt"]);
    assert_eq!(
        names(&f, "leaf_file"),
        vec!["event_dir/child/leaf_file.txt"]
    );

    let moved = f.root.join("moved_dir");
    fs::rename(&new_dir, &moved).unwrap();
    apply_event(
        &f.idx.conn(),
        &[f.root.clone()],
        &Job::Moved(new_dir.clone(), moved.clone()),
        &stop,
    )
    .unwrap();
    assert_eq!(
        names(&f, "leaf_file"),
        vec!["moved_dir/child/leaf_file.txt"]
    );

    fs::remove_dir_all(&moved).unwrap();
    apply_event(
        &f.idx.conn(),
        &[f.root.clone()],
        &Job::Deleted(moved),
        &stop,
    )
    .unwrap();
    assert!(names(&f, "leaf_file").is_empty());

    let hidden = f.root.join(".cache/x_hidden.txt");
    touch(&hidden);
    apply_event(
        &f.idx.conn(),
        &[f.root.clone()],
        &Job::Created(hidden),
        &stop,
    )
    .unwrap();
    let outside = f.root.parent().unwrap().join("outside_file.txt");
    touch(&outside);
    apply_event(
        &f.idx.conn(),
        &[f.root.clone()],
        &Job::Created(outside),
        &stop,
    )
    .unwrap();
    assert!(names(&f, "x_hidden").is_empty());
    assert!(f.idx.query("outside_file", 10).unwrap().is_empty());
}

#[test]
fn created_event_for_missing_path_is_a_delete() {
    let f = built();
    let p = f.root.join("docs/report_final.txt");
    fs::remove_file(&p).unwrap();
    apply_event(&f.idx.conn(), &[f.root.clone()], &Job::Created(p), &never).unwrap();
    assert!(names(&f, "report").is_empty());
}

#[test]
fn wildcard_characters_in_folder_names_do_not_leak_deletes() {
    let f = built();
    assert_eq!(names(&f, "a_b").len(), 1);
    assert_eq!(names(&f, "keep").len(), 1);
    remove_tree(&f.idx.conn(), f.root.join("100%_done").to_str().unwrap()).unwrap();
    assert!(names(&f, "a_b").is_empty());
    assert_eq!(names(&f, "keep"), vec!["100x_done/keep.txt"]);

    touch(&f.root.join("docs2/sibling.txt"));
    delta_scan(&f.idx, &[f.root.clone()], &never).unwrap();
    remove_tree(&f.idx.conn(), f.root.join("docs").to_str().unwrap()).unwrap();
    assert_eq!(names(&f, "sibling"), vec!["docs2/sibling.txt"]);
}

#[test]
fn scoped_query_only_returns_that_subtree() {
    let f = built();
    touch(&f.root.join("docs/shared_name.txt"));
    touch(&f.root.join("pics/shared_name.txt"));
    touch(&f.root.join("docs2/shared_name.txt"));
    delta_scan(&f.idx, &[f.root.clone()], &never).unwrap();
    let hits = f
        .idx
        .query_in("shared_name", Some(&f.root.join("docs")), 50)
        .unwrap();
    assert_eq!(hits, vec![f.root.join("docs/shared_name.txt")]);
    assert_eq!(f.idx.query("shared_name", 50).unwrap().len(), 3);
}

#[test]
fn kind_change_file_to_dir_is_handled() {
    let f = built();
    let p = f.root.join("docs/morph");
    touch(&p);
    delta_scan(&f.idx, &[f.root.clone()], &never).unwrap();
    fs::remove_file(&p).unwrap();
    touch(&p.join("inner_of_morph.txt"));
    set_mtime(&f.root.join("docs"), now_ns() + 10_000_000_000);
    delta_scan(&f.idx, &[f.root.clone()], &never).unwrap();
    assert_eq!(
        names(&f, "inner_of_morph"),
        vec!["docs/morph/inner_of_morph.txt"]
    );
}

#[test]
fn symlinks_are_not_followed_and_non_utf8_is_skipped() {
    use std::os::unix::ffi::OsStrExt;
    let f = fixture();
    std::os::unix::fs::symlink(&f.root, f.root.join("docs/loop_to_root")).unwrap();
    let bad = f
        .root
        .join("docs")
        .join(std::ffi::OsStr::from_bytes(b"bad_\xff_name.txt"));
    fs::write(&bad, "x").unwrap();
    assert!(full_scan(&f.idx, &[f.root.clone()], &never).unwrap());
    assert_eq!(names(&f, "loop_to_root"), vec!["docs/loop_to_root"]);
    assert!(names(&f, "report").len() == 1);
    assert!(names(&f, "bad_").is_empty());

    delta_scan(&f.idx, &[f.root.clone()], &never).unwrap();
}

#[test]
fn old_schema_is_migrated_and_not_ready() {
    let tmp = tempfile::tempdir().unwrap();
    let db = tmp.path().join("index.db");
    {
        let c = Connection::open(&db).unwrap();
        c.execute_batch(
            "CREATE VIRTUAL TABLE file_index USING fts5(name, path UNINDEXED, is_dir UNINDEXED, tokenize='trigram');
             INSERT INTO file_index VALUES ('old.txt','/x/old.txt',0);",
        )
        .unwrap();
    }
    let idx = SearchIndex::open_at(&db).unwrap();
    assert!(!idx.is_ready());
    assert_eq!(idx.len(), 0);
    let again = SearchIndex::open_at(&db).unwrap();
    assert!(!again.is_ready());
}

#[test]
fn interrupted_full_scan_is_not_ready() {
    let f = fixture();
    assert!(!full_scan(&f.idx, &[f.root.clone()], &|| true).unwrap());
    assert!(!f.idx.is_ready());
}

#[test]
fn worker_applies_events_and_periodic_delta() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("home");
    touch(&root.join("a/first.txt"));
    let db = tmp.path().join("index.db");
    ENABLED.store(true, Ordering::Relaxed);
    let (tx, rx) = mpsc::channel();
    let (r, d) = (root.clone(), db.clone());
    std::thread::spawn(move || worker_loop(rx, d, vec![r]));

    let wait_for = |q: &str, present: bool| {
        for _ in 0..100 {
            let hit = SearchIndex::open_at(&db)
                .map(|i| !i.query(q, 5).unwrap().is_empty())
                .unwrap_or(false);
            if hit == present {
                return true;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        false
    };
    assert!(wait_for("first", true), "initial crawl");

    let p = root.join("a/live_event.txt");
    touch(&p);
    tx.send(Job::Created(p.clone())).unwrap();
    assert!(wait_for("live_event", true), "created event");

    fs::remove_file(&p).unwrap();
    tx.send(Job::Deleted(p)).unwrap();
    assert!(wait_for("live_event", false), "deleted event");

    touch(&root.join("b/external.txt"));
    tx.send(Job::Delta { force: true }).unwrap();
    assert!(wait_for("external", true), "delta scan");
    ENABLED.store(false, Ordering::Relaxed);
}
