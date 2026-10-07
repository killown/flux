use crate::model::{AppMsg, FluxApp};
use adw::prelude::*;
use relm4::prelude::*;

pub(super) fn handle(
    app: &mut FluxApp,
    msg: AppMsg,
    sender: &AsyncComponentSender<FluxApp>,
) -> Result<(), AppMsg> {
    match msg {
        AppMsg::SetUiScale(scale) => app.handle_set_ui_scale(scale),
        AppMsg::SetBackgroundAlpha { slot, alpha } => app.handle_set_background_alpha(slot, alpha),
        AppMsg::SetFluxBackground { target, slot } => app.handle_set_flux_background(target, slot),
        AppMsg::ClearFluxBackgrounds => app.handle_clear_flux_backgrounds(sender),
        AppMsg::SetScaleFontWithIcons(val) => app.handle_set_scale_font_with_icons(val, sender),
        AppMsg::ToggleHeaderBar => app.handle_toggle_header_bar(),
        AppMsg::SetTheme(theme) => app.handle_set_theme(theme),
        AppMsg::SetShortcut(key, val) => app.handle_set_shortcut(key, val),
        AppMsg::SetMaximized(max) => app.handle_set_maximized(max),
        AppMsg::SetWindowWidth(val) => app.handle_set_window_size(Some(val), None),
        AppMsg::SetWindowHeight(val) => app.handle_set_window_size(None, Some(val)),
        AppMsg::SetShowRecents(val) => app.handle_set_show_recents(val, sender),
        AppMsg::SetRecentsRow(val) => app.handle_set_recents_row(val, sender),
        AppMsg::SetMaxHistory(val) => app.handle_set_max_history(val),
        AppMsg::ShowAbout => FluxApp::show_about_window(),
        AppMsg::ShowHelp => {
            let help_win = crate::ui::HelpWindow::builder().launch(()).detach();
            help_win.widget().present();
        }
        AppMsg::OpenDebugWindow => crate::ui::debug::show_debug_window(app),
        AppMsg::ShowToast(msg) => app.handle_show_toast(msg),
        other => return Err(other),
    }
    Ok(())
}
