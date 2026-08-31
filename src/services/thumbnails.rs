use crate::model::{AppMsg, FluxApp};
use crate::services::constants;
use crate::utils;
use futures::stream::{self, StreamExt};
use relm4::prelude::*;
use std::path::PathBuf;
use std::sync::atomic::Ordering;

impl FluxApp {
    pub fn spawn_thumbnail_loader(
        &self,
        media_tasks: Vec<(u32, PathBuf)>,
        current_session: u64,
        sender: AsyncComponentSender<Self>,
    ) {
        let session_arc = self.load_id.clone();

        if media_tasks.is_empty() {
            return;
        }

        relm4::spawn(async move {
            // Process tasks with bounded concurrency to eliminate task churn
            stream::iter(media_tasks)
                .map(|(grid_idx, media_path)| {
                    let inner_sender = sender.clone();
                    let inner_session = session_arc.clone();
                    let session_id = current_session;

                    async move {
                        if inner_session.load(Ordering::Acquire) != session_id {
                            return;
                        }

                        let texture = utils::get_or_create_thumbnail(&media_path).await;

                        if inner_session.load(Ordering::Acquire) != session_id {
                            return;
                        }

                        if let Some(texture) = texture {
                            inner_sender.input(AppMsg::ThumbnailReady {
                                grid_idx,
                                texture,
                                load_id: session_id,
                            });
                        }
                    }
                })
                .buffer_unordered(constants::MAX_THUMBNAIL_THREADS)
                .take_while(|_| {
                    let active = session_arc.load(Ordering::Acquire) == current_session;
                    async move { active }
                })
                .collect::<Vec<()>>()
                .await;
        });
    }
}
