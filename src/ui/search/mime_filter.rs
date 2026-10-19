use crate::model::{AppMsg, FluxApp};
use relm4::prelude::*;

impl FluxApp {
    /// Toggles the ephemeral MIME filter based on the selected item.
    pub fn handle_filter_by_selected_mime(&mut self, sender: &AsyncComponentSender<Self>) {
        if self.mime_filter.is_some() {
            sender.input(AppMsg::ClearMimeFilter);
            return;
        }
        if self
            .current_path
            .to_string_lossy()
            .starts_with(crate::services::archive::ARCHIVE_URI)
        {
            return;
        }
        let Some(path) = self.get_selected_path() else {
            return;
        };
        let mime = if path.is_dir() {
            crate::ui::constants::MIME_DIR.to_string()
        } else {
            crate::utils::media::get_mime_type(&path)
        };
        self.mime_filter = Some(mime);
        crate::ui::panels::preview_registry::set_mime_filter_active(true);
        sender.input(AppMsg::Refresh);
    }

    /// Drops the ephemeral MIME filter and reloads the current directory.
    pub fn handle_clear_mime_filter(&mut self, sender: &AsyncComponentSender<Self>) {
        if self.mime_filter.take().is_some() {
            crate::ui::panels::preview_registry::set_mime_filter_active(false);
            sender.input(AppMsg::Refresh);
        }
    }
}
