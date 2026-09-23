use crate::model::{AppMsg, FluxApp};
use crate::services::constants::MAX_CONTENT_SEARCH_RESULTS;
use crate::ui::paste_ops::NEXT_TASK_ID;
use crate::utils::search::{parse_size_filter, SizeOp};
use aho_corasick::AhoCorasick;
use gtk::gio::prelude::*;
use ignore::{ParallelVisitor, ParallelVisitorBuilder, WalkBuilder, WalkState};
use memmap2::Mmap;
use relm4::prelude::*;
use relm4::AsyncComponentSender;
use std::collections::HashSet;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::os::unix::fs::MetadataExt;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, OnceLock};

/// Static AhoCorasick matcher for forbidden system and virtual paths.
static FORBIDDEN_PATHS: OnceLock<AhoCorasick> = OnceLock::new();

fn forbidden_matcher() -> &'static AhoCorasick {
    FORBIDDEN_PATHS.get_or_init(|| {
        AhoCorasick::builder()
            .build([
                b"/proc/".as_slice(),
                b"/sys/".as_slice(),
                b"/dev/".as_slice(),
                b"/run/".as_slice(),
                b"/var/run/".as_slice(),
                b"/dosdevices/".as_slice(),
                b"/Prefixes/".as_slice(),
                b"/compatdata/".as_slice(),
                b"/drive_c/".as_slice(),
            ])
            .expect("valid forbidden path patterns")
    })
}

pub fn start_content_search(
    app: &mut FluxApp,
    term: String,
    ext_filter: Option<String>,
    sender: AsyncComponentSender<FluxApp>,
) {
    if term.trim().is_empty() {
        return;
    }

    let (size_op, clean_term) = if let Some((op, rest)) = parse_size_filter(&term) {
        (Some(op), rest)
    } else {
        (None, term.clone())
    };

    if clean_term.trim().is_empty() && size_op.is_none() {
        return;
    }

    // Cancel any previous search
    if let Some(cancellable) = app.content_search_cancellable.take() {
        cancellable.cancel();
    }
    app.is_content_searching = true;
    app.is_loading = true;
    app.files.clear();
    app.filter.clear();

    // Force and save list mode layout for content search results snippet display
    if !app.search_saved_layout {
        app.saved_list_mode = app.is_list_mode;
        app.saved_max_columns = app.files.view.max_columns();
        app.search_saved_layout = true;
    }
    app.is_list_mode = true;
    app.files.view.set_min_columns(1);
    app.files.view.set_max_columns(1);
    app.sync_list_mode();

    let cancellable = gtk::gio::Cancellable::new();
    app.content_search_cancellable = Some(cancellable.clone());

    let session_id = NEXT_TASK_ID.fetch_add(1, Ordering::Relaxed);
    app.load_id.store(session_id, Ordering::SeqCst);

    let current_dir = app.current_path.clone();
    let term_lc = Arc::new(clean_term.to_lowercase());

    // Build the case-insensitive matcher once, each visitor thread clones the Arc.
    let matcher: Option<Arc<AhoCorasick>> = if !term_lc.is_empty() {
        AhoCorasick::builder()
            .ascii_case_insensitive(true)
            .build([term_lc.as_str()])
            .ok()
            .map(Arc::new)
    } else {
        None
    };
    let load_id = app.load_id.clone();
    let show_hidden = app.show_hidden;

    // Parse extension filter once, outside the walk.
    let allowed_exts: Option<Arc<Vec<String>>> = ext_filter.as_ref().map(|s| {
        Arc::new(
            s.split(',')
                .map(|part| part.trim().to_lowercase())
                .filter(|s| !s.is_empty())
                .collect(),
        )
    });

    relm4::spawn_blocking(move || {
        let mut builder = WalkBuilder::new(&current_dir);
        builder
            .hidden(!show_hidden)
            .parents(true)
            .ignore(true)
            .git_ignore(true)
            .git_global(true)
            .git_exclude(true)
            .follow_links(true)
            .same_file_system(false);

        let forbidden = forbidden_matcher();
        let walker = builder
            .filter_entry(move |entry| {
                let bytes = crate::utils::osstr_to_bytes(entry.path().as_os_str());
                !forbidden.is_match(bytes)
            })
            .build_parallel();

        let count = Arc::new(AtomicUsize::new(0));

        struct ContentVisitor {
            sender: AsyncComponentSender<FluxApp>,
            load_id: Arc<AtomicU64>,
            session_id: u64,
            cancellable: gtk::gio::Cancellable,
            count: Arc<AtomicUsize>,
            size_op: Option<SizeOp>,
            allowed_exts: Option<Arc<Vec<String>>>,
            term_lc: Arc<String>,
            matcher: Option<Arc<AhoCorasick>>,
            visited_inodes: HashSet<(u64, u64)>,
        }

        impl ParallelVisitor for ContentVisitor {
            fn visit(&mut self, result: Result<ignore::DirEntry, ignore::Error>) -> WalkState {
                if self.load_id.load(Ordering::Acquire) != self.session_id
                    || self.cancellable.is_cancelled()
                    || self.count.load(Ordering::Relaxed) >= MAX_CONTENT_SEARCH_RESULTS
                {
                    return WalkState::Quit;
                }

                let entry = match result {
                    Ok(e) => e,
                    Err(_) => return WalkState::Continue,
                };

                // Only inspect regular files
                if !entry.file_type().is_some_and(|ft| ft.is_file()) {
                    return WalkState::Continue;
                }

                // ---- Extension filter ----
                if let Some(ref exts) = self.allowed_exts {
                    let file_ext = entry
                        .path()
                        .extension()
                        .and_then(|e| e.to_str())
                        .map(|s| s.to_lowercase())
                        .unwrap_or_default();
                    if !exts.is_empty() && !exts.contains(&file_ext) {
                        return WalkState::Continue;
                    }
                }

                if entry.path_is_symlink() {
                    if let Ok(meta) = entry.metadata() {
                        let key = (meta.dev(), meta.ino());
                        if !self.visited_inodes.insert(key) {
                            return WalkState::Continue;
                        }
                    }
                }

                let path = entry.into_path();

                // ---- Size Filter ----
                if let Some(ref op) = self.size_op {
                    let metadata = match std::fs::metadata(&path) {
                        Ok(m) => m,
                        Err(_) => return WalkState::Continue,
                    };
                    let size = metadata.len();
                    let size_match = match op {
                        SizeOp::Gt(v) => size > *v,
                        SizeOp::Lt(v) => size < *v,
                        SizeOp::Range(l, r) => size >= *l && size <= *r,
                    };
                    if !size_match {
                        return WalkState::Continue;
                    }
                }

                if self.term_lc.is_empty() {
                    let curr = self.count.fetch_add(1, Ordering::Relaxed);
                    if curr < MAX_CONTENT_SEARCH_RESULTS {
                        self.sender.input(AppMsg::ContentSearchResult {
                            path,
                            line: "Matched Filter".to_string(),
                            line_number: 0,
                            session: self.session_id,
                        });
                    }
                    return WalkState::Continue;
                }

                let file = match File::open(&path) {
                    Ok(f) => f,
                    Err(_) => return WalkState::Continue,
                };

                let matcher = match self.matcher.as_ref() {
                    Some(m) => m,
                    None => return WalkState::Continue,
                };

                // Fast path: memory-mapped buffer search
                if let Ok(mmap) = unsafe { Mmap::map(&file) } {
                    let bytes = &mmap[..];
                    if bytes.is_empty() {
                        return WalkState::Continue;
                    }

                    // Early binary bail-out
                    let inspect_len = bytes.len().min(1024);
                    if bytes[..inspect_len].contains(&0) {
                        return WalkState::Continue;
                    }

                    if let Some(m) = matcher.find(bytes) {
                        let match_start = m.start();
                        // Find line boundaries around the match
                        let line_start = bytes[..match_start]
                            .iter()
                            .rposition(|&b| b == b'\n')
                            .map(|idx| idx + 1)
                            .unwrap_or(0);

                        let line_end = bytes[m.end()..]
                            .iter()
                            .position(|&b| b == b'\n')
                            .map(|idx| m.end() + idx)
                            .unwrap_or(bytes.len())
                            .min(line_start + 1024);

                        let line_number =
                            bytes[..line_start].iter().filter(|&&b| b == b'\n').count() + 1;

                        let line_slice = &bytes[line_start..line_end];
                        let line = String::from_utf8_lossy(line_slice).trim().to_string();

                        let curr = self.count.fetch_add(1, Ordering::Relaxed);
                        if curr < MAX_CONTENT_SEARCH_RESULTS {
                            self.sender.input(AppMsg::ContentSearchResult {
                                path,
                                line,
                                line_number,
                                session: self.session_id,
                            });
                        }
                    }
                    return WalkState::Continue;
                }

                // Fallback for zero-byte or unmappable files
                let mut reader = BufReader::new(file);
                let mut buf = String::new();
                let mut line_number: usize = 0;

                loop {
                    if self.load_id.load(Ordering::Acquire) != self.session_id
                        || self.cancellable.is_cancelled()
                        || self.count.load(Ordering::Relaxed) >= MAX_CONTENT_SEARCH_RESULTS
                    {
                        break;
                    }
                    buf.clear();
                    match reader.read_line(&mut buf) {
                        Ok(0) | Err(_) => break, // EOF or read error
                        Ok(_) => {}
                    }
                    line_number += 1;

                    if matcher.is_match(buf.as_bytes()) {
                        let curr = self.count.fetch_add(1, Ordering::Relaxed);
                        if curr < MAX_CONTENT_SEARCH_RESULTS {
                            self.sender.input(AppMsg::ContentSearchResult {
                                path: path.clone(),
                                line: buf.trim().to_string(),
                                line_number,
                                session: self.session_id,
                            });
                        }
                        break; // First line hit per file only
                    }
                }

                WalkState::Continue
            }
        }

        struct ContentVisitorBuilder {
            sender: AsyncComponentSender<FluxApp>,
            load_id: Arc<AtomicU64>,
            session_id: u64,
            cancellable: gtk::gio::Cancellable,
            count: Arc<AtomicUsize>,
            size_op: Option<SizeOp>,
            allowed_exts: Option<Arc<Vec<String>>>,
            term_lc: Arc<String>,
            matcher: Option<Arc<AhoCorasick>>,
        }

        impl<'s> ParallelVisitorBuilder<'s> for ContentVisitorBuilder {
            fn build(&mut self) -> Box<dyn ParallelVisitor + 's> {
                Box::new(ContentVisitor {
                    sender: self.sender.clone(),
                    load_id: self.load_id.clone(),
                    session_id: self.session_id,
                    cancellable: self.cancellable.clone(),
                    count: self.count.clone(),
                    size_op: self.size_op.clone(),
                    allowed_exts: self.allowed_exts.clone(),
                    term_lc: self.term_lc.clone(),
                    matcher: self.matcher.clone(),
                    visited_inodes: HashSet::new(),
                })
            }
        }

        let mut visitor_builder = ContentVisitorBuilder {
            sender: sender.clone(),
            load_id: load_id.clone(),
            session_id,
            cancellable: cancellable.clone(),
            count,
            size_op,
            allowed_exts,
            term_lc,
            matcher,
        };

        walker.visit(&mut visitor_builder);

        if !cancellable.is_cancelled() && load_id.load(Ordering::Acquire) == session_id {
            sender.input(AppMsg::ContentSearchDone {
                session: session_id,
            });
        }
    });
}
