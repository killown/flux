use crate::app::state::FluxApp;
use crate::domain::model::{ActionsMsg, AppMsg, AppSidebarMsg, NavigationMsg, SearchMsg, TagsMsg};
use relm4::prelude::*;

mod content;
mod content_walk;
mod extension_filter;
mod extension_walk;
mod filter;
mod input;
mod mime_filter;
mod misc;

pub(super) fn handle(
    app: &mut FluxApp,
    msg: AppMsg,
    sender: &AsyncComponentSender<FluxApp>,
) -> Result<(), AppMsg> {
    crate::hit!("handle");
    match msg {
        AppMsg::Tags(TagsMsg::SetTagPanelWidth(val)) => app.handle_set_tag_panel_width(val),
        AppMsg::Search(SearchMsg::SetSearchPanelWidth(val)) => {
            app.handle_set_search_panel_width(val)
        }
        AppMsg::Actions(ActionsMsg::SetActionsPanelWidth(val)) => {
            app.handle_set_actions_panel_width(val)
        }
        AppMsg::Search(SearchMsg::UpdateFilter(query)) => app.handle_update_filter(query, sender),
        AppMsg::Search(SearchMsg::SearchInput(c)) => app.handle_search_input(c),
        AppMsg::Search(SearchMsg::SearchBackspace) => app.handle_search_backspace(sender),
        AppMsg::Search(SearchMsg::CloseSearchSync) => app.search.just_opened = false,
        AppMsg::Search(SearchMsg::SetExtensionFilter(patterns)) => {
            app.handle_set_extension_filter(patterns, sender)
        }
        AppMsg::Search(SearchMsg::FilterBySelectedMime) => {
            app.handle_filter_by_selected_mime(sender)
        }
        AppMsg::Search(SearchMsg::ClearMimeFilter) => app.handle_clear_mime_filter(sender),
        AppMsg::Search(SearchMsg::ClearExtensionFilter) => {
            app.handle_clear_extension_filter(sender)
        }
        AppMsg::Search(SearchMsg::StartAdvancedSearch(params)) => {
            app.header_view = crate::ui::constants::VIEW_SEARCH.to_string();
            app.search.last_was_advanced = true;
            extension_walk::start_advanced_search(app, params, sender.clone());
        }
        AppMsg::Search(SearchMsg::StartExtensionSearch(patterns)) => {
            extension_walk::start_extension_search(app, patterns, sender.clone());
        }
        AppMsg::Search(SearchMsg::ExtensionSearchBatch { results, session }) => {
            app.handle_extension_search_batch(results, session)
        }
        AppMsg::Search(SearchMsg::StartContentSearch(term, ext_filter)) => {
            app.header_view = crate::ui::constants::VIEW_SEARCH.to_string();
            content_walk::start_content_search(app, term, ext_filter, sender.clone());
        }
        AppMsg::Search(SearchMsg::SetEnableFileIndexing(enabled)) => {
            app.config.ui.enable_file_indexing = enabled;
            crate::utils::save_config(&app.config);
            if enabled {
                crate::infra::search::indexer::build_home_index_async();
            }
        }
        AppMsg::Search(SearchMsg::ContentSearchResult {
            path,
            line,
            line_number,
            session,
        }) => {
            app.handle_content_search_result(path, line, line_number, session);
        }
        AppMsg::Search(SearchMsg::ContentSearchDone { session }) => {
            app.handle_content_search_done(session)
        }
        AppMsg::Search(SearchMsg::CancelContentSearch) => {
            sender.input(AppMsg::Navigation(NavigationMsg::Refresh));
            app.reset_from_content_search();
        }
        AppMsg::Search(SearchMsg::SetMaxSearchResults(val)) => {
            app.handle_set_max_search_results(val)
        }
        AppMsg::Search(SearchMsg::SetMaxContentSearchResults(val)) => {
            app.handle_set_max_content_search_results(val, sender)
        }
        AppMsg::Tags(TagsMsg::ApplyTagsToSelection(tags)) => {
            app.handle_apply_tags_to_selection(tags, sender)
        }
        AppMsg::Tags(TagsMsg::RemoveTagFromSelection(tag)) => {
            app.handle_remove_tag_from_selection(tag, sender)
        }
        AppMsg::Sidebar(AppSidebarMsg::AddTagToSidebar(tag)) => app.handle_add_tag_to_sidebar(tag),
        AppMsg::Tags(TagsMsg::OpenTagPicker) => app.handle_open_tag_picker(sender),
        AppMsg::Tags(TagsMsg::TagsReady {
            paths,
            tags,
            available_tags,
        }) => app.handle_tags_ready(paths, tags, available_tags, sender),
        AppMsg::Tags(TagsMsg::SetFileTags { path, tags }) => {
            app.handle_set_file_tags(path, tags, sender)
        }
        AppMsg::Tags(TagsMsg::DeleteTagGlobally(tag)) => {
            app.handle_delete_tag_globally(tag, sender)
        }
        AppMsg::Tags(TagsMsg::ToggleTagPanel) => {
            app.toggle_sidebar_right_panel(crate::domain::model::RightPanelType::Tag, sender)
        }
        AppMsg::Search(SearchMsg::ToggleSearchPanel) => {
            app.toggle_sidebar_right_panel(crate::domain::model::RightPanelType::Search, sender)
        }
        AppMsg::Actions(ActionsMsg::ToggleActionsPanel) => {
            app.toggle_sidebar_right_panel(crate::domain::model::RightPanelType::Actions, sender)
        }
        other => return Err(other),
    }
    Ok(())
}
