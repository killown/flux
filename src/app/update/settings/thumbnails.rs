use crate::app::state::FluxApp;
use crate::domain::model::{AppMsg, NavigationMsg};
use crate::utils;
use relm4::prelude::*;

impl FluxApp {
    pub fn handle_set_lazy_thumbnails(&mut self, val: bool) {
        self.config.ui.lazy_thumbnails = val;
        utils::save_config(&self.config);
    }

    pub fn handle_set_archive_thumbnail_cache(&mut self, val: bool) {
        self.config.ui.archive_thumbnail_cache = val;
        crate::infra::archive::thumb_cache::set_enabled(val);
        utils::save_config(&self.config);
    }

    pub fn handle_set_thumbnail_size(&mut self, val: i32, sender: &AsyncComponentSender<Self>) {
        self.config.ui.thumbnail_size = val;
        utils::save_config(&self.config);
        sender.input(AppMsg::Navigation(NavigationMsg::Refresh));
    }

    pub fn handle_set_autoplay_video_previews(&mut self, val: bool) {
        crate::hit!("handle_set_autoplay_video_previews");
        self.config.ui.autoplay_video_previews = val;
        utils::save_config(&self.config);
        if !val {
            self.stop_video_preview();
        } else {
            self.sync_video_preview();
        }
    }

    pub fn handle_set_show_thumbnails(&mut self, val: bool, sender: &AsyncComponentSender<Self>) {
        self.config.ui.show_thumbnails = val;
        utils::save_config(&self.config);
        sender.input(AppMsg::Navigation(NavigationMsg::Refresh));
    }

    pub fn handle_set_thumbnail_type(
        &mut self,
        type_name: String,
        enabled: bool,
        sender: &AsyncComponentSender<Self>,
    ) {
        crate::hit!("handle_set_thumbnail_type");
        match type_name.as_str() {
            "images" => self.config.ui.thumbnail_types.images = enabled,
            "videos" => self.config.ui.thumbnail_types.videos = enabled,
            "fonts" => self.config.ui.thumbnail_types.fonts = enabled,
            "pdfs" => self.config.ui.thumbnail_types.pdfs = enabled,
            "executables" => self.config.ui.thumbnail_types.executables = enabled,
            _ => {}
        }
        utils::save_config(&self.config);
        sender.input(AppMsg::Navigation(NavigationMsg::Refresh));
    }

    pub fn handle_set_ffmpeg_threads(&mut self, val: usize) {
        self.config.ui.ffmpeg_threads = val;
        utils::save_config(&self.config);
    }

    pub fn handle_set_ffmpeg_seek_seconds(&mut self, val: f64) {
        self.config.ui.ffmpeg_seek_seconds = val;
        utils::save_config(&self.config);
    }

    pub fn handle_set_ffmpeg_auto_rotate(&mut self, val: bool) {
        self.config.ui.ffmpeg_auto_rotate = val;
        utils::save_config(&self.config);
    }

    pub fn handle_set_thumbnail_threads(&mut self, val: usize) {
        self.config.ui.thumbnail_threads = val;
        utils::save_config(&self.config);
    }

    pub fn handle_set_fish_theme(&mut self, val: bool) {
        crate::hit!("handle_set_fish_theme");
        self.config.ui.fish_theme = val;
        crate::utils::save_config(&self.config);

        if val {
            // Immediately regenerate the snippet with the current theme.
            // `apply_theme` is idempotent and cheap.
            self.term.widget.apply_theme(&self.config.ui.terminal);
            if let Some(ref sidebar_term) = self.panels.terminal.extras.terminal {
                sidebar_term.apply_theme(&self.config.ui.terminal);
            }
        }
        // Turning it off does not delete the file: the snippet is guarded by
        // `set -q FLUX_THEME_FG` and by the check in `write_fish_theme_snippet`,
        // so leaving it on disk is harmless and lets the user flip it back on
        // without a surprise "where did my file go".
    }
}
