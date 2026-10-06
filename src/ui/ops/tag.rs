use crate::model::{AppMsg, FluxApp};
use relm4::prelude::*;

impl FluxApp {
    pub fn handle_apply_tags_to_selection(
        &self,
        tags: Vec<String>,
        sender: &AsyncComponentSender<Self>,
    ) {
        let selection = self.get_selection();
        for path in selection {
            let _ = self
                .state_db
                .set_tags(&path, &tags, chrono::Utc::now().timestamp());
            let _ = crate::utils::xattr::write_tags(&path, &tags);
        }
        sender.input(AppMsg::ShowToast(crate::i18n::tr("Tags updated")));
    }

    pub fn handle_remove_tag_from_selection(
        &self,
        tag: String,
        sender: &AsyncComponentSender<Self>,
    ) {
        let target_tag = tag.trim_start_matches('#').to_lowercase();
        let selection = self.get_selection();
        for path in selection {
            let mut tags = crate::utils::xattr::read_tags(&path);
            tags.retain(|t| t.trim_start_matches('#').to_lowercase() != target_tag);
            let _ = crate::utils::xattr::write_tags(&path, &tags);
            let mtime = std::fs::metadata(&path)
                .and_then(|m| m.modified())
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs() as i64)
                .unwrap_or(0);
            let _ = self.state_db.set_tags(&path, &tags, mtime);
        }
        sender.input(AppMsg::ShowToast(crate::i18n::tr("Tag removed")));
        sender.input(AppMsg::Refresh);
    }

    pub fn handle_open_tag_picker(&self, sender: &AsyncComponentSender<Self>) {
        let selection = self.get_selection();
        let paths = if selection.is_empty() {
            vec![self.current_path.clone()]
        } else {
            selection
        };

        let state_db = self.state_db.clone();
        let sender_tag = sender.clone();
        let paths_clone = paths.clone();

        std::thread::spawn(move || {
            let first_path = &paths_clone[0];
            let initial_tags = crate::utils::xattr::read_tags(first_path);
            let available_tags = state_db.list_all_tags().unwrap_or_default();

            sender_tag.input(AppMsg::TagsReady {
                paths: paths_clone,
                tags: initial_tags,
                available_tags,
            });
        });
    }

    pub fn handle_tags_ready(
        &self,
        paths: Vec<std::path::PathBuf>,
        tags: Vec<String>,
        available_tags: Vec<String>,
        sender: &AsyncComponentSender<Self>,
    ) {
        let parent_widget = self.files.view.clone();
        crate::ui::dialog::tag_picker::show_tag_picker(
            &parent_widget,
            paths,
            tags,
            available_tags,
            sender.clone(),
        );
    }

    pub fn handle_set_file_tags(
        &self,
        path: std::path::PathBuf,
        tags: Vec<String>,
        sender: &AsyncComponentSender<Self>,
    ) {
        let state_db = self.state_db.clone();
        let path_clone = path.clone();
        let tags_clone = tags.clone();
        let sender_refresh = sender.clone();

        std::thread::spawn(move || {
            let _ = crate::utils::xattr::write_tags(&path_clone, &tags_clone);
            let mtime = std::fs::metadata(&path_clone)
                .and_then(|m| m.modified())
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs() as i64)
                .unwrap_or(0);

            let _ = state_db.set_tags(&path_clone, &tags_clone, mtime);
            sender_refresh.input(AppMsg::Refresh);
        });
    }

    pub fn handle_delete_tag_globally(&self, tag: String, sender: &AsyncComponentSender<Self>) {
        let state_db = self.state_db.clone();
        let sender_refresh = sender.clone();
        std::thread::spawn(move || {
            let _ = state_db.delete_tag_globally(&tag);
            sender_refresh.input(AppMsg::Refresh);
        });
    }

    pub fn handle_navigate_tag(&mut self, tag: String, sender: &AsyncComponentSender<Self>) {
        let filter_str = format!(":tag:{}", tag);
        self.search_just_opened = false;
        self.header_view = crate::ui::constants::VIEW_SEARCH.to_string();
        sender.input(AppMsg::UpdateFilter(filter_str));
    }
}
