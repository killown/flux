use crate::model::{AppMsg, FluxApp};
use crate::services::constants;
use crate::utils;
use relm4::prelude::*;
use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::sync::Arc;

impl FluxApp {
    pub fn spawn_thumbnail_loader(
        &self,
        media_tasks: Vec<(String, PathBuf)>,
        current_session: u64,
        sender: AsyncComponentSender<Self>,
    ) {
        let session_arc = self.load_id.clone();

        if media_tasks.is_empty() {
            return;
        }

        // Use Arc<Semaphore> to share across tasks
        let semaphore = Arc::new(tokio::sync::Semaphore::new(
            constants::MAX_THUMBNAIL_THREADS,
        ));

        relm4::spawn(async move {
            let mut handles = Vec::new();

            for (name, media_path) in media_tasks {
                if session_arc.load(Ordering::Acquire) != current_session {
                    break;
                }

                let sem_clone = semaphore.clone();
                let inner_sender = sender.clone();
                let inner_session = session_arc.clone();
                let session_id = current_session;
                let task_name = name.clone();
                let task_path = media_path.clone();

                let handle = tokio::spawn(async move {
                    let _permit = sem_clone.acquire().await.unwrap();

                    let texture = utils::get_or_create_thumbnail(&task_path).await;

                    if inner_session.load(Ordering::Acquire) != session_id {
                        return;
                    }

                    if let Some(texture) = texture {
                        if inner_session.load(Ordering::Acquire) == session_id {
                            inner_sender.input(AppMsg::ThumbnailReady {
                                name: task_name,
                                texture,
                                load_id: session_id,
                            });
                        }
                    }
                });

                handles.push(handle);
            }

            for handle in handles {
                if session_arc.load(Ordering::Acquire) != current_session {
                    handle.abort();
                    break;
                }
                let _ = handle.await;
            }
        });
    }
}
