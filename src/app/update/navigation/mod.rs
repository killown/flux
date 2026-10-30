use crate::domain::model::{
    AppMsg, ArchiveMsg, FluxApp, NavigationMsg, NetworkMsg, SearchMsg, TabsMsg, TagsMsg,
};
use relm4::prelude::*;

mod history;
mod navigate;
mod navigate_mode;
mod quick_list;

pub(super) fn handle(
    app: &mut FluxApp,
    msg: AppMsg,
    sender: &AsyncComponentSender<FluxApp>,
) -> Result<(), AppMsg> {
    crate::hit!("update::navigation");
    match msg {
        AppMsg::Navigation(NavigationMsg::Navigate(path)) => {
            app.stop_video_preview();
            app.handle_navigate(path, sender);
        }
        AppMsg::Navigation(NavigationMsg::GoBack) => {
            app.stop_video_preview();
            app.handle_go_back(sender);
        }
        AppMsg::Navigation(NavigationMsg::GoForward) => {
            app.stop_video_preview();
            app.handle_go_forward(sender);
            sender.input(AppMsg::Navigation(NavigationMsg::Refresh));
        }
        AppMsg::Navigation(NavigationMsg::ToggleNavigateMode) => {
            app.handle_toggle_navigate_mode(sender)
        }
        AppMsg::Navigation(NavigationMsg::KeyChar(c)) => app.handle_key_char(c, sender),
        AppMsg::Navigation(NavigationMsg::KeyEnter) => app.handle_key_enter(sender),
        AppMsg::Navigation(NavigationMsg::KeyBackspace) => app.handle_key_backspace(sender),
        AppMsg::Navigation(NavigationMsg::KeyEscape) => app.handle_key_escape(sender),
        AppMsg::Navigation(NavigationMsg::CycleByLetter(c)) => {
            app.handle_cycle_by_letter(c, sender)
        }
        AppMsg::Navigation(NavigationMsg::CycleByLetterCancel) => {
            app.handle_cycle_by_letter_cancel(sender)
        }
        AppMsg::Navigation(NavigationMsg::CycleByLetterReanchor) => {
            app.handle_cycle_by_letter_reanchor(sender)
        }
        AppMsg::Search(SearchMsg::SyncPathEntry) => {}
        AppMsg::Navigation(NavigationMsg::ToggleLocationPanel) => {
            app.toggle_sidebar_right_panel(crate::domain::model::RightPanelType::Location, sender)
        }
        AppMsg::Network(NetworkMsg::ToggleConnectPanel) => {
            app.toggle_sidebar_right_panel(crate::domain::model::RightPanelType::Connect, sender)
        }
        AppMsg::Navigation(NavigationMsg::JumpToRecent(rank)) => {
            let target_index = if rank == 0 { 0 } else { rank - 1 };
            if let Some(target_path) = app.recent_stack.get(target_index).cloned() {
                if rank != 0 && target_path == app.current_path {
                    return Ok(());
                }
                sender.input(AppMsg::Navigation(NavigationMsg::Navigate(target_path)));
            }
        }
        AppMsg::Navigation(NavigationMsg::ClearRecents) => app.handle_clear_recents(sender),
        AppMsg::Navigation(NavigationMsg::ClearRecentsFinished(result)) => {
            app.handle_clear_recents_finished(result, sender)
        }
        AppMsg::Tabs(TabsMsg::NewTab(target_path)) => app.handle_new_tab(target_path, sender),
        AppMsg::Tabs(TabsMsg::OpenTabs(targets)) => app.handle_open_tabs(targets, sender),
        AppMsg::Navigation(NavigationMsg::Open { position, tab_id }) => {
            app.handle_open(position, tab_id, sender)
        }
        AppMsg::Navigation(NavigationMsg::ActivateItems(items)) => {
            app.activate_items(items, sender)
        }
        AppMsg::Tabs(TabsMsg::SwitchTab(index)) => app.handle_switch_tab(index, sender),
        AppMsg::Tabs(TabsMsg::TabReordered {
            moved_id,
            new_position,
        }) => app.handle_tab_reordered(moved_id, new_position),
        AppMsg::Tabs(TabsMsg::CloseTab(index_opt)) => app.handle_close_tab(index_opt, sender),
        AppMsg::Tabs(TabsMsg::NextTab) => app.handle_next_tab(),
        AppMsg::Tabs(TabsMsg::PrevTab) => app.handle_prev_tab(),
        AppMsg::Navigation(NavigationMsg::Refresh) => {
            app.preview.active_video = None;
            app.folder_cache.clear();
            app.handle_refresh_path(sender);
        }
        AppMsg::Archive(ArchiveMsg::EnterArchive(archive_path)) => {
            app.stop_video_preview();
            app.handle_enter_archive(archive_path, sender)
        }
        AppMsg::Archive(ArchiveMsg::LoadArchiveWithPassword {
            archive_path,
            prefix,
            password,
        }) => {
            app.archive.locked = false;
            app.load_archive(archive_path, prefix, Some(password), sender);
            app.update_breadcrumbs();
        }
        AppMsg::Network(NetworkMsg::NavigateNetwork) => app.handle_navigate_network(sender),
        AppMsg::Tags(TagsMsg::NavigateTag(tag)) => app.handle_navigate_tag(tag, sender),
        other => return Err(other),
    }
    Ok(())
}
