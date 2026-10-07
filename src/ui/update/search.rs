use crate::model::{AppMsg, FluxApp};
use relm4::prelude::*;

pub(super) fn handle(
    app: &mut FluxApp,
    msg: AppMsg,
    sender: &AsyncComponentSender<FluxApp>,
) -> Result<(), AppMsg> {
    match msg {
        AppMsg::SetTagPanelWidth(val) => app.handle_set_tag_panel_width(val),
        AppMsg::SetSearchPanelWidth(val) => app.handle_set_search_panel_width(val),
        AppMsg::UpdateFilter(query) => app.handle_update_filter(query, sender),
        AppMsg::SearchInput(c) => app.handle_search_input(c),
        AppMsg::SearchBackspace => app.handle_search_backspace(sender),
        AppMsg::CloseSearchSync => app.search_just_opened = false,
        AppMsg::SetExtensionFilter(patterns) => app.handle_set_extension_filter(patterns, sender),
        AppMsg::ClearExtensionFilter => app.handle_clear_extension_filter(sender),
        AppMsg::StartAdvancedSearch(params) => {
            app.header_view = crate::ui::constants::VIEW_SEARCH.to_string();
            app.last_search_was_advanced = true;
            crate::services::search::start_advanced_search(app, params, sender.clone());
        }
        AppMsg::StartExtensionSearch(patterns) => {
            crate::services::search::start_extension_search(app, patterns, sender.clone());
        }
        AppMsg::ExtensionSearchBatch { results, session } => {
            app.handle_extension_search_batch(results, session)
        }
        AppMsg::StartContentSearch(term, ext_filter) => {
            app.header_view = crate::ui::constants::VIEW_SEARCH.to_string();
            crate::services::search::start_content_search(app, term, ext_filter, sender.clone());
        }
        AppMsg::SetEnableFileIndexing(enabled) => {
            app.config.ui.enable_file_indexing = enabled;
            crate::utils::save_config(&app.config);
            if enabled {
                crate::services::search::indexer::build_home_index_async();
            }
        }
        AppMsg::ContentSearchResult {
            path,
            line,
            line_number,
            session,
        } => {
            app.handle_content_search_result(path, line, line_number, session);
        }
        AppMsg::ContentSearchDone { session } => app.handle_content_search_done(session),
        AppMsg::CancelContentSearch => {
            sender.input(AppMsg::Refresh);
            app.reset_from_content_search();
        }
        AppMsg::SetMaxSearchResults(val) => app.handle_set_max_search_results(val),
        AppMsg::SetMaxContentSearchResults(val) => {
            app.handle_set_max_content_search_results(val, sender)
        }
        AppMsg::ApplyTagsToSelection(tags) => app.handle_apply_tags_to_selection(tags, sender),
        AppMsg::RemoveTagFromSelection(tag) => app.handle_remove_tag_from_selection(tag, sender),
        AppMsg::AddTagToSidebar(tag) => app.handle_add_tag_to_sidebar(tag),
        AppMsg::OpenTagPicker => app.handle_open_tag_picker(sender),
        AppMsg::TagsReady {
            paths,
            tags,
            available_tags,
        } => app.handle_tags_ready(paths, tags, available_tags, sender),
        AppMsg::SetFileTags { path, tags } => app.handle_set_file_tags(path, tags, sender),
        AppMsg::DeleteTagGlobally(tag) => app.handle_delete_tag_globally(tag, sender),
        AppMsg::ToggleTagPanel => {
            app.toggle_sidebar_right_panel(crate::model::RightPanelType::Tag, sender)
        }
        AppMsg::ToggleSearchPanel => {
            app.toggle_sidebar_right_panel(crate::model::RightPanelType::Search, sender)
        }
        other => return Err(other),
    }
    Ok(())
}
