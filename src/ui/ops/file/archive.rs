use crate::model::{AppMsg, FluxApp};
use relm4::prelude::*;
use std::sync::atomic::Ordering;

impl FluxApp {
    pub fn handle_extract_archive(&self, sender: &AsyncComponentSender<Self>) {
        let uri = self.current_path.to_string_lossy().to_string();
        let Some((archive_path, _)) = crate::services::archive::parse_archive_uri(&uri) else {
            return;
        };
        let stem = archive_path
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        let parent = archive_path
            .parent()
            .unwrap_or(std::path::Path::new("."))
            .to_path_buf();
        let base_name = format!("{}_extracted", stem);
        let mut dest = parent.join(&base_name);
        let mut counter = 2;
        while dest.exists() {
            dest = parent.join(format!("{}_{}", base_name, counter));
            counter += 1;
        }

        if let Err(e) = std::fs::create_dir_all(&dest) {
            sender.input(AppMsg::ShowToast(format!("Extract failed: {e}")));
            return;
        }

        let task_id = crate::ui::ops::paste::NEXT_TASK_ID.fetch_add(1, Ordering::Relaxed);
        let cancellable = gtk::gio::Cancellable::new();
        let label = format!(
            "Extracting {}",
            archive_path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
        );

        let total_bytes = crate::services::archive::archive_total_bytes(
            &archive_path,
            self.cached_archive_password.as_deref(),
        );

        sender.input(AppMsg::TaskProgress {
            id: task_id,
            label: label.clone(),
            current: 0,
            total: total_bytes,
            total_items: 1,
            cancellable: cancellable.clone(),
        });

        let password = self.cached_archive_password.clone();
        let s = sender.clone();

        // ── Polling thread: reads dest dir size every 150 ms ─────────────────
        let dest_poll = dest.clone();
        let label_poll = label.clone();
        let cancellable_poll = cancellable.clone();
        let s_poll = s.clone();
        let done_flag = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let done_flag_extractor = done_flag.clone();

        std::thread::spawn(move || {
            /// Sums regular-file bytes without following symlinks.
            fn dir_size(path: &std::path::Path) -> u64 {
                let Ok(rd) = std::fs::read_dir(path) else {
                    return 0;
                };
                rd.flatten().fold(0u64, |acc, e| match e.file_type() {
                    Ok(t) if t.is_dir() => acc + dir_size(&e.path()),
                    Ok(t) if t.is_file() => acc + e.metadata().map(|m| m.len()).unwrap_or(0),
                    _ => acc,
                })
            }
            loop {
                if done_flag.load(std::sync::atomic::Ordering::Relaxed) {
                    break;
                }
                let current = dir_size(&dest_poll);
                s_poll.input(AppMsg::TaskProgress {
                    id: task_id,
                    label: label_poll.clone(),
                    current,
                    total: total_bytes,
                    total_items: 1,
                    cancellable: cancellable_poll.clone(),
                });
                std::thread::sleep(std::time::Duration::from_millis(150));
            }
        });

        // ── Extraction thread ─────────────────────────────────────────────────
        relm4::spawn_blocking(move || {
            let result = crate::services::archive::extract_archive_to_dir(
                &archive_path,
                &dest,
                password.as_deref(),
            );

            done_flag_extractor.store(true, std::sync::atomic::Ordering::Relaxed);

            s.input(AppMsg::TaskProgress {
                id: task_id,
                label: label.clone(),
                current: total_bytes,
                total: total_bytes,
                total_items: 1,
                cancellable: cancellable.clone(),
            });
            s.input(AppMsg::TaskCompleted(task_id));

            match result {
                Ok(()) => {
                    s.input(AppMsg::ShowToast(format!(
                        "Extracted to {}",
                        dest.display()
                    )));
                    s.input(AppMsg::InvalidateCacheAndNavigate(dest));
                }
                Err(e) => {
                    let _ = std::fs::remove_dir_all(&dest);
                    s.input(AppMsg::ShowToast(format!("Extract failed: {e}")));
                }
            }
        });
    }
}
