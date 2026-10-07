use crate::model::{AppMsg, FluxApp};
use relm4::prelude::*;

pub(super) fn handle(
    app: &mut FluxApp,
    msg: AppMsg,
    sender: &AsyncComponentSender<FluxApp>,
) -> Result<(), AppMsg> {
    match msg {
        AppMsg::OpenActiveDiffLine { line, query } => {
            if let Some(ref path) = app.active_diff_target {
                if let Err(e) = crate::utils::helpers::launch_editor_at_line(
                    &app.config.ui.diff_editor,
                    path,
                    line,
                    query.as_deref(),
                ) {
                    sender.input(AppMsg::ShowToast(format!("Failed to open editor: {}", e)));
                }
            } else {
                sender.input(AppMsg::ShowToast(
                    "No active diff target selected".to_string(),
                ));
            }
        }
        AppMsg::ToggleDiffPanel => {
            app.toggle_sidebar_right_panel(crate::model::RightPanelType::Diff, sender)
        }
        AppMsg::ShowFileDiff(path) => app.handle_show_file_diff(path, sender),
        AppMsg::DiffLoaded { path, diff } => {
            if let Some(ref buffer) = app.diff_text_buffer {
                crate::ui::diff_panel::apply_diff_markup(buffer, &diff);
            }
            app.active_diff_target = Some(path);
        }
        AppMsg::SetDiffPanelWidth(val) => app.handle_set_diff_panel_width(val),
        AppMsg::SetAutoShowDiff(val) => app.handle_set_auto_show_diff(val),
        AppMsg::ShowGitStatusView => app.handle_show_git_status_view(sender.clone()),
        AppMsg::SetGitRepoActive(is_active) => app.is_in_git_repo = is_active,
        AppMsg::GitStatusReady {
            path,
            load_id,
            updates,
        } => app.handle_git_status_ready(path, load_id, updates),
        other => return Err(other),
    }
    Ok(())
}
