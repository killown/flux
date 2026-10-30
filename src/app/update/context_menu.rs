use crate::domain::model::{ActionsMsg, AppMsg, FluxApp};
use relm4::prelude::*;

#[allow(clippy::result_large_err)]
pub(super) fn handle(
    app: &mut FluxApp,
    msg: AppMsg,
    sender: &AsyncComponentSender<FluxApp>,
) -> Result<(), AppMsg> {
    crate::hit!("handle");
    match msg {
        AppMsg::Actions(ActionsMsg::PrepareContextMenu(x, y, path)) => {
            app.handle_prepare_context_menu(x, y, path, sender)
        }
        AppMsg::Actions(ActionsMsg::PrepareSecondaryMenu { x, y, path }) => {
            app.handle_prepare_secondary_menu(x, y, path, sender)
        }
        AppMsg::Actions(ActionsMsg::ShowContextMenu { x, y, path, mime }) => {
            app.build_and_show_context_menu(x, y, path, mime, sender)
        }
        AppMsg::Actions(ActionsMsg::ShowSecondaryMenu {
            x,
            y,
            path,
            mime,
            actions,
        }) => app.build_and_show_secondary_menu(x, y, path, mime, actions, sender),
        other => return Err(other),
    }
    Ok(())
}
