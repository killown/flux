use crate::model::{AppMsg, FluxApp};
use crate::services::constants;
use crate::utils;
use futures::StreamExt;
use relm4::prelude::*;
use std::path::PathBuf;
use std::sync::atomic::Ordering;

impl FluxApp {
    pub fn spawn_thumbnail_loader(
        &self,
        media_tasks: Vec<(String, PathBuf)>,
        current_session: u64,
        sender: AsyncComponentSender<Self>,
    ) {
        let session_arc = self.load_id.clone();

        relm4::spawn(async move {
            let mut stream = futures::stream::iter(media_tasks)
                .map(|(name, media_path)| {
                    let inner_sender = sender.clone();
                    let inner_session = session_arc.clone();
                    async move {
                        if inner_session.load(Ordering::SeqCst) != current_session {
                            return;
                        }
                        let res = tokio::task::spawn_blocking(move || {
                            utils::get_or_create_thumbnail(&media_path)
                        })
                        .await;

                        if let Ok(Some(texture)) = res {
                            if inner_session.load(Ordering::SeqCst) == current_session {
                                inner_sender.input(AppMsg::ThumbnailReady {
                                    name,
                                    texture,
                                    load_id: current_session,
                                });
                            }
                        }
                    }
                })
                .buffer_unordered(constants::MAX_THUMBNAIL_THREADS);

            while stream.next().await.is_some() {
                if session_arc.load(Ordering::SeqCst) != current_session {
                    break;
                }
            }
        });
    }
}
