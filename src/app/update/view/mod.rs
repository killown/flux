use crate::domain::model::{
    AppMsg, ArchiveMsg, FluxApp, IconsMsg, NavigationMsg, PreviewMsg, PropertiesMsg, SearchMsg,
    SettingsMsg, ViewMsg, WindowMsg,
};
use crate::ui::constants;
use relm4::prelude::*;

mod layout;
pub(crate) mod prefetch;
pub(crate) mod preview;
mod selection;
mod text_encoding;
mod thumbnails;
mod video_preview;

pub(super) fn handle(
    app: &mut FluxApp,
    msg: AppMsg,
    sender: &AsyncComponentSender<FluxApp>,
) -> Result<(), AppMsg> {
    crate::hit!("handle");
    match msg {
        AppMsg::View(ViewMsg::SelectionChanged { tab_id }) => {
            app.handle_selection_changed(tab_id, sender);
            // Keep the contextual-actions panel in sync with the selection while open.
            if app.panels.actions.visible {
                app.request_actions_panel_load(sender);
            }
            // Same for the Properties panel: it describes the selected item.
            if app.panels.properties.visible {
                app.panels.properties.extras.properties_target = app.get_selected_path();
                app.request_properties_panel_load(sender);
            }
        }
        AppMsg::View(ViewMsg::QuerySelection { reply }) => {
            let _ = reply.send(app.get_selection());
        }
        AppMsg::View(ViewMsg::ToggleListMode) => app.handle_toggle_list_mode(),
        AppMsg::Search(SearchMsg::ToggleFilterBar) => {
            if app.header_view == constants::VIEW_FILTER {
                app.handle_clear_extension_filter(sender);
                app.handle_switch_header(constants::VIEW_PATH.to_string());
            } else {
                app.handle_switch_header(constants::VIEW_FILTER.to_string());
            }
        }
        AppMsg::View(ViewMsg::ToggleSortOrder) => app.handle_toggle_sort_order(sender),
        AppMsg::View(ViewMsg::CycleSort) => app.handle_cycle_sort(sender),
        AppMsg::View(ViewMsg::CycleFolderPriority) => app.handle_cycle_folder_priority(sender),
        AppMsg::View(ViewMsg::SetAsc(asc)) => app.handle_set_asc(asc, sender),
        AppMsg::View(ViewMsg::SetDefaultSort(sort)) => app.handle_set_default_sort(sort, sender),
        AppMsg::Settings(SettingsMsg::SetFoldersFirst(val)) => {
            app.handle_set_folders_first(val, sender)
        }
        AppMsg::Settings(SettingsMsg::SetGridSpacing(val)) => {
            app.handle_set_grid_spacing(val, sender)
        }
        AppMsg::Settings(SettingsMsg::SetMaxWidthChars(val)) => {
            app.handle_set_max_width_chars(val, sender)
        }
        AppMsg::Settings(SettingsMsg::SetBreadcrumbCount(val)) => {
            app.handle_set_breadcrumb_count(val)
        }
        AppMsg::Settings(SettingsMsg::SetBreadcrumbMaxChars(val)) => {
            app.handle_set_breadcrumb_max_chars(val)
        }
        AppMsg::Settings(SettingsMsg::SetExpandLabels(val)) => {
            app.handle_set_expand_labels(val, sender)
        }
        AppMsg::View(ViewMsg::Zoom(delta)) => app.handle_zoom(delta),
        AppMsg::Search(SearchMsg::SwitchHeader(view_name)) => app.handle_switch_header(view_name),
        AppMsg::Preview(PreviewMsg::TogglePreview) => app.handle_toggle_preview(sender),
        AppMsg::Preview(PreviewMsg::PreviewLoad { path, session }) => {
            app.handle_preview_load(path, session)
        }
        AppMsg::View(ViewMsg::SelectionStep(delta)) => app.handle_selection_step(delta, sender),
        AppMsg::Preview(PreviewMsg::PreviewReady {
            path,
            session,
            content,
        }) => app.handle_preview_ready(path, session, content),
        AppMsg::Search(SearchMsg::TogglePreviewAndFilterBySelectedMime) => {
            app.handle_toggle_preview_and_filter_by_selected_mime(sender)
        }
        AppMsg::Preview(PreviewMsg::StartupPreview) => app.handle_startup_preview(sender),
        AppMsg::Preview(PreviewMsg::TogglePreviewFullscreen) => {
            app.handle_toggle_preview_fullscreen()
        }
        AppMsg::Preview(PreviewMsg::ExitPreviewFullscreen) => app.handle_exit_preview_fullscreen(),
        AppMsg::Preview(PreviewMsg::TogglePreviewGridFocus) => {
            app.handle_toggle_preview_grid_focus()
        }
        AppMsg::Preview(PreviewMsg::FocusPaneLeft) => app.handle_focus_pane(-1),
        AppMsg::Preview(PreviewMsg::FocusPaneRight) => app.handle_focus_pane(1),
        AppMsg::Preview(PreviewMsg::SetPreviewPanelWidth(val)) => {
            app.handle_set_preview_panel_width(val)
        }
        AppMsg::Preview(PreviewMsg::PreviewPrefetch { path, session }) => {
            app.handle_preview_prefetch(path, session, sender)
        }
        AppMsg::Preview(PreviewMsg::PreviewPrefetched { .. }) => {}
        AppMsg::Search(SearchMsg::CycleFilterMode) => {
            app.filters.mode = match app.filters.mode {
                crate::domain::model::FilterMode::All => {
                    crate::domain::model::FilterMode::OnlyFolders
                }
                crate::domain::model::FilterMode::OnlyFolders => {
                    crate::domain::model::FilterMode::OnlyFiles
                }
                crate::domain::model::FilterMode::OnlyFiles => {
                    crate::domain::model::FilterMode::All
                }
            };
            sender.input(AppMsg::Navigation(NavigationMsg::Refresh));
        }

        AppMsg::Icons(IconsMsg::SetIconSize(val)) => app.handle_set_icon_size(val, sender),
        AppMsg::Icons(IconsMsg::SetListIconSize(val)) => app.handle_set_list_icon_size(val, sender),
        AppMsg::Icons(IconsMsg::SetShowEmptyDirEmblem(val)) => {
            app.handle_set_show_empty_dir_emblem(val)
        }
        AppMsg::Icons(IconsMsg::SetShowSymlinkEmblem(val)) => {
            app.handle_set_show_symlink_emblem(val, sender)
        }
        AppMsg::Settings(SettingsMsg::SetShowCsd(val)) => app.handle_set_show_csd(val),
        AppMsg::Settings(SettingsMsg::SetHeaderShowFilter(val)) => {
            app.config.ui.header_show_filter = val;
            crate::utils::save_config(&app.config);
        }
        AppMsg::Settings(SettingsMsg::SetHeaderShowSort(val)) => {
            app.config.ui.header_show_sort = val;
            crate::utils::save_config(&app.config);
        }
        AppMsg::Settings(SettingsMsg::SetHeaderShowView(val)) => {
            app.config.ui.header_show_view = val;
            crate::utils::save_config(&app.config);
        }
        AppMsg::Settings(SettingsMsg::SetHeaderShowPreview(val)) => {
            app.config.ui.header_show_preview = val;
            crate::utils::save_config(&app.config);
        }
        AppMsg::Settings(SettingsMsg::SetSidebarMenuRequiresCtrl(val)) => {
            app.config.ui.sidebar_menu_requires_ctrl = val;
            crate::utils::save_config(&app.config);
        }
        AppMsg::Settings(SettingsMsg::SetPanelMenuRequiresCtrl(val)) => {
            app.config.ui.panel_menu_requires_ctrl = val;
            crate::utils::save_config(&app.config);
        }
        AppMsg::Window(WindowMsg::SetWindowControlsLeft(val)) => {
            app.handle_set_window_controls_left(val)
        }
        AppMsg::Settings(SettingsMsg::SetShowXdgDirs(val)) => {
            app.handle_set_show_xdg_dirs(val, sender)
        }
        AppMsg::Settings(SettingsMsg::SetShowHidden(val)) => {
            app.handle_set_show_hidden(val, sender)
        }
        AppMsg::View(ViewMsg::ToggleHidden) => app.handle_set_show_hidden(!app.show_hidden, sender),
        AppMsg::View(ViewMsg::ToggleStatusBar) => app.statusbar_visible = !app.statusbar_visible,
        AppMsg::View(ViewMsg::ToggleCurrentFoldersFirst) => {
            app.handle_toggle_current_folders_first(sender)
        }
        AppMsg::Settings(SettingsMsg::SetSingleClick(val)) => app.handle_set_single_click(val),
        AppMsg::Settings(SettingsMsg::SetAutoListThreshold(val)) => {
            app.handle_set_auto_list_threshold(val)
        }
        AppMsg::Archive(ArchiveMsg::SetArchivesAsFolders(val)) => {
            app.handle_set_archives_as_folders(val, sender)
        }
        AppMsg::View(ViewMsg::ToggleSingleClick) => app.handle_toggle_single_click(),
        AppMsg::View(ViewMsg::SetScrolledToBottom(at_bottom)) => app.scrolled_to_bottom = at_bottom,

        AppMsg::Icons(IconsMsg::SetAutoMimeBodyColor(c)) => {
            app.handle_set_auto_mime_body_color(c, sender)
        }
        AppMsg::Icons(IconsMsg::SetAutoMimeFontColor(c)) => {
            app.handle_set_auto_mime_font_color(c, sender)
        }
        AppMsg::Icons(IconsMsg::SetAutoGenerateMimeIcons(v)) => {
            app.handle_set_auto_generate_mime_icons(v, sender)
        }
        AppMsg::Icons(IconsMsg::SetAutoMimeAccentColor(c)) => {
            app.handle_set_auto_mime_accent_color(c, sender)
        }
        AppMsg::Icons(IconsMsg::SetAutoMimeFontSize(s)) => {
            app.handle_set_auto_mime_font_size(s, sender)
        }
        AppMsg::Icons(IconsMsg::ResetExtensionIcon(ext)) => {
            app.handle_reset_extension_icon(ext, sender)
        }
        AppMsg::Icons(IconsMsg::SetFileIcon { path, image_path }) => {
            app.handle_set_file_icon(path, image_path, sender)
        }
        AppMsg::Icons(IconsMsg::ResetFileIcon(path)) => app.handle_reset_file_icon(path, sender),
        AppMsg::Icons(IconsMsg::SetFolderIcon { path, icon_name }) => {
            app.handle_set_folder_icon(path, icon_name, sender)
        }
        AppMsg::Icons(IconsMsg::ResetFolderIcon(path)) => {
            app.handle_reset_folder_icon(path, sender)
        }
        AppMsg::Icons(IconsMsg::TriggerResetIcon) => app.handle_trigger_reset_icon(sender),
        AppMsg::Icons(IconsMsg::ShowIconPicker(target_path)) => {
            app.show_icon_picker(target_path, sender)
        }
        AppMsg::Icons(IconsMsg::TriggerIconPicker) => app.handle_trigger_icon_picker(sender),
        AppMsg::Icons(IconsMsg::FolderIconsReady { icons, session }) => {
            app.handle_folder_icons_ready(icons, session)
        }
        AppMsg::Properties(PropertiesMsg::MediaDurationReady(d)) => {
            app.handle_media_duration_ready(d)
        }
        AppMsg::Properties(PropertiesMsg::FileMetaReady { mime, dimensions }) => {
            app.handle_file_meta_ready(mime, dimensions)
        }
        AppMsg::Settings(SettingsMsg::SetHiddenExtensions(exts)) => {
            app.handle_set_hidden_extensions(exts, sender)
        }
        other => return Err(other),
    }
    Ok(())
}
