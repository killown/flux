use crate::domain::model::{AppMsg, FluxApp};
use relm4::prelude::*;

impl FluxApp {
    /// Toggles the ephemeral MIME filter based on the selected item.
    pub fn handle_filter_by_selected_mime(&mut self, sender: &AsyncComponentSender<Self>) {
        crate::hit!("handle_filter_by_selected_mime");
        if self.mime_filter.is_some() {
            sender.input(AppMsg::ClearMimeFilter);
            return;
        }
        // Any explicit toggle (Ctrl+M, header button) claims ownership:
        // closing the preview panel must not clear this filter.
        self.mime_filter_from_space = false;
        if self
            .current_path
            .to_string_lossy()
            .starts_with(crate::infra::archive::ARCHIVE_URI)
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
        crate::ui::panels::preview::registry::set_mime_filter_active(true);
        sender.input(AppMsg::Refresh);
    }

    /// Drops the ephemeral MIME filter and reloads the current directory.
    pub fn handle_clear_mime_filter(&mut self, sender: &AsyncComponentSender<Self>) {
        crate::hit!("handle_clear_mime_filter");
        self.mime_filter_from_space = false;
        if self.mime_filter.take().is_some() {
            crate::ui::panels::preview::registry::set_mime_filter_active(false);
            sender.input(AppMsg::Refresh);
        }
    }
}
