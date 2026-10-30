use crate::domain::model::{AppMsg, ActionsMsg, CommandMsg, NavigationMsg, PropertiesMsg, TasksMsg, TransferMsg, FluxApp};
use relm4::prelude::*;

pub(super) fn handle(
    app: &mut FluxApp,
    msg: AppMsg,
    sender: &AsyncComponentSender<FluxApp>,
) -> Result<(), AppMsg> {
    crate::hit!("handle");
    match msg {
        AppMsg::Actions(ActionsMsg::ShowOpenWithDialog(path)) => app.show_open_with_dialog(path, sender),
        AppMsg::Transfer(TransferMsg::FileConflictDetected { context, resolver }) => {
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
        AppMsg::Properties(PropertiesMsg::InspectDirectory(path)) => app.show_dir_inspector_dialog(path, sender),
        AppMsg::Transfer(TransferMsg::ConflictDialogClosed) => app.conflict_dialog_active = false,
        AppMsg::Transfer(TransferMsg::SetConflictPolicy(_)) => {}

        AppMsg::Tasks(TasksMsg::TaskProgress {
            id,
            label,
            current,
            total,
            total_items,
            cancellable,
        }) => app.handle_task_progress(id, label, current, total, total_items, cancellable),
        AppMsg::Tasks(TasksMsg::TaskCompleted(id)) => app.handle_task_completed(id),
        AppMsg::Tasks(TasksMsg::CancelTask(id)) => app.handle_cancel_task(id, sender),
        AppMsg::Tasks(TasksMsg::CancelAllTasks) => app.handle_cancel_all_tasks(sender),
        AppMsg::Tasks(TasksMsg::TaskQueueTick) => app.handle_task_queue_tick(sender),
        AppMsg::Transfer(TransferMsg::ShowTransferDialog) => app.handle_show_transfer_dialog(),
        AppMsg::Transfer(TransferMsg::ShowTransferDialogIfActive(id)) => app.handle_show_transfer_dialog_if_active(id),
        AppMsg::Transfer(TransferMsg::TransferDialogClosed) => app.handle_transfer_dialog_closed(),

        AppMsg::Navigation(NavigationMsg::Open { position, tab_id }) => app.handle_open(position, tab_id, sender),
        AppMsg::Navigation(NavigationMsg::Activate) => app.handle_activate(sender),
        AppMsg::Actions(ActionsMsg::LaunchWithApp(app_id)) => app.handle_launch_with_app(app_id),
        AppMsg::Actions(ActionsMsg::ExecuteCommand(cmd_template)) => app.handle_execute_command(cmd_template, sender),
        AppMsg::Actions(ActionsMsg::ExecuteSecondaryAction {
            command,
            no_command_dialog,
        }) => app.handle_execute_command_with(command, Some(no_command_dialog), sender),
        AppMsg::Actions(ActionsMsg::PromptNewContextualAction { mime }) => {
            app.show_prompt_new_contextual_action(mime, sender)
        }
        AppMsg::Actions(ActionsMsg::SaveContextualAction {
            mime,
            category_scope,
            label,
            submenu,
            mime_types,
            command,
            toast,
            no_command_dialog,
        }) => app.handle_save_contextual_action(
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
        AppMsg::Actions(ActionsMsg::ReloadActionsTemplate { path }) => app.handle_reload_actions_template(path, sender),
        AppMsg::Actions(ActionsMsg::ContextualActionSaved(result)) => app.handle_contextual_action_saved(result, sender),
        AppMsg::Actions(ActionsMsg::ActionsPanelLoad { session }) => app.handle_actions_panel_refresh(session),
        AppMsg::Actions(ActionsMsg::ActionsPanelReady {
            session,
            mime,
            actions,
            template_path,
        }) => app.handle_actions_panel_ready(session, mime, actions, template_path, sender),
        AppMsg::Command(CommandMsg::ToggleNoCommandDialog(action_name)) => {
            app.handle_toggle_no_command_dialog(action_name)
        }
        AppMsg::Command(CommandMsg::RefreshCommandDialog(action_name)) => app.handle_refresh_command_dialog(action_name),
        AppMsg::Command(CommandMsg::ShowCommandDialog(id)) => app.handle_show_command_dialog(id),
        AppMsg::Command(CommandMsg::ShowCommandDialogIfActive(id)) => app.handle_show_command_dialog_if_active(id),
        AppMsg::Command(CommandMsg::CommandOutput {
            id,
            line,
            is_stderr,
        }) => app.handle_command_output(id, line, is_stderr),
        AppMsg::Command(CommandMsg::CommandDialogClosed) => app.handle_command_dialog_closed(),
        AppMsg::Command(CommandMsg::CommandFinished {
            id,
            success,
            exit_code,
        }) => app.handle_command_finished(id, success, exit_code, sender),
        other => return Err(other),
    }
    Ok(())
}
