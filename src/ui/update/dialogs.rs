use crate::model::{AppMsg, FluxApp};
use relm4::prelude::*;

pub(super) fn handle(
    app: &mut FluxApp,
    msg: AppMsg,
    sender: &AsyncComponentSender<FluxApp>,
) -> Result<(), AppMsg> {
    match msg {
        AppMsg::ShowOpenWithDialog(path) => app.show_open_with_dialog(path, sender),
        AppMsg::FileConflictDetected { context, resolver } => {
            if let Some(mut dialog) = app.transfer_dialog.take() {
                dialog.close();
            }
            let tx = resolver
                .lock()
                .expect("conflict resolver mutex poisoned")
                .take()
                .expect("FileConflictDetected handled more than once");
            app.conflict_dialog_active = true;
            crate::ui::dialog::conflict::show_conflict_dialog(context, tx, sender.clone());
        }
        AppMsg::InspectDirectory(path) => app.show_dir_inspector_dialog(path, sender),
        AppMsg::ConflictDialogClosed => app.conflict_dialog_active = false,
        AppMsg::SetConflictPolicy(_) => {}

        AppMsg::TaskProgress {
            id,
            label,
            current,
            total,
            total_items,
            cancellable,
        } => app.handle_task_progress(id, label, current, total, total_items, cancellable),
        AppMsg::TaskCompleted(id) => app.handle_task_completed(id),
        AppMsg::CancelTask(id) => app.handle_cancel_task(id, sender),
        AppMsg::CancelAllTasks => app.handle_cancel_all_tasks(sender),
        AppMsg::TaskQueueTick => app.handle_task_queue_tick(sender),
        AppMsg::ShowTransferDialog => app.handle_show_transfer_dialog(),
        AppMsg::ShowTransferDialogIfActive(id) => app.handle_show_transfer_dialog_if_active(id),
        AppMsg::TransferDialogClosed => app.handle_transfer_dialog_closed(),

        AppMsg::Open { position, tab_id } => app.handle_open(position, tab_id, sender),
        AppMsg::Activate => app.handle_activate(sender),
        AppMsg::LaunchWithApp(app_id) => app.handle_launch_with_app(app_id),
        AppMsg::ExecuteCommand(cmd_template) => app.handle_execute_command(cmd_template, sender),
        AppMsg::ExecuteSecondaryAction {
            command,
            no_command_dialog,
        } => app.handle_execute_command_with(command, Some(no_command_dialog), sender),
        AppMsg::PromptNewContextualAction { mime } => {
            app.show_prompt_new_contextual_action(mime, sender)
        }
        AppMsg::SaveContextualAction {
            mime,
            category_scope,
            label,
            submenu,
            mime_types,
            command,
            toast,
            no_command_dialog,
        } => app.handle_save_contextual_action(
            mime,
            category_scope,
            crate::ui::menu::secondary::NewSecondaryAction {
                label,
                submenu,
                mime_types,
                command,
                toast,
                no_command_dialog,
            },
        ),
        AppMsg::ReloadActionsTemplate { path } => app.handle_reload_actions_template(path, sender),
        AppMsg::ContextualActionSaved(result) => app.handle_contextual_action_saved(result, sender),
        AppMsg::ActionsPanelLoad { session } => app.handle_actions_panel_refresh(session),
        AppMsg::ActionsPanelReady {
            session,
            mime,
            actions,
            template_path,
        } => app.handle_actions_panel_ready(session, mime, actions, template_path, sender),
        AppMsg::ToggleNoCommandDialog(action_name) => {
            app.handle_toggle_no_command_dialog(action_name)
        }
        AppMsg::RefreshCommandDialog(action_name) => app.handle_refresh_command_dialog(action_name),
        AppMsg::ShowCommandDialog(id) => app.handle_show_command_dialog(id),
        AppMsg::ShowCommandDialogIfActive(id) => app.handle_show_command_dialog_if_active(id),
        AppMsg::CommandOutput {
            id,
            line,
            is_stderr,
        } => app.handle_command_output(id, line, is_stderr),
        AppMsg::CommandDialogClosed => app.handle_command_dialog_closed(),
        AppMsg::CommandFinished {
            id,
            success,
            exit_code,
        } => app.handle_command_finished(id, success, exit_code, sender),
        other => return Err(other),
    }
    Ok(())
}
