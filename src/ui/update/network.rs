use crate::model::{AppMsg, FluxApp};
use relm4::prelude::*;

pub(super) fn handle(
    app: &mut FluxApp,
    msg: AppMsg,
    sender: &AsyncComponentSender<FluxApp>,
) -> Result<(), AppMsg> {
    match msg {
        AppMsg::NetworkLoaded { uri, contexts } => app.handle_network_loaded(uri, contexts),
        AppMsg::ConnectToServer { uri, credentials } => {
            app.handle_connect_to_server(uri, credentials, sender)
        }
        AppMsg::UnmountNetwork(uri) => app.handle_unmount_network(uri, sender),
        AppMsg::AddNetworkBookmark { name, uri } => {
            app.handle_add_network_bookmark(name, uri, sender)
        }
        AppMsg::RemoveNetworkBookmark(uri) => app.handle_remove_network_bookmark(uri, sender),
        AppMsg::RefreshNetworkSidebar => app.handle_refresh_network_sidebar(sender),
        AppMsg::SystemMountsReady(mounts) => app.handle_system_mounts_ready(mounts),
        AppMsg::UnmountDevice(path) => app.handle_unmount_device(path, sender),
        AppMsg::UnlockLuksImage { path } => app.show_luks_passphrase_dialog(path, sender),
        AppMsg::LuksMounted { mount_point, .. } => {
            sender.input(AppMsg::Navigate(mount_point));
            sender.input(AppMsg::ShowToast(crate::i18n::tr("Volume mounted.")));
        }
        AppMsg::TerminalCwdChanged(path) => app.handle_terminal_cwd_changed(path, sender),
        AppMsg::SetTerminalShell(shell) => {
            app.config.ui.terminal.shell = shell;
            crate::utils::save_config(&app.config);
        }
        // The single place that decides what F4 means: it follows the configured position.
        AppMsg::ToggleTerminal => match app.config.ui.terminal.position {
            crate::model::TerminalPosition::Bottom => app.handle_toggle_terminal(),
            crate::model::TerminalPosition::Sidebar => {
                app.toggle_sidebar_right_panel(crate::model::RightPanelType::Terminal, sender);
            }
            crate::model::TerminalPosition::Fullscreen => {
                app.handle_toggle_terminal_fullscreen(sender);
            }
        },
        AppMsg::ToggleTerminalFullscreen => app.handle_toggle_terminal_fullscreen(sender),
        AppMsg::SetTerminalPosition(pos) => app.handle_set_terminal_position(pos, sender),
        AppMsg::SetTerminalHeight(h) => app.handle_set_terminal_config(Some(h), None, None, None),
        AppMsg::SetTerminalFont(f) => app.handle_set_terminal_config(None, Some(f), None, None),
        AppMsg::SetTerminalFgColor(c) => app.handle_set_terminal_config(None, None, Some(c), None),
        AppMsg::SetTerminalBgColor(c) => app.handle_set_terminal_config(None, None, None, Some(c)),
        other => return Err(other),
    }
    Ok(())
}
