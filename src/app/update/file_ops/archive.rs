use crate::domain::model::{AppMsg, ArchiveMsg, FluxApp, LoadingMsg, TasksMsg, WindowMsg};
use relm4::prelude::*;
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;

/// Raises the flag on drop so the progress poller stops even if the extractor panics.
struct ExtractDone(std::sync::Arc<std::sync::atomic::AtomicBool>);

impl Drop for ExtractDone {
    fn drop(&mut self) {
        self.0.store(true, Ordering::Relaxed);
    }
}

impl FluxApp {
    pub fn handle_extract_archive(&self, sender: &AsyncComponentSender<Self>) {
        crate::hit!("handle_extract_archive");
        let uri = self.current_path.to_string_lossy().to_string();
        let Some((archive_path, _)) = crate::infra::archive::parse_archive_uri(&uri) else {
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
        // `create_dir` is atomic, so the directory we end up with is always ours
        // and the `remove_dir_all` on failure can never hit someone else's folder.
        loop {
            match std::fs::create_dir(&dest) {
                Ok(()) => break,
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                    dest = parent.join(format!("{}_{}", base_name, counter));
                    counter += 1;
                }
                Err(e) => {
                    sender.input(AppMsg::Window(WindowMsg::ShowToast(format!(
                        "Extract failed: {e}"
                    ))));
                    return;
                }
            }
        }

        let task_id =
            crate::app::update::file_ops::paste::NEXT_TASK_ID.fetch_add(1, Ordering::Relaxed);
        let cancellable = gtk::gio::Cancellable::new();
        let label = format!(
            "Extracting {}",
            archive_path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
        );

        sender.input(AppMsg::Tasks(TasksMsg::TaskProgress {
            id: task_id,
            label: label.clone(),
            current: 0,
            total: 0,
            total_items: 1,
            cancellable: cancellable.clone(),
        }));

        let password = self.cached_archive_passwords.get(&archive_path).cloned();
        let s = sender.clone();

        // ── Polling thread: reads dest dir size every 150 ms ─────────────────
        let dest_poll = dest.clone();
        let label_poll = label.clone();
        let cancellable_poll = cancellable.clone();
        let s_poll = s.clone();
        let done_flag = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let done_flag_extractor = done_flag.clone();
        let total_shared = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0));
        let total_poll = total_shared.clone();

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
                s_poll.input(AppMsg::Tasks(TasksMsg::TaskProgress {
                    id: task_id,
                    label: label_poll.clone(),
                    current,
                    total: total_poll.load(std::sync::atomic::Ordering::Relaxed),
                    total_items: 1,
                    cancellable: cancellable_poll.clone(),
                }));
                std::thread::sleep(std::time::Duration::from_millis(150));
            }
        });

        // ── Extraction thread ─────────────────────────────────────────────────
        relm4::spawn_blocking(move || {
            // Raises the flag on any exit from this closure, panics included.
            let done = ExtractDone(done_flag_extractor);
            let total_bytes =
                crate::infra::archive::archive_total_bytes(&archive_path, password.as_deref());
            total_shared.store(total_bytes, Ordering::Relaxed);

            let result = crate::infra::archive::extract_archive_to_dir(
                &archive_path,
                &dest,
                password.as_deref(),
            );

            // Stop the poller before reporting completion.
            drop(done);

            s.input(AppMsg::Tasks(TasksMsg::TaskProgress {
                id: task_id,
                label: label.clone(),
                current: total_bytes,
                total: total_bytes,
                total_items: 1,
                cancellable: cancellable.clone(),
            }));
            s.input(AppMsg::Tasks(TasksMsg::TaskCompleted(task_id)));

            match result {
                Ok(()) => {
                    s.input(AppMsg::Window(WindowMsg::ShowToast(format!(
                        "Extracted to {}",
                        dest.display()
                    ))));
                    s.input(AppMsg::Loading(LoadingMsg::InvalidateCacheAndNavigate(
                        dest,
                    )));
                }
                Err(e) => {
                    let _ = std::fs::remove_dir_all(&dest);
                    s.input(AppMsg::Window(WindowMsg::ShowToast(format!(
                        "Extract failed: {e}"
                    ))));
                }
            }
        });
    }

    /// Extracts an archive nested inside another archive (`a.zip` containing
    /// `b.zip`) next to the outer archive, then enters the extracted copy.
    pub fn handle_extract_nested_archive(
        &mut self,
        archive_path: PathBuf,
        inner_path: String,
        sender: &AsyncComponentSender<Self>,
    ) {
        crate::hit!("handle_extract_nested_archive");
        let Some(dest_dir) = archive_path.parent().map(Path::to_path_buf) else {
            return;
        };

        let key = self.cache_key(&dest_dir);
        self.folder_cache.remove(&key);

        let entry_name = Path::new(&inner_path)
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        let parent_prefix = Path::new(&inner_path)
            .parent()
            .and_then(|p| p.to_str())
            .unwrap_or("")
            .to_string();

        let task_id =
            crate::app::update::file_ops::paste::NEXT_TASK_ID.fetch_add(1, Ordering::Relaxed);
        let cancellable = gtk::gio::Cancellable::new();
        let label = format!("Extracting {entry_name}");
        sender.input(AppMsg::Tasks(TasksMsg::TaskProgress {
            id: task_id,
            label: label.clone(),
            current: 0,
            total: 0,
            total_items: 1,
            cancellable: cancellable.clone(),
        }));

        let password = self.cached_archive_passwords.get(&archive_path).cloned();
        let s = sender.clone();

        relm4::spawn_blocking(move || {
            let result = crate::infra::archive::extract_entry_to_unique_file(
                &archive_path,
                &inner_path,
                password.as_deref(),
                &dest_dir,
            );

            s.input(AppMsg::Tasks(TasksMsg::TaskProgress {
                id: task_id,
                label,
                current: 1,
                total: 1,
                total_items: 1,
                cancellable,
            }));
            s.input(AppMsg::Tasks(TasksMsg::TaskCompleted(task_id)));

            match result {
                Ok(dest) => {
                    s.input(AppMsg::Window(WindowMsg::ShowToast(format!(
                        "Extracted to {}",
                        dest.display()
                    ))));
                    s.input(AppMsg::Archive(ArchiveMsg::EnterArchive(dest)));
                }
                Err(crate::infra::archive::ArchiveError::PasswordRequired) => {
                    s.input(AppMsg::Archive(ArchiveMsg::PromptArchivePassword {
                        archive_path,
                        prefix: parent_prefix,
                        wrong_password: false,
                    }));
                }
                Err(crate::infra::archive::ArchiveError::WrongPassword) => {
                    s.input(AppMsg::Archive(ArchiveMsg::PromptArchivePassword {
                        archive_path,
                        prefix: parent_prefix,
                        wrong_password: true,
                    }));
                }
                Err(e) => {
                    s.input(AppMsg::Window(WindowMsg::ShowToast(format!(
                        "Extract failed: {e}"
                    ))));
                }
            }
        });
    }
}
