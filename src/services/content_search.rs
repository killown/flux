use crate::model::{AppMsg, FluxApp};
use crate::utils::helpers::NEXT_TASK_ID;
use adw::gio::prelude::*;
use gtk::gio;
use relm4::prelude::*;
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

    // Cancel any previous search
    if let Some(cancellable) = app.content_search_cancellable.take() {
        cancellable.cancel();
    }
    app.is_content_searching = true;
    app.files.clear();
    app.filter.clear();

    let cancellable = gio::Cancellable::new();
    app.content_search_cancellable = Some(cancellable.clone());

    let session_id = NEXT_TASK_ID.fetch_add(1, Ordering::Relaxed);
    app.load_id.store(session_id, Ordering::SeqCst);

    let current_dir = app.current_path.clone();
    let term_lc = term.to_lowercase();
    let load_id = app.load_id.clone();

    // Parse extension filter once, outside the walk.
    let allowed_exts: Option<Vec<String>> = ext_filter.as_ref().map(|s| {
        s.split(',')
            .map(|part| part.trim().to_lowercase())
            .filter(|s| !s.is_empty())
            .collect()
    });

    relm4::spawn_blocking(move || {
        // Recursive walk function now takes the allowed extensions as a reference.
        fn walk(
            dir: &gio::File,
            term_lc: &str,
            cancellable: &gio::Cancellable,
            session_id: u64,
            sender: &AsyncComponentSender<FluxApp>,
            load_id: &std::sync::atomic::AtomicU64,
            allowed_exts: &Option<Vec<String>>,
        ) {
            if load_id.load(Ordering::Acquire) != session_id || cancellable.is_cancelled() {
                return;
            }

            // Skip system paths that often block
            if let Some(path) = dir.path() {
                let s = path.to_string_lossy();
                if s.starts_with("/proc/") || s.starts_with("/sys/") || s.starts_with("/dev/") {
                    return;
                }
            }

            let enumerator = match dir.enumerate_children(
                "standard::name,standard::type,standard::content-type",
                gio::FileQueryInfoFlags::NONE,
                Some(cancellable),
            ) {
                Ok(e) => e,
                Err(_) => return,
            };

            while let Ok(Some(info)) = enumerator.next_file(Some(cancellable)) {
                if load_id.load(Ordering::Acquire) != session_id || cancellable.is_cancelled() {
                    break;
                }

                let child = dir.child(info.name());
                let file_type = info.file_type();

                // Skip symlinks to avoid cycles
                if file_type == gio::FileType::SymbolicLink {
                    continue;
                }

                if file_type == gio::FileType::Directory {
                    walk(
                        &child,
                        term_lc,
                        cancellable,
                        session_id,
                        sender,
                        load_id,
                        allowed_exts,
                    );
                    continue;
                }

                // Only regular files
                if file_type != gio::FileType::Regular {
                    continue;
                }

                // ---- Extension filter ----
                // If an extension filter is provided, check that the file's extension is in the list.
                if let Some(ref exts) = allowed_exts {
                    if let Some(path) = child.path() {
                        let file_ext = path
                            .extension()
                            .and_then(|e| e.to_str())
                            .map(|s| s.to_lowercase())
                            .unwrap_or_default();
                        if !exts.is_empty() && !exts.contains(&file_ext) {
                            continue; // skip this file
                        }
                    } else {
                        // No path? skip if filter is active (can't determine extension).
                        continue;
                    }
                }

                // Optional MIME filter
                if let Some(content_type) = info.content_type() {
                    let mime = content_type.to_string();
                    if !mime.starts_with("text/") && !mime.contains("json") && !mime.contains("xml")
                    {
                        continue;
                    }
                }

                if let Some(path) = child.path() {
                    // Read directly on the background thread pool.
                    let content = match std::fs::read_to_string(&path) {
                        Ok(c) => c,
                        Err(_) => continue, // Skip unreadable/binary files
                    };

                    for (line_number, line) in content.lines().enumerate() {
                        // Check cancellation before each line
                        if load_id.load(Ordering::Acquire) != session_id
                            || cancellable.is_cancelled()
                        {
                            break;
                        }
                        if line.to_lowercase().contains(term_lc) {
                            if load_id.load(Ordering::Acquire) == session_id
                                && !cancellable.is_cancelled()
                            {
                                sender.input(AppMsg::ContentSearchResult {
                                    path: path.clone(),
                                    line: line.trim().to_string(),
                                    line_number: line_number + 1,
                                    session: session_id,
                                });
                            }
                            break;
                        }
                    }
                }
            }
        }

        walk(
            &gio::File::for_path(&current_dir),
            &term_lc,
            &cancellable,
            session_id,
            &sender,
            &load_id,
            &allowed_exts,
        );

        // Only send completion if still active
        if !cancellable.is_cancelled() && load_id.load(Ordering::Acquire) == session_id {
            sender.input(AppMsg::ContentSearchDone {
                session: session_id,
            });
        }
    });
}
