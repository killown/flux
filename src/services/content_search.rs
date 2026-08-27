use crate::model::{AppMsg, FluxApp};
use crate::services::constants::MAX_CONTENT_SEARCH_RESULTS;
use crate::ui::paste_ops::NEXT_TASK_ID;
use crate::utils::search::{parse_size_filter, SizeOp};
use gtk::gio::prelude::*;
use ignore::WalkBuilder;
use relm4::prelude::*;
use relm4::AsyncComponentSender;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::sync::atomic::Ordering;

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
    let term_lc = clean_term.to_lowercase();
    let load_id = app.load_id.clone();
    let show_hidden = app.show_hidden;

    // Parse extension filter once, outside the walk.
    let allowed_exts: Option<Vec<String>> = ext_filter.as_ref().map(|s| {
        s.split(',')
            .map(|part| part.trim().to_lowercase())
            .filter(|s| !s.is_empty())
            .collect()
    });

    relm4::spawn_blocking(move || {
        let mut count: usize = 0;
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
            .build();

        for result in walker {
            if load_id.load(Ordering::Acquire) != session_id
                || cancellable.is_cancelled()
                || count >= MAX_CONTENT_SEARCH_RESULTS
            {
                break;
            }

            let entry = match result {
                Ok(e) => e,
                Err(_) => continue,
            };

            // Only inspect regular files
            if !entry.file_type().is_some_and(|ft| ft.is_file()) {
                continue;
            }

            let path = entry.into_path();

            // ---- Size Filter ----
            if let Some(ref op) = size_op {
                let metadata = match std::fs::metadata(&path) {
                    Ok(m) => m,
                    Err(_) => continue,
                };
                let size = metadata.len();
                let size_match = match op {
                    SizeOp::Gt(v) => size > *v,
                    SizeOp::Lt(v) => size < *v,
                    SizeOp::Range(l, r) => size >= *l && size <= *r,
                };
                if !size_match {
                    continue;
                }
            }

            // ---- Extension filter ----
            if let Some(ref exts) = allowed_exts {
                let file_ext = path
                    .extension()
                    .and_then(|e| e.to_str())
                    .map(|s| s.to_lowercase())
                    .unwrap_or_default();
                if !exts.is_empty() && !exts.contains(&file_ext) {
                    continue;
                }
            }

            if term_lc.is_empty() {
                sender.input(AppMsg::ContentSearchResult {
                    path: path.clone(),
                    line: "Matched Filter".to_string(),
                    line_number: 0,
                    session: session_id,
                });
                count += 1;
                continue;
            }

            let file = match File::open(&path) {
                Ok(f) => f,
                Err(_) => continue,
            };
            let reader = BufReader::new(file);

            for (line_number, line_result) in reader.lines().enumerate() {
                if load_id.load(Ordering::Acquire) != session_id || cancellable.is_cancelled() {
                    break;
                }
                let line = match line_result {
                    Ok(l) => l,
                    Err(_) => break,
                };

                if line.to_lowercase().contains(&term_lc)
                    && load_id.load(Ordering::Acquire) == session_id
                    && !cancellable.is_cancelled()
                {
                    sender.input(AppMsg::ContentSearchResult {
                        path: path.clone(),
                        line: line.trim().to_string(),
                        line_number: line_number + 1,
                        session: session_id,
                    });
                    count += 1;
                    break; // Matches first line hit per file to keep it blazing fast
                }
            }
        }

        if !cancellable.is_cancelled() && load_id.load(Ordering::Acquire) == session_id {
            sender.input(AppMsg::ContentSearchDone {
                session: session_id,
            });
        }
    });
}
