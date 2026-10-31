use crate::app::state::FluxApp;
use crate::domain::model::constants::SAVE_SEARCH_AS_TAG_MAX_FILES;
use crate::domain::model::{AppMsg, NavigationMsg, SearchMsg, TagsMsg, WindowMsg};
use gtk::prelude::*;
use relm4::prelude::*;

/// Adds `tag` to the tags `path` already has, in the database and in its extended attributes.
///
/// Returns `false` when the file could not be tagged. When the extended-attribute write
/// fails, the previous tag set goes back into the database, so the two stores never
/// disagree about the file (the tag views read the extended attributes).
fn add_tag_to_path(
    state_db: &crate::infra::db::StateManager,
    path: &std::path::Path,
    tag: &str,
) -> bool {
    let previous = crate::utils::xattr::read_tags(path);
    let mut tags = previous.clone();
    if !tags.iter().any(|t| t.to_lowercase() == tag) {
        tags.push(tag.to_string());
    }
    let Some(mtime) = std::fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs() as i64)
    else {
        return false;
    };
    if state_db.set_tags(path, &tags, mtime).is_err() {
        return false;
    }
    if crate::utils::xattr::write_tags(path, &tags).is_err() {
        let _ = state_db.set_tags(path, &previous, mtime);
        return false;
    }
    true
}

impl FluxApp {
    pub fn handle_apply_tags_to_selection(
        &self,
        tags: Vec<String>,
        sender: &AsyncComponentSender<Self>,
    ) {
        crate::hit!("handle_apply_tags_to_selection");
        let selection = self.get_selection();
        for path in selection {
            let _ = self
                .state_db
                .set_tags(&path, &tags, chrono::Utc::now().timestamp());
            let _ = crate::utils::xattr::write_tags(&path, &tags);
        }
        sender.input(AppMsg::Window(WindowMsg::ShowToast(crate::i18n::tr(
            "Tags updated",
        ))));
    }

    pub fn handle_remove_tag_from_selection(
        &self,
        tag: String,
        sender: &AsyncComponentSender<Self>,
    ) {
        crate::hit!("handle_remove_tag_from_selection");
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
        sender.input(AppMsg::Window(WindowMsg::ShowToast(crate::i18n::tr(
            "Tag removed",
        ))));
        sender.input(AppMsg::Navigation(NavigationMsg::Refresh));
    }

    pub fn handle_open_tag_picker(&self, sender: &AsyncComponentSender<Self>) {
        crate::hit!("handle_open_tag_picker");
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

            sender_tag.input(AppMsg::Tags(TagsMsg::TagsReady {
                paths: paths_clone,
                tags: initial_tags,
                available_tags,
            }));
        });
    }

    pub fn handle_tags_ready(
        &self,
        paths: Vec<std::path::PathBuf>,
        tags: Vec<String>,
        available_tags: Vec<String>,
        sender: &AsyncComponentSender<Self>,
    ) {
        crate::hit!("handle_tags_ready");
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
        crate::hit!("handle_set_file_tags");
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
            sender_refresh.input(AppMsg::Navigation(NavigationMsg::Refresh));
        });
    }

    pub fn handle_delete_tag_globally(&self, tag: String, sender: &AsyncComponentSender<Self>) {
        crate::hit!("handle_delete_tag_globally");
        let state_db = self.state_db.clone();
        let sender_refresh = sender.clone();
        std::thread::spawn(move || {
            let _ = state_db.delete_tag_globally(&tag);
            sender_refresh.input(AppMsg::Navigation(NavigationMsg::Refresh));
        });
    }

    pub fn handle_navigate_tag(&mut self, tag: String, sender: &AsyncComponentSender<Self>) {
        crate::hit!("handle_navigate_tag");
        let filter_str = format!(":tag:{}", tag);
        self.search.just_opened = false;
        self.header_view = crate::domain::model::constants::VIEW_SEARCH.to_string();
        sender.input(AppMsg::Search(SearchMsg::UpdateFilter(filter_str)));
    }

    /// Number of results the active tab shows for the current search, or `0` when it
    /// shows nothing that "Save as tag" can label.
    ///
    /// Results only count when the item store holds exactly what the grid shows. A view
    /// filter (fuzzy name, size) that hides rows would make the count and the tagged
    /// paths differ from what the user sees. Virtual locations never count, their
    /// paths are not files on disk.
    fn search_result_count(&self) -> usize {
        let in_search = self.filters.extension.is_some()
            || self.search.saved_layout
            || (self.header_view == crate::domain::model::constants::VIEW_SEARCH
                && !self.filter.is_empty());
        let virtual_location = !self.current_path.is_absolute()
            || self
                .current_path
                .to_string_lossy()
                .starts_with(crate::infra::archive::ARCHIVE_URI);
        if !in_search || virtual_location {
            return 0;
        }
        let Some(tab) = self.tabs.get(self.active_tab_index) else {
            return 0;
        };
        let stored = tab.files.len() as usize;
        let visible = tab
            .files
            .view
            .model()
            .and_then(|m| m.downcast::<gtk::MultiSelection>().ok())
            .map_or(0, |m| m.n_items() as usize);
        if visible == stored {
            stored
        } else {
            0
        }
    }

    /// Turns the search panel's "Save as tag" button on only while the current search has
    /// between 1 and `SAVE_SEARCH_AS_TAG_MAX_FILES` results.
    pub fn sync_save_search_tag_button(&self) {
        if !self.panels.search.initialized {
            return;
        }
        let Some(revealer) = self.panels.search.revealer.as_ref() else {
            return;
        };
        let Some(button) = crate::utils::helpers::find_widget_by_name(
            revealer.upcast_ref::<gtk::Widget>(),
            crate::ui::panels::search::SAVE_AS_TAG_BUTTON_NAME,
        ) else {
            return;
        };
        let enabled = (1..=SAVE_SEARCH_AS_TAG_MAX_FILES).contains(&self.search_result_count());
        let tooltip = crate::ui::panels::search::save_as_tag_tooltip(enabled);
        button.set_sensitive(enabled);
        button.set_tooltip_text(Some(&tooltip));
    }

    /// Opens the "Save search as tag" dialog for the results the active tab shows.
    pub fn handle_prompt_save_search_as_tag(&self, sender: &AsyncComponentSender<Self>) {
        crate::hit!("handle_prompt_save_search_as_tag");
        let count = self.search_result_count();
        if count == 0 {
            sender.input(AppMsg::Window(WindowMsg::ShowToast(crate::i18n::tr(
                "No search results to tag",
            ))));
            return;
        }
        if count > SAVE_SEARCH_AS_TAG_MAX_FILES {
            let msg = crate::i18n::tr("Too many results to tag ({count}), the limit is {max}")
                .replace("{count}", &count.to_string())
                .replace("{max}", &SAVE_SEARCH_AS_TAG_MAX_FILES.to_string());
            sender.input(AppMsg::Window(WindowMsg::ShowToast(msg)));
            return;
        }

        let files = &self.tabs[self.active_tab_index].files;
        let mut seen = std::collections::HashSet::new();
        let mut paths = Vec::with_capacity(count);
        for i in 0..files.len() {
            if let Some(item) = files.get(i) {
                let path = item.borrow().path.clone();
                if seen.insert(path.clone()) {
                    paths.push(path);
                }
            }
        }
        self.show_save_search_as_tag(paths, sender);
    }

    /// Adds `tag` to every path of a saved search on a worker thread, then reports the
    /// outcome with [`TagsMsg::SaveSearchAsTagDone`].
    pub fn handle_save_search_as_tag(
        &self,
        tag: String,
        paths: Vec<std::path::PathBuf>,
        pin_to_sidebar: bool,
        sender: &AsyncComponentSender<Self>,
    ) {
        crate::hit!("handle_save_search_as_tag");
        let tag = tag.trim().trim_start_matches('#').to_lowercase();
        if tag.is_empty() || paths.is_empty() {
            return;
        }
        let state_db = self.state_db.clone();
        let sender = sender.clone();
        relm4::spawn_blocking(move || {
            let mut count = 0usize;
            let mut failed = 0usize;
            for path in &paths {
                if add_tag_to_path(&state_db, path, &tag) {
                    count += 1;
                } else {
                    failed += 1;
                }
            }
            sender.input(AppMsg::Tags(TagsMsg::SaveSearchAsTagDone {
                tag,
                count,
                failed,
                pin_to_sidebar,
            }));
        });
    }

    /// Shows how many files got the tag and, when asked and at least one did, pins the
    /// tag to the sidebar unless a sidebar entry for it already exists.
    pub fn handle_save_search_as_tag_done(
        &mut self,
        tag: String,
        count: usize,
        failed: usize,
        pin_to_sidebar: bool,
        sender: &AsyncComponentSender<Self>,
    ) {
        crate::hit!("handle_save_search_as_tag_done");
        let msg = if count == 0 {
            crate::i18n::tr("Could not tag any files as #{tag}").replace("{tag}", &tag)
        } else {
            crate::i18n::tr("Tagged {count} of {total} files as #{tag}")
                .replace("{count}", &count.to_string())
                .replace("{total}", &(count + failed).to_string())
                .replace("{tag}", &tag)
        };
        sender.input(AppMsg::Window(WindowMsg::ShowToast(msg)));

        if pin_to_sidebar && count > 0 {
            let uri = format!("tags://{}", tag);
            if !self.config.sidebar.iter().any(|place| place.path == uri) {
                self.handle_add_tag_to_sidebar(tag);
            }
        }
    }
}
