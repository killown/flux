use rusqlite::{params, Connection};
use std::collections::HashMap;
use std::io;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::sync::OnceLock;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

const SCHEMA_VERSION: i64 = 2;
const DELTA_INTERVAL: Duration = Duration::from_secs(300);
const DELTA_MIN_GAP: Duration = Duration::from_secs(30);
const DEBOUNCE: Duration = Duration::from_millis(150);
const MAX_BATCH: usize = 4096;
/// Directory modifications within this window are untrusted to avoid race conditions.
const RACY_WINDOW_NS: i64 = 2_000_000_000;

fn should_index(name: &str) -> bool {
    if name.starts_with('.') {
        return false;
    }
    !matches!(
        name,
        "target" | "node_modules" | "dosdevices" | "drive_c" | "compatdata" | "pfx"
    )
}

fn is_indexable(root: &Path, path: &Path) -> bool {
    if path.starts_with("/run/user") {
        return false;
    }
    if is_under_wine_root(path) {
        return false;
    }
    let Ok(rel) = path.strip_prefix(root) else {
        return false;
    };
    let mut any = false;
    for c in rel.components() {
        match c.as_os_str().to_str() {
            Some(s) if should_index(s) => any = true,
            _ => return false,
        }
    }
    any
}

/// True for paths inside a Wine / Proton prefix root. Filtering the
/// container directory names (`drive_c`, `compatdata`, `pfx`, `dosdevices`)
/// handles the common case, but a user can override `WINEPREFIX` to any
/// directory, so the well-known roots are checked explicitly too.
fn is_under_wine_root(path: &Path) -> bool {
    let Some(home) = dirs::home_dir() else {
        return false;
    };
    for rel in [
        ".wine",
        ".local/share/wineprefixes",
        ".steam/steam/steamapps/compatdata",
        ".local/share/Steam/steamapps/compatdata",
    ] {
        if path.starts_with(home.join(rel)) {
            return true;
        }
    }
    false
}

fn join(parent: &str, name: &str) -> String {
    format!("{}/{}", parent.trim_end_matches('/'), name)
}

/// Returns the path as a string with trailing slashes stripped. Roots like
/// "//" collapse to "/" so `join()` never sees an empty parent and produces
/// a child with a spurious leading slash.
fn path_str(p: &Path) -> Option<&str> {
    let s = p.to_str()?;
    let stripped = s.trim_end_matches('/');
    if stripped.is_empty() && !s.is_empty() {
        Some("/")
    } else {
        Some(stripped)
    }
}

pub fn now_ns() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as i64)
        .unwrap_or(0)
}

fn mtime_ns(meta: &std::fs::Metadata) -> i64 {
    meta.mtime() * 1_000_000_000 + meta.mtime_nsec()
}

pub struct SearchIndex {
    conn: Connection,
}

const SCHEMA: &str = "
    CREATE TABLE IF NOT EXISTS meta (key TEXT PRIMARY KEY, value TEXT);
    CREATE TABLE IF NOT EXISTS entries (
        id     INTEGER PRIMARY KEY,
        path   TEXT NOT NULL UNIQUE,
        parent TEXT NOT NULL,
        name   TEXT NOT NULL,
        is_dir INTEGER NOT NULL,
        mtime  INTEGER NOT NULL DEFAULT 0
    );
    CREATE INDEX IF NOT EXISTS entries_parent ON entries(parent);
    CREATE VIRTUAL TABLE IF NOT EXISTS entries_fts USING fts5(
        name, content='entries', content_rowid='id', tokenize='trigram'
    );
    CREATE TRIGGER IF NOT EXISTS entries_ai AFTER INSERT ON entries BEGIN
        INSERT INTO entries_fts(rowid, name) VALUES (new.id, new.name);
    END;
    CREATE TRIGGER IF NOT EXISTS entries_ad AFTER DELETE ON entries BEGIN
        INSERT INTO entries_fts(entries_fts, rowid, name) VALUES ('delete', old.id, old.name);
    END;
    CREATE TRIGGER IF NOT EXISTS entries_au AFTER UPDATE OF name ON entries BEGIN
        INSERT INTO entries_fts(entries_fts, rowid, name) VALUES ('delete', old.id, old.name);
        INSERT INTO entries_fts(rowid, name) VALUES (new.id, new.name);
    END;
";

const DROP_ALL: &str = "
    DROP TRIGGER IF EXISTS entries_ai;
    DROP TRIGGER IF EXISTS entries_ad;
    DROP TRIGGER IF EXISTS entries_au;
    DROP TABLE IF EXISTS entries_fts;
    DROP TABLE IF EXISTS entries;
    DROP TABLE IF EXISTS meta;
    DROP TABLE IF EXISTS file_index;
";

const UPSERT: &str = "
    INSERT INTO entries (path, parent, name, is_dir, mtime) VALUES (?1, ?2, ?3, ?4, ?5)
    ON CONFLICT(path) DO UPDATE SET
        name = excluded.name, is_dir = excluded.is_dir, mtime = excluded.mtime
    WHERE entries.name != excluded.name
       OR entries.is_dir != excluded.is_dir
       OR entries.mtime != excluded.mtime";

impl SearchIndex {
    pub fn db_path() -> PathBuf {
        dirs::cache_dir()
            .unwrap_or_else(|| PathBuf::from("/tmp"))
            .join("flux")
            .join("index.db")
    }

    pub fn open_or_create() -> Result<Self> {
        Self::open_at(&Self::db_path())
    }

    pub fn open_at(db_path: &Path) -> Result<Self> {
        if let Some(dir) = db_path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let mut conn = Connection::open(db_path)?;
        conn.busy_timeout(Duration::from_secs(5))?;
        conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA synchronous = NORMAL;
             PRAGMA temp_store = MEMORY;
             PRAGMA mmap_size = 268435456;",
        )?;
        Self::migrate(&mut conn)?;
        Ok(Self { conn })
    }

    #[doc(hidden)]
    #[allow(dead_code)]
    pub fn conn(&self) -> &Connection {
        &self.conn
    }

    fn migrate(conn: &mut Connection) -> Result<()> {
        let version: i64 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
        if version == SCHEMA_VERSION {
            return Ok(());
        }
        let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let version: i64 = tx.query_row("PRAGMA user_version", [], |r| r.get(0))?;
        if version != SCHEMA_VERSION {
            tx.execute_batch(DROP_ALL)?;
            tx.execute_batch(SCHEMA)?;
            tx.execute_batch(&format!("PRAGMA user_version = {SCHEMA_VERSION};"))?;
        }
        tx.commit()?;
        Ok(())
    }

    fn meta(&self, key: &str) -> Option<String> {
        self.conn
            .query_row("SELECT value FROM meta WHERE key = ?1", [key], |r| r.get(0))
            .ok()
    }

    /// Returns true when the initial baseline crawl has finished.
    pub fn is_ready(&self) -> bool {
        self.meta("full_scan_done").as_deref() == Some("1")
    }

    #[allow(dead_code)]
    pub fn len(&self) -> usize {
        self.conn
            .query_row("SELECT count(*) FROM entries", [], |r| r.get::<_, i64>(0))
            .unwrap_or(0) as usize
    }

    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    #[allow(dead_code)]
    pub fn query(&self, pattern: &str, limit: usize) -> Result<Vec<PathBuf>> {
        self.query_in(pattern, None, limit)
    }

    /// Executes an FTS5 trigram match, optionally bounded to a directory prefix.
    pub fn query_in(
        &self,
        pattern: &str,
        scope: Option<&Path>,
        limit: usize,
    ) -> Result<Vec<PathBuf>> {
        let fts_query = format!("\"{}\"", pattern.replace('"', "\"\""));
        let (lo, hi) = match scope.and_then(path_str) {
            Some(s) => {
                let s = s.trim_end_matches('/');
                (format!("{s}/"), format!("{s}0"))
            }
            None => (String::new(), "\u{10FFFF}".to_string()),
        };
        let mut stmt = self.conn.prepare_cached(
            "SELECT e.path FROM entries_fts JOIN entries e ON e.id = entries_fts.rowid
             WHERE entries_fts MATCH ?1 AND e.path >= ?2 AND e.path < ?3
             LIMIT ?4",
        )?;
        let rows = stmt.query_map(params![fts_query, lo, hi, limit as i64], |r| {
            r.get::<_, String>(0).map(PathBuf::from)
        })?;
        Ok(rows.flatten().collect())
    }

    /// Executes a fuzzy match over indexed entry names, optionally bounded to a
    /// directory prefix. Returns up to `limit` paths ordered by descending score.
    pub fn query_fuzzy(
        &self,
        pattern: &str,
        scope: Option<&Path>,
        limit: usize,
    ) -> Result<Vec<PathBuf>> {
        use nucleo::pattern::{CaseMatching, Normalization, Pattern};
        use nucleo::Utf32Str;

        let parsed = Pattern::parse(pattern, CaseMatching::Ignore, Normalization::Smart);
        let mut matcher = nucleo::Matcher::new(nucleo::Config::DEFAULT);

        let (lo, hi) = match scope.and_then(path_str) {
            Some(s) => {
                let s = s.trim_end_matches('/');
                (format!("{s}/"), format!("{s}0"))
            }
            None => (String::new(), "\u{10FFFF}".to_string()),
        };

        let mut stmt = self
            .conn
            .prepare_cached("SELECT path, name FROM entries WHERE path >= ?1 AND path < ?2")?;
        let rows = stmt.query_map(params![lo, hi], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
        })?;

        let mut hits: Vec<(u32, PathBuf)> = Vec::new();
        let mut buf: Vec<char> = Vec::with_capacity(256);
        for row in rows.flatten() {
            let (path, name) = row;
            buf.clear();
            let utf32 = Utf32Str::new(&name, &mut buf);
            if let Some(score) = parsed.score(utf32, &mut matcher) {
                hits.push((score, PathBuf::from(path)));
            }
        }
        hits.sort_by_key(|(s, _)| std::cmp::Reverse(*s));
        hits.truncate(limit);
        Ok(hits.into_iter().map(|(_, p)| p).collect())
    }

    #[allow(dead_code)]
    pub fn delete_paths(&mut self, paths: &[String]) -> Result<()> {
        let tx = self.conn.transaction()?;
        for p in paths {
            remove_tree(&tx, p)?;
        }
        tx.commit()?;
        Ok(())
    }
}

/// Range query deletes the directory and all children without SQL `LIKE` wildcard ambiguities.
pub fn remove_tree(conn: &Connection, path: &str) -> Result<()> {
    let p = path.trim_end_matches('/');
    if p.is_empty() {
        return Ok(());
    }
    conn.prepare_cached("DELETE FROM entries WHERE path = ?1 OR (path >= ?2 AND path < ?3)")?
        .execute(params![p, format!("{p}/"), format!("{p}0")])?;
    Ok(())
}

fn upsert(conn: &Connection, parent: &str, name: &str, is_dir: bool, mtime: i64) -> Result<()> {
    conn.prepare_cached(UPSERT)?.execute(params![
        join(parent, name),
        parent,
        name,
        is_dir as i32,
        mtime
    ])?;
    Ok(())
}

fn set_dir_mtime(conn: &Connection, dir: &str, mtime: i64) -> Result<()> {
    let stored = if now_ns() - mtime < RACY_WINDOW_NS {
        0
    } else {
        mtime
    };
    conn.prepare_cached("UPDATE entries SET mtime = ?2 WHERE path = ?1")?
        .execute(params![dir, stored])?;
    Ok(())
}

fn read_listing(dir: &Path) -> io::Result<(i64, Vec<(String, bool)>)> {
    let mtime = mtime_ns(&std::fs::metadata(dir)?);
    let mut out = Vec::new();
    for entry in std::fs::read_dir(dir)?.flatten() {
        let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
            continue;
        };
        if !should_index(&name) {
            continue;
        }
        let is_dir = match entry.file_type() {
            Ok(t) if t.is_symlink() => std::fs::metadata(entry.path())
                .map(|m| m.is_dir())
                .unwrap_or(false),
            Ok(t) => t.is_dir(),
            Err(_) => false,
        };
        out.push((name, is_dir));
    }
    Ok((mtime, out))
}

fn collect_index_roots() -> Vec<PathBuf> {
    let home = dirs::home_dir();
    let mut roots: Vec<PathBuf> = Vec::new();

    let disks = sysinfo::Disks::new_with_refreshed_list();
    for disk in disks.list() {
        let fs = disk.file_system().to_string_lossy();
        if fs.starts_with("fuse") {
            continue;
        }

        let mp = disk.mount_point();

        // Synthetic FUSE-backed views under /run/user/<uid> (flatpak
        // document portal, GVFS, etc.) mirror real files and would
        // otherwise make every file appear twice in the index.
        if mp.starts_with("/run/user") {
            continue;
        }

        if mp == Path::new("/") || mp == Path::new("/home") {
            continue;
        }
        if let Some(ref h) = home {
            if mp == h.as_path() {
                continue;
            }
        }

        roots.push(mp.to_path_buf());
    }

    if let Some(h) = home {
        roots.push(h);
    }
    roots
}

fn index_subtree(
    conn: &Connection,
    root: &Path,
    should_stop: &dyn Fn() -> bool,
    tick: &mut dyn FnMut(&Connection),
) -> Result<()> {
    let mut visited: std::collections::HashSet<(u64, u64)> = std::collections::HashSet::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        if should_stop() {
            return Ok(());
        }
        if let Ok(meta) = std::fs::metadata(&dir) {
            if !visited.insert((meta.dev(), meta.ino())) {
                continue;
            }
        }
        let Some(dir_s) = path_str(&dir).map(str::to_owned) else {
            continue;
        };
        let Ok((mtime, listing)) = read_listing(&dir) else {
            continue;
        };
        for (name, is_dir) in &listing {
            upsert(conn, &dir_s, name, *is_dir, 0)?;
            tick(conn);
            if *is_dir {
                stack.push(PathBuf::from(join(&dir_s, name)));
            }
        }
        set_dir_mtime(conn, &dir_s, mtime)?;
    }
    Ok(())
}

fn sync_dir(conn: &Connection, dir: &Path, should_stop: &dyn Fn() -> bool) -> Result<()> {
    let Some(dir_s) = path_str(dir).map(str::to_owned) else {
        return Ok(());
    };
    let (mtime, listing) = match read_listing(dir) {
        Ok(v) => v,
        Err(e)
            if e.kind() == io::ErrorKind::NotFound || e.raw_os_error() == Some(libc::ENOTDIR) =>
        {
            return remove_tree(conn, &dir_s);
        }
        Err(_) => return Ok(()),
    };

    let existing: HashMap<String, bool> = {
        let mut stmt = conn.prepare_cached("SELECT name, is_dir FROM entries WHERE parent = ?1")?;
        let rows = stmt.query_map([&dir_s], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)? != 0))
        })?;
        rows.flatten().collect()
    };
    let live: HashMap<&str, bool> = listing.iter().map(|(n, d)| (n.as_str(), *d)).collect();

    for (name, was_dir) in &existing {
        if live.get(name.as_str()) != Some(was_dir) {
            remove_tree(conn, &join(&dir_s, name))?;
        }
    }
    for (name, is_dir) in &listing {
        if existing.get(name) == Some(is_dir) {
            continue;
        }
        upsert(conn, &dir_s, name, *is_dir, 0)?;
        if *is_dir {
            index_subtree(
                conn,
                Path::new(&join(&dir_s, name)),
                should_stop,
                &mut |_| {},
            )?;
        }
    }
    set_dir_mtime(conn, &dir_s, mtime)
}

pub fn full_scan(
    idx: &SearchIndex,
    roots: &[PathBuf],
    should_stop: &dyn Fn() -> bool,
) -> Result<bool> {
    const COMMIT_EVERY: usize = 20_000;
    let conn = &idx.conn;
    conn.execute_batch("BEGIN IMMEDIATE")?;
    conn.execute_batch(DROP_ALL)?;
    conn.execute_batch(SCHEMA)?;
    let mut rows = 0usize;
    let mut all_finished = true;
    for root in roots {
        if should_stop() {
            all_finished = false;
            break;
        }
        let res = index_subtree(conn, root, should_stop, &mut |c| {
            rows += 1;
            if rows.is_multiple_of(COMMIT_EVERY) {
                let _ = c.execute_batch("COMMIT; BEGIN IMMEDIATE");
            }
        });
        if res.is_err() {
            all_finished = false;
        }
    }
    if all_finished {
        conn.execute(
            "INSERT OR REPLACE INTO meta (key, value) VALUES ('full_scan_done', '1')",
            [],
        )?;
    }
    conn.execute_batch("COMMIT")?;
    Ok(all_finished)
}

pub fn delta_scan(
    idx: &SearchIndex,
    roots: &[PathBuf],
    should_stop: &dyn Fn() -> bool,
) -> Result<()> {
    let conn = &idx.conn;
    let dirs: Vec<(String, i64)> = {
        let mut stmt = conn.prepare("SELECT path, mtime FROM entries WHERE is_dir = 1")?;
        let rows = stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))?;
        rows.flatten().collect()
    };

    let mut changed: Vec<PathBuf> = roots.to_vec();
    let mut gone: Vec<String> = Vec::new();
    let fallback_root = roots.first().map(|p| p.as_path()).unwrap_or(Path::new("/"));
    for (path, stored) in dirs {
        if should_stop() {
            return Ok(());
        }
        match std::fs::metadata(&path) {
            Ok(m) if m.is_dir() => {
                if mtime_ns(&m) != stored || stored == 0 {
                    changed.push(PathBuf::from(path));
                }
            }
            Ok(_) => changed.push(
                Path::new(&path)
                    .parent()
                    .unwrap_or(fallback_root)
                    .to_path_buf(),
            ),
            Err(e)
                if e.kind() == io::ErrorKind::NotFound
                    || e.raw_os_error() == Some(libc::ENOTDIR) =>
            {
                gone.push(path)
            }
            Err(_) => {}
        }
    }

    conn.execute_batch("BEGIN IMMEDIATE")?;
    let res = (|| -> Result<()> {
        for p in &gone {
            remove_tree(conn, p)?;
        }
        for d in &changed {
            if should_stop() {
                break;
            }
            sync_dir(conn, d, should_stop)?;
        }
        Ok(())
    })();
    conn.execute_batch(if res.is_ok() { "COMMIT" } else { "ROLLBACK" })?;
    res
}

#[allow(dead_code)]
pub enum Job {
    Created(PathBuf),
    Deleted(PathBuf),
    Moved(PathBuf, PathBuf),
    SyncDir(PathBuf),
    Delta { force: bool },
    Rebuild,
}

pub static ENABLED: AtomicBool = AtomicBool::new(false);
static WORKER: OnceLock<Sender<Job>> = OnceLock::new();
/// Mirror of the `full_scan_done` flag, updated by the worker. `is_ready()`
/// reads this instead of opening SQLite, because the search box calls it on
/// every keystroke.
static INDEX_READY: AtomicBool = AtomicBool::new(false);

pub fn apply_event(
    conn: &Connection,
    roots: &[PathBuf],
    job: &Job,
    stop: &dyn Fn() -> bool,
) -> Result<()> {
    match job {
        Job::Created(p) => {
            if !roots.iter().any(|r| is_indexable(r, p)) {
                return Ok(());
            }
            let (Some(parent), Some(name)) = (
                p.parent().and_then(path_str),
                p.file_name().and_then(|n| n.to_str()),
            ) else {
                return Ok(());
            };
            match std::fs::metadata(p) {
                Ok(m) => {
                    upsert(conn, parent, name, m.is_dir(), 0)?;
                    if m.is_dir() {
                        index_subtree(conn, p, stop, &mut |_| {})?;
                    }
                }
                Err(_) => remove_tree(conn, path_str(p).unwrap_or_default())?,
            }
        }
        Job::Deleted(p) => {
            if let Some(s) = path_str(p) {
                remove_tree(conn, s)?;
            }
        }
        Job::Moved(from, to) => {
            apply_event(conn, roots, &Job::Deleted(from.clone()), stop)?;
            apply_event(conn, roots, &Job::Created(to.clone()), stop)?;
        }
        Job::SyncDir(d) => {
            if roots.iter().any(|r| d == r || is_indexable(r, d)) {
                sync_dir(conn, d, stop)?;
            }
        }
        Job::Delta { .. } | Job::Rebuild => {}
    }
    Ok(())
}

pub fn worker_loop(rx: mpsc::Receiver<Job>, db_path: PathBuf, roots: Vec<PathBuf>) {
    let Ok(mut idx) = SearchIndex::open_at(&db_path) else {
        return;
    };
    let stop = || !ENABLED.load(Ordering::Relaxed);
    let mut last_delta: Option<Instant> = None;

    let run_delta = |idx: &SearchIndex, last: &mut Option<Instant>| {
        let _ = delta_scan(idx, &roots, &stop);
        *last = Some(Instant::now());
    };
    let build = |idx: &SearchIndex| {
        if full_scan(idx, &roots, &stop).unwrap_or(false) {
            INDEX_READY.store(true, Ordering::Relaxed);
        }
    };

    if idx.is_ready() {
        INDEX_READY.store(true, Ordering::Relaxed);
        run_delta(&idx, &mut last_delta);
    } else {
        build(&idx);
        last_delta = Some(Instant::now());
    }

    loop {
        let first = match rx.recv_timeout(DELTA_INTERVAL) {
            Ok(j) => j,
            Err(RecvTimeoutError::Timeout) => {
                if ENABLED.load(Ordering::Relaxed) {
                    run_delta(&idx, &mut last_delta);
                }
                continue;
            }
            Err(RecvTimeoutError::Disconnected) => return,
        };
        if !ENABLED.load(Ordering::Relaxed) {
            while rx.try_recv().is_ok() {}
            continue;
        }

        let mut jobs = vec![first];
        while jobs.len() < MAX_BATCH {
            match rx.recv_timeout(DEBOUNCE) {
                Ok(j) => jobs.push(j),
                Err(_) => break,
            }
        }

        let mut want_delta = false;
        let mut force_delta = false;
        let mut rebuild = false;
        for j in &jobs {
            match j {
                Job::Delta { force } => {
                    want_delta = true;
                    force_delta |= *force;
                }
                Job::Rebuild => rebuild = true,
                _ => {}
            }
        }

        if rebuild {
            build(&idx);
            last_delta = Some(Instant::now());
            continue;
        }
        if !idx.is_ready() {
            build(&idx);
            last_delta = Some(Instant::now());
            continue;
        }

        if let Ok(tx) = idx.conn.transaction() {
            for j in &jobs {
                if apply_event(&tx, &roots, j, &stop).is_err() {
                    break;
                }
            }
            let _ = tx.commit();
        }
        let due = last_delta.is_none_or(|t| t.elapsed() >= DELTA_MIN_GAP);
        if want_delta && (force_delta || due) {
            run_delta(&idx, &mut last_delta);
        }
    }
}

fn ensure_worker() -> Option<&'static Sender<Job>> {
    if let Some(tx) = WORKER.get() {
        return Some(tx);
    }
    let roots = collect_index_roots();
    if roots.is_empty() {
        return None;
    }
    let (tx, rx) = mpsc::channel();
    // Only publish the sender after the thread is up. If spawn fails and we
    // still publish, every notify_* call queues forever into a channel
    // nobody drains.
    let spawned = std::thread::Builder::new()
        .name("flux-indexer".into())
        .spawn(move || worker_loop(rx, SearchIndex::db_path(), roots));
    if spawned.is_err() {
        return None;
    }
    Some(WORKER.get_or_init(|| tx))
}

fn send(job: Job) {
    if !ENABLED.load(Ordering::Relaxed) {
        return;
    }
    if let Some(tx) = ensure_worker() {
        let _ = tx.send(job);
    }
}

pub fn set_enabled(enabled: bool) {
    ENABLED.store(enabled, Ordering::Relaxed);
    if enabled {
        if let Some(tx) = ensure_worker() {
            let _ = tx.send(Job::Delta { force: true });
        }
    }
}

pub fn build_home_index_async() {
    if crate::utils::load_config().ui.enable_file_indexing {
        set_enabled(true);
    }
}

pub fn notify_created(path: &Path) {
    send(Job::Created(path.to_path_buf()));
}

pub fn notify_deleted(path: &Path) {
    send(Job::Deleted(path.to_path_buf()));
}

pub fn notify_moved(from: &Path, to: &Path) {
    send(Job::Moved(from.to_path_buf(), to.to_path_buf()));
}

#[allow(dead_code)]
pub fn notify_dir_changed(dir: &Path) {
    send(Job::SyncDir(dir.to_path_buf()));
}

pub fn request_delta_scan() {
    send(Job::Delta { force: false });
}

#[allow(dead_code)]
pub fn request_rebuild() {
    send(Job::Rebuild);
}

pub fn prune_paths(paths: Vec<String>) {
    for p in paths {
        send(Job::Deleted(PathBuf::from(p)));
    }
}

/// True when indexing is enabled and the baseline crawl finished. Reads a
/// cached flag so this can be called cheaply from the UI thread.
pub fn is_ready() -> bool {
    ENABLED.load(Ordering::Relaxed) && INDEX_READY.load(Ordering::Relaxed)
}
