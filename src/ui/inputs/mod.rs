mod capture;
mod click_toggle;
mod deselect;
mod middle_click;
mod mouse;
mod navigation;
mod shortcuts;
mod util;

use crate::model::FluxApp;
use crate::ui::keymap::KeyMap;
use gtk::prelude::*;
use relm4::prelude::*;

/// Attaches all input controllers to the window/view.
///
/// `terminal_area` is the [`gtk::DrawingArea`] that backs the embedded terminal.
/// When it holds keyboard focus, `Tab` is yielded to it (shell completion) instead
/// of being consumed by the file-grid cycle shortcut.
pub fn setup_controllers(
    window: &impl IsA<gtk::Widget>,
    grid_view: &gtk::GridView,
    sender: relm4::AsyncComponentSender<FluxApp>,
    header_stack: &gtk::Stack,
    config_single_click: bool,
    keymap: &KeyMap,
    terminal_area: &gtk::DrawingArea,
) {
    grid_view.set_enable_rubberband(true);

    navigation::setup_navigation(window, header_stack, sender.clone());
    click_toggle::setup_click_toggle(window, grid_view, config_single_click);
    middle_click::setup_middle_click(window, sender.clone());
    capture::setup_capture(window, sender.clone(), terminal_area);
    navigation::setup_history(window, sender.clone(), keymap);
    shortcuts::setup_global_shortcuts(window, sender.clone(), keymap);
    mouse::setup_swipe(window, sender.clone());
    mouse::setup_mouse_back_forward(window, sender.clone());
    shortcuts::setup_settings_shortcut(window, keymap);
    deselect::setup_deselect_on_background_click(grid_view, sender.clone());

    crate::ui::menu::secondary::setup_secondary_menu_gesture(window, sender);
}
