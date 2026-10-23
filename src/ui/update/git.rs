use crate::model::{AppMsg, FluxApp};
use adw::prelude::*;
use relm4::prelude::*;

pub(super) fn handle(
    app: &mut FluxApp,
    msg: AppMsg,
    sender: &AsyncComponentSender<FluxApp>,
) -> Result<(), AppMsg> {
    crate::hit!("handle");
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
                crate::ui::panels::apply_diff_markup(buffer, &diff);
            }
            app.active_diff_target = Some(path);
        }
        AppMsg::SetDiffPanelWidth(val) => app.handle_set_diff_panel_width(val),
        AppMsg::SetAutoShowDiff(val) => app.handle_set_auto_show_diff(val),
        AppMsg::ShowGitStatusView => app.handle_show_git_status_view(sender.clone()),
        AppMsg::SetGitRepoActive(is_active) => app.is_in_git_repo = is_active,
        AppMsg::GitApplyPatch(patch) => app.handle_git_apply_patch(patch, sender),
        AppMsg::PromptGitApplyPatch => {
            if let Some(parent) = app.root_window.as_ref() {
                crate::ui::dialog::git_apply_patch::show_apply_patch_dialog(
                    Some(parent.upcast_ref()),
                    sender.clone(),
                );
            }
        }
        AppMsg::GitStageAll => app.handle_git_stage_all(sender),
        AppMsg::GitCommit(msg) => app.handle_git_commit(msg, sender),
        AppMsg::GitDiscardAll => app.handle_git_discard_all(sender),
        AppMsg::GitActionFinished(result) => app.handle_git_action_finished(result, sender),
        AppMsg::GitStageForCommit(paths) => app.handle_git_stage_for_commit(paths, sender),
        AppMsg::GitCommitDialogReady => {
            if let Some(parent) = app.root_window.as_ref() {
                crate::ui::dialog::git_commit::show_commit_dialog(
                    Some(parent.upcast_ref()),
                    sender.clone(),
                );
            }
        }
        AppMsg::PromptCommitWithSelection => {
            let paths = app.get_selection();
            if paths.is_empty() {
                if let Some(parent) = app.root_window.as_ref() {
                    crate::ui::dialog::git_commit::show_commit_dialog(
                        Some(parent.upcast_ref()),
                        sender.clone(),
                    );
                }
            } else {
                app.handle_git_stage_for_commit(paths, sender);
            }
        }
        AppMsg::GitUnstageAll => app.handle_git_unstage_all(sender),
        AppMsg::GitDiscardSelected => app.handle_git_discard_selected(sender),
        AppMsg::GitRestoreLastDiscard => app.handle_git_restore_last_discard(sender),
        AppMsg::GitUndoApply => app.handle_git_undo_apply(sender),
        AppMsg::GitStash => app.handle_git_stash(sender),
        AppMsg::GitStashPop => app.handle_git_stash_pop(sender),
        AppMsg::GitPull => app.handle_git_pull(sender),
        AppMsg::GitPush => app.handle_git_push(sender),
        AppMsg::GitFetch => app.handle_git_fetch(sender),
        AppMsg::PromptGitSwitchBranch => app.handle_prompt_git_switch_branch(sender),
        AppMsg::GitBranchesReady { current, branches } => {
            if let Some(parent) = app.root_window.as_ref() {
                crate::ui::dialog::git_switch_branch::show_switch_branch_dialog(
                    Some(parent.upcast_ref()),
                    sender.clone(),
                    current,
                    branches,
                );
            }
        }
        AppMsg::GitSwitchBranch(name) => app.handle_git_switch_branch(name, sender),
        AppMsg::GitCreateBranch(name) => app.handle_git_create_branch(name, sender),
        AppMsg::GitCopyDiff => app.handle_git_copy_diff(sender),
        AppMsg::GitCopyDiffReady(text) => app.handle_git_copy_diff_ready(text, sender),
        AppMsg::PromptGitAmend => app.handle_prompt_git_amend(sender),
        AppMsg::GitAmendDialogReady(last_message) => {
            if let Some(parent) = app.root_window.as_ref() {
                crate::ui::dialog::git_commit::show_amend_dialog(
                    Some(parent.upcast_ref()),
                    sender.clone(),
                    &last_message,
                );
            }
        }
        AppMsg::GitAmend(msg) => app.handle_git_amend(msg, sender),
        AppMsg::GitShowLog => app.handle_git_show_log(sender),
        AppMsg::GitOutputReady { title, text } => {
            if let Some(parent) = app.root_window.as_ref() {
                crate::ui::dialog::git_output::show_git_output_dialog(
                    Some(parent.upcast_ref()),
                    &title,
                    &text,
                );
            }
        }
        AppMsg::GitOpenRemote => app.handle_git_open_remote(sender),
        AppMsg::ShowGitStatusStaged => app.handle_show_git_status_filtered(
            crate::services::git::StatusFilter::Staged,
            sender.clone(),
        ),
        AppMsg::ShowGitStatusUnstaged => app.handle_show_git_status_filtered(
            crate::services::git::StatusFilter::Unstaged,
            sender.clone(),
        ),
        AppMsg::GitStatusReady {
            path,
            load_id,
            updates,
        } => app.handle_git_status_ready(path, load_id, updates),
        other => return Err(other),
    }
    Ok(())
}
