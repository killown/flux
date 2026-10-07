use crate::model::{AppMsg, FluxApp};
use crate::utils;
use relm4::prelude::*;

impl FluxApp {
    pub fn handle_set_lazy_thumbnails(&mut self, val: bool) {
        self.config.ui.lazy_thumbnails = val;
        utils::save_config(&self.config);
    }

    pub fn handle_set_thumbnail_size(&mut self, val: i32, sender: &AsyncComponentSender<Self>) {
        self.config.ui.thumbnail_size = val;
        utils::save_config(&self.config);
        sender.input(AppMsg::Refresh);
    }

    pub fn handle_set_autoplay_video_previews(&mut self, val: bool) {
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
        sender.input(AppMsg::Refresh);
    }

    pub fn handle_set_thumbnail_type(
        &mut self,
        type_name: String,
        enabled: bool,
        sender: &AsyncComponentSender<Self>,
    ) {
        match type_name.as_str() {
            "images" => self.config.ui.thumbnail_types.images = enabled,
            "videos" => self.config.ui.thumbnail_types.videos = enabled,
            "fonts" => self.config.ui.thumbnail_types.fonts = enabled,
            "pdfs" => self.config.ui.thumbnail_types.pdfs = enabled,
            "executables" => self.config.ui.thumbnail_types.executables = enabled,
            _ => {}
        }
        utils::save_config(&self.config);
        sender.input(AppMsg::Refresh);
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
}
