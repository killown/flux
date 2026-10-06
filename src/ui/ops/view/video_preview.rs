use crate::i18n::tr;
use crate::model::{AppMsg, FluxApp};
use crate::utils;
use adw::gio::prelude::*;
use adw::prelude::*;
use gtk::glib;
use relm4::prelude::*;
use std::path::PathBuf;
use std::time::{Duration, Instant};

const VIDEO_PREVIEW_LAUNCH_LIMIT: usize = 5;
const VIDEO_PREVIEW_LAUNCH_WINDOW: Duration = Duration::from_secs(1);
const VIDEO_PREVIEW_COOLDOWN: Duration = Duration::from_secs(5);

impl FluxApp {
    /// Finds the grid cell widget that is currently showing the given file path.
    ///
    /// Looks up the item by the `flux_path` data key that `FileItem::bind` stores on
    /// the overlay root and card box.
    ///
    /// NOTE: do not rewrite this to match on `widget_name()`. `bind()` intentionally
    /// never sets the widget name (see the warning in `components.rs`), so a name
    /// lookup always returns `None` and video previews silently do nothing.
    pub fn find_widget_by_path(&self, path: &std::path::Path) -> Option<gtk::Widget> {
        fn search(widget: &gtk::Widget, target: &std::path::Path) -> Option<gtk::Widget> {
            let hit = unsafe {
                widget
                    .data::<PathBuf>("flux_path")
                    .is_some_and(|p| p.as_ref() == target)
            };
            if hit {
                return Some(widget.clone());
            }
            let mut child = widget.first_child();
            while let Some(c) = child {
                if let Some(found) = search(&c, target) {
                    return Some(found);
                }
                child = c.next_sibling();
            }
            None
        }

        let active_view = self
            .tabs
            .get(self.active_tab_index)
            .map(|t| &t.files.view)?;
        search(active_view.as_ref(), path)
    }

    pub fn stop_video_preview(&mut self) {
        if let Some(source_id) = self.video_preview_source.take() {
            source_id.remove();
        }

        let Some(active_path) = self.active_video_preview.take() else {
            return;
        };

        if let Some(widget) = self.find_widget_by_path(&active_path) {
            unsafe {
                if let Some(video_ptr) = widget.data::<gtk::Video>("video_widget") {
                    let video = video_ptr.as_ref();
                    if let Some(stream) = video.media_stream() {
                        stream.pause();
                    }
                }
                if let Some(stack_ptr) = widget.data::<gtk::Stack>("preview_stack") {
                    stack_ptr.as_ref().set_visible_child_name("icon");
                }
            }
        }
    }

    pub fn sync_video_preview(&mut self) {
        crate::hit!("sync_video_preview");
        if !self.config.ui.autoplay_video_previews {
            self.stop_video_preview();
            return;
        }

        let now = Instant::now();

        if self
            .video_preview_cooldown_until
            .is_some_and(|until| now < until)
        {
            self.stop_video_preview();
            return;
        }
        self.video_preview_cooldown_until = None;

        let selection = self.get_selection();
        if selection.len() != 1 {
            self.stop_video_preview();
            return;
        }

        let selected_path = selection[0].clone();
        let (_, is_vid) = utils::media::is_visual_media(&selected_path);
        if !is_vid {
            self.stop_video_preview();
            return;
        }

        if self.active_video_preview.as_ref() == Some(&selected_path) {
            let still_playing = self
                .find_widget_by_path(&selected_path)
                .map(|w| unsafe {
                    w.data::<gtk::Video>("video_widget")
                        .map(|ptr| ptr.as_ref().media_stream().is_some())
                        .unwrap_or(false)
                })
                .unwrap_or(false);

            if still_playing {
                return;
            }
        }

        self.stop_video_preview();

        if self.find_widget_by_path(&selected_path).is_some() {
            self.handle_trigger_video_preview(selected_path);
        } else {
            let target_path = selected_path;
            let source_id =
                glib::timeout_add_local_once(std::time::Duration::from_millis(100), move || {
                    if let Some(s) = crate::model::SENDER.get() {
                        let _ = s.send(AppMsg::TriggerVideoPreview(target_path));
                    }
                });
            self.video_preview_source = Some(source_id);
        }
    }

    pub fn handle_trigger_video_preview(&mut self, path: std::path::PathBuf) {
        crate::hit!("handle_trigger_video_preview");
        self.video_preview_source = None;

        let now = Instant::now();

        if self
            .video_preview_cooldown_until
            .is_some_and(|until| now < until)
        {
            return;
        }

        while self
            .video_preview_launches
            .front()
            .is_some_and(|t| now.duration_since(*t) > VIDEO_PREVIEW_LAUNCH_WINDOW)
        {
            self.video_preview_launches.pop_front();
        }

        if self.video_preview_launches.len() >= VIDEO_PREVIEW_LAUNCH_LIMIT {
            self.video_preview_cooldown_until = Some(now + VIDEO_PREVIEW_COOLDOWN);
            self.video_preview_launches.clear();
            self.stop_video_preview();

            if let Some(s) = crate::model::SENDER.get() {
                let msg = tr("Video preview throttled: too many quick selections, waiting {}s")
                    .replace("{}", &VIDEO_PREVIEW_COOLDOWN.as_secs().to_string());
                let _ = s.send(AppMsg::ShowToast(msg));
            }

            return;
        }

        self.video_preview_launches.push_back(now);

        let selection = self.get_selection();
        if selection.len() != 1 || selection[0] != path {
            return;
        }

        // Ensure any active preview is fully torn down first
        self.stop_video_preview();

        self.active_video_preview = Some(path.clone());

        // Resolve real disk path: if it's an archive virtual path, extract to a tempfile first
        let resolved_file = if let Some((archive_path, inner_path)) =
            crate::services::archive::parse_archive_uri(&path.to_string_lossy())
        {
            match crate::services::archive::extract_entry_to_tempfile(
                &archive_path,
                &inner_path,
                None,
            ) {
                Ok(tmp) => {
                    let tmp_path = tmp.path().to_path_buf();
                    // Keep the temp file alive until preview stops or app exits
                    if let Ok(file) = tmp.keep() {
                        let (_, path_buf) = file;
                        crate::services::archive::register_temp_file(path_buf.clone());
                        Some(gtk::gio::File::for_path(path_buf))
                    } else {
                        Some(gtk::gio::File::for_path(tmp_path))
                    }
                }
                Err(_) => None,
            }
        } else {
            Some(gtk::gio::File::for_path(&path))
        };

        let Some(gfile) = resolved_file else {
            return;
        };

        if let Some(child) = self.find_widget_by_path(&path) {
            let media_file = gtk::MediaFile::for_file(&gfile);
            media_file.set_muted(true);
            media_file.set_loop(true);

            unsafe {
                if let Some(video_ptr) = child.data::<gtk::Video>("video_widget") {
                    let video = video_ptr.as_ref();
                    if let Some(old_stream) = video.media_stream() {
                        old_stream.pause();
                    }
                    video.set_autoplay(true);
                    video.set_loop(true);
                    video.set_media_stream(Some(&media_file));
                }
                if let Some(stack_ptr) = child.data::<gtk::Stack>("preview_stack") {
                    stack_ptr.as_ref().set_visible_child_name("video");
                }
            }

            media_file.play();
        }
    }
}
