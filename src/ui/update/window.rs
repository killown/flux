use crate::model::{AppMsg, FluxApp, ToggleVerb, WindowVisibility};
use adw::prelude::*;
use relm4::prelude::*;

/// Applies `verb` to the main window and returns the visibility it ends up with.
fn apply_visibility(verb: ToggleVerb, app: &FluxApp) -> WindowVisibility {
    crate::hit!("apply_visibility");
    let Some(window) = app.root_window.as_ref() else {
        return WindowVisibility::Hidden;
    };
    let visible = match verb {
        ToggleVerb::Toggle => !window.is_visible(),
        ToggleVerb::Show => true,
        ToggleVerb::Hide => false,
    };
    window.set_visible(visible);
    if visible {
        window.present();
        WindowVisibility::Visible
    } else {
        WindowVisibility::Hidden
    }
}

pub(super) fn handle(
    app: &mut FluxApp,
    msg: AppMsg,
    sender: &AsyncComponentSender<FluxApp>,
) -> Result<(), AppMsg> {
    crate::hit!("handle");
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
        AppMsg::SetFishTheme(val) => app.handle_set_fish_theme(val),
        AppMsg::SetMaxHistory(val) => app.handle_set_max_history(val),
        AppMsg::ShowAbout => FluxApp::show_about_window(),
        AppMsg::ShowHelp => {
            let help_win = crate::ui::HelpWindow::builder().launch(()).detach();
            help_win.widget().present();
        }
        AppMsg::OpenDebugWindow => crate::ui::debug::show_debug_window(app),
        AppMsg::ShowToast(msg) => app.handle_show_toast(msg),
        AppMsg::ToggleWindowVisibility { verb, reply } => {
            let _ = reply.send(apply_visibility(verb, app));
        }
        other => return Err(other),
    }
    Ok(())
}
