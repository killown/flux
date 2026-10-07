use crate::model::{AppMsg, FluxApp};
use relm4::prelude::*;

pub(super) fn handle(
    app: &mut FluxApp,
    msg: AppMsg,
    sender: &AsyncComponentSender<FluxApp>,
) -> Result<(), AppMsg> {
    match msg {
        AppMsg::SelectionChanged => app.handle_selection_changed(sender),
        AppMsg::ToggleListMode => app.handle_toggle_list_mode(),
        AppMsg::ToggleSortOrder => app.handle_toggle_sort_order(sender),
        AppMsg::CycleSort => app.handle_cycle_sort(sender),
        AppMsg::CycleFolderPriority => app.handle_cycle_folder_priority(sender),
        AppMsg::SetAsc(asc) => app.handle_set_asc(asc, sender),
        AppMsg::SetDefaultSort(sort) => app.handle_set_default_sort(sort, sender),
        AppMsg::SetFoldersFirst(val) => app.handle_set_folders_first(val, sender),
        AppMsg::SetGridSpacing(val) => app.handle_set_grid_spacing(val, sender),
        AppMsg::SetMaxWidthChars(val) => app.handle_set_max_width_chars(val, sender),
        AppMsg::SetExpandLabels(val) => app.handle_set_expand_labels(val, sender),
        AppMsg::Zoom(delta) => app.handle_zoom(delta),
        AppMsg::SwitchHeader(view_name) => app.handle_switch_header(view_name),

        AppMsg::SetIconSize(val) => app.handle_set_icon_size(val, sender),
        AppMsg::SetListIconSize(val) => app.handle_set_list_icon_size(val, sender),
        AppMsg::SetShowEmptyDirEmblem(val) => app.handle_set_show_empty_dir_emblem(val),
        AppMsg::SetShowSymlinkEmblem(val) => app.handle_set_show_symlink_emblem(val, sender),
        AppMsg::SetShowCsd(val) => app.handle_set_show_csd(val),
        AppMsg::SetWindowControlsLeft(val) => app.handle_set_window_controls_left(val),
        AppMsg::SetShowXdgDirs(val) => app.handle_set_show_xdg_dirs(val, sender),
        AppMsg::SetShowHidden(val) => app.handle_set_show_hidden(val, sender),
        AppMsg::ToggleHidden => app.handle_set_show_hidden(!app.show_hidden, sender),
        AppMsg::ToggleStatusBar => app.statusbar_visible = !app.statusbar_visible,
        AppMsg::ToggleCurrentFoldersFirst => app.handle_toggle_current_folders_first(sender),
        AppMsg::SetSingleClick(val) => app.handle_set_single_click(val),
        AppMsg::ToggleSingleClick => app.handle_toggle_single_click(),
        AppMsg::SetScrolledToBottom(at_bottom) => app.scrolled_to_bottom = at_bottom,

        AppMsg::SetAutoMimeBodyColor(c) => app.handle_set_auto_mime_body_color(c, sender),
        AppMsg::SetAutoMimeFontColor(c) => app.handle_set_auto_mime_font_color(c, sender),
        AppMsg::SetAutoGenerateMimeIcons(v) => app.handle_set_auto_generate_mime_icons(v, sender),
        AppMsg::SetAutoMimeAccentColor(c) => app.handle_set_auto_mime_accent_color(c, sender),
        AppMsg::SetAutoMimeFontSize(s) => app.handle_set_auto_mime_font_size(s, sender),
        AppMsg::ResetExtensionIcon(ext) => app.handle_reset_extension_icon(ext, sender),
        AppMsg::SetFileIcon { path, image_path } => {
            app.handle_set_file_icon(path, image_path, sender)
        }
        AppMsg::ResetFileIcon(path) => app.handle_reset_file_icon(path, sender),
        AppMsg::SetFolderIcon { path, icon_name } => {
            app.handle_set_folder_icon(path, icon_name, sender)
        }
        AppMsg::ResetFolderIcon(path) => app.handle_reset_folder_icon(path, sender),
        AppMsg::TriggerResetIcon => app.handle_trigger_reset_icon(sender),
        AppMsg::ShowIconPicker(target_path) => app.show_icon_picker(target_path, sender),
        AppMsg::TriggerIconPicker => app.handle_trigger_icon_picker(sender),
        AppMsg::FolderIconsReady { icons, session } => {
            app.handle_folder_icons_ready(icons, session)
        }
        AppMsg::MediaDurationReady(d) => app.handle_media_duration_ready(d),
        AppMsg::FileMetaReady { mime, dimensions } => app.handle_file_meta_ready(mime, dimensions),
        AppMsg::SetHiddenExtensions(exts) => app.handle_set_hidden_extensions(exts, sender),
        other => return Err(other),
    }
    Ok(())
}
