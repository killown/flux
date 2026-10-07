use adw::gdk;
use gtk::glib;
use gtk::prelude::*;

/// Toggles `single_click_activate` off while any key is held, and restores it on
/// the release of the last modifier - but only when `config_single_click` is set.
pub fn setup_click_toggle(
    window: &impl IsA<gtk::Widget>,
    grid_view: &gtk::GridView,
    config_single_click: bool,
) {
    let controller = gtk::EventControllerKey::new();
    controller.set_propagation_phase(gtk::PropagationPhase::Capture);

    let grid_view_press = grid_view.clone();
    controller.connect_key_pressed(move |_, _keyval, _, _| {
        grid_view_press.set_single_click_activate(false);
        glib::Propagation::Proceed
    });

    let grid_view_release = grid_view.clone();
    controller.connect_key_released(move |_, keyval, _, state| {
        // GTK reports state *before* the release, so the released key's
        // modifier bit is still set. Strip it out manually so we can tell
        // whether any other key is still held.
        let released_mask = match keyval {
            gdk::Key::Control_L | gdk::Key::Control_R => gdk::ModifierType::CONTROL_MASK,
            gdk::Key::Shift_L | gdk::Key::Shift_R => gdk::ModifierType::SHIFT_MASK,
            gdk::Key::Alt_L | gdk::Key::Alt_R => gdk::ModifierType::ALT_MASK,
            gdk::Key::Super_L | gdk::Key::Super_R => gdk::ModifierType::SUPER_MASK,
            gdk::Key::Hyper_L | gdk::Key::Hyper_R => gdk::ModifierType::HYPER_MASK,
            gdk::Key::Meta_L | gdk::Key::Meta_R => gdk::ModifierType::META_MASK,
            _ => gdk::ModifierType::empty(),
        };

        let relevant = gdk::ModifierType::CONTROL_MASK
            | gdk::ModifierType::SHIFT_MASK
            | gdk::ModifierType::ALT_MASK
            | gdk::ModifierType::SUPER_MASK
            | gdk::ModifierType::HYPER_MASK
            | gdk::ModifierType::META_MASK;

        let remaining = (state & relevant) - released_mask;

        if remaining.is_empty() && config_single_click {
            grid_view_release.set_single_click_activate(true);
        }
    });

    window.add_controller(controller);
}
