use crate::model::{AppMsg, FluxApp};
use relm4::prelude::*;

pub(super) fn handle(
    app: &mut FluxApp,
    msg: AppMsg,
    sender: &AsyncComponentSender<FluxApp>,
) -> Result<(), AppMsg> {
    match msg {
        AppMsg::RefreshSidebar => app.handle_refresh_sidebar(),
        AppMsg::RemoveFromSidebar(path) => app.handle_remove_from_sidebar(path),
        AppMsg::AddToSidebarPermanent => app.handle_add_to_sidebar_permanent(),
        AppMsg::ReorderSidebar { from, to } => app.handle_reorder_sidebar(from, to),
        AppMsg::PromptSidebarRename { path, current_name } => {
            app.handle_prompt_sidebar_rename(path, current_name, sender)
        }
        AppMsg::RenameSidebarPlace { path, new_name } => {
            app.handle_rename_sidebar_place(path, new_name, sender)
        }
        AppMsg::PromptSidebarRenameSection {
            old_name,
            current_name,
        } => app.show_prompt_sidebar_rename_section(old_name, current_name, sender),
        AppMsg::RenameSidebarSection { old_name, new_name } => {
            app.handle_rename_sidebar_section(old_name, new_name)
        }
        AppMsg::RemoveSidebarSection(name) => app.handle_remove_sidebar_section(name),
        AppMsg::PromptNewSidebarSection => app.show_prompt_new_sidebar_section(sender),
        AppMsg::AddSidebarSection(title) => app.handle_add_sidebar_section(title),
        AppMsg::SidebarDropMove {
            source_paths,
            dest_path,
        } => app.handle_sidebar_drop_move(source_paths, dest_path, sender),
        AppMsg::PinFolderAt {
            path,
            before,
            label_name,
        } => app.handle_pin_folder_at(path, before, label_name),
        AppMsg::ShowSidebarPinZone(_) => {}
        AppMsg::ToggleSidebar => app.handle_toggle_sidebar(),
        AppMsg::SetSidebarWidth(val) => app.handle_set_sidebar_width(val),
        AppMsg::ShowSidebarIconPicker(path) => app.show_sidebar_icon_picker(path, sender),

        AppMsg::AddExclusive(p) => app.handle_add_exclusive(p, sender),
        AppMsg::ClearExclusive => app.handle_clear_exclusive(sender),
        AppMsg::RemoveQuickItem(p) => app.handle_remove_quick_item(p, sender),
        AppMsg::RebuildQuickPanel => app.handle_rebuild_quick_panel(sender),
        AppMsg::NextExclusive => app.handle_next_exclusive(sender),
        AppMsg::PrevExclusive => app.handle_prev_exclusive(sender),
        AppMsg::UpdateExclusiveSlot(index) => {
            if index < app.exclusive_list.len() {
                app.exclusive_list[index] = app.current_path.clone();
                app.exclusive_index = Some(index);
                app.handle_rebuild_quick_panel(sender);
                sender.input(AppMsg::ShowToast(crate::i18n::tr(
                    "Quick list slot updated",
                )));
            }
        }
        AppMsg::MoveFilesToTarget {
            sources,
            destination,
        } => app.handle_move_files_to_target(sources, destination, sender),
        other => return Err(other),
    }
    Ok(())
}
