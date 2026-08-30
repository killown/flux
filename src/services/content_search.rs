use crate::model::{AppMsg, FluxApp};
use crate::services::constants::MAX_CONTENT_SEARCH_RESULTS;
use crate::ui::paste_ops::NEXT_TASK_ID;
use crate::utils::search::{parse_size_filter, SizeOp};
use aho_corasick::AhoCorasick;
use gtk::gio::prelude::*;
use ignore::{ParallelVisitor, ParallelVisitorBuilder, WalkBuilder, WalkState};
use relm4::prelude::*;
use relm4::AsyncComponentSender;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;

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

        let walker = builder
            .filter_entry(|entry| {
                let path = entry.path();
                let s = path.to_string_lossy();
                !s.contains("/proc/")
                    && !s.contains("/sys/")
                    && !s.contains("/dev/")
                    && !s.contains("/dosdevices/")
                    && !s.contains("/Prefixes/")
                    && !s.contains("/compatdata/")
                    && !s.contains("/drive_c/")
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

                // ---- Extension filter ----
                if let Some(ref exts) = self.allowed_exts {
                    let file_ext = path
                        .extension()
                        .and_then(|e| e.to_str())
                        .map(|s| s.to_lowercase())
                        .unwrap_or_default();
                    if !exts.is_empty() && !exts.contains(&file_ext) {
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
                let mut reader = BufReader::new(file);
                // Reuse a single buffer across all lines, zero allocation per line.
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

                    // aho-corasick ascii_case_insensitive search, no lowercase alloc.
                    let matched = self
                        .matcher
                        .as_ref()
                        .map(|m| m.is_match(buf.as_bytes()))
                        .unwrap_or(false);

                    if matched {
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
