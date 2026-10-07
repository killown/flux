use crate::model::{AppMsg, FluxApp};
use relm4::prelude::*;

pub(super) fn handle(
    app: &mut FluxApp,
    msg: AppMsg,
    sender: &AsyncComponentSender<FluxApp>,
) -> Result<(), AppMsg> {
    match msg {
        AppMsg::PrepareContextMenu(x, y, path) => {
            app.handle_prepare_context_menu(x, y, path, sender)
        }
        AppMsg::PrepareSecondaryMenu { x, y, path } => {
            app.handle_prepare_secondary_menu(x, y, path, sender)
        }
        AppMsg::ShowContextMenu { x, y, path, mime } => {
            app.build_and_show_context_menu(x, y, path, mime, sender)
        }
        AppMsg::ShowSecondaryMenu {
            x,
            y,
            path,
            mime,
            actions,
        } => app.build_and_show_secondary_menu(x, y, path, mime, actions, sender),
        other => return Err(other),
    }
    Ok(())
}
