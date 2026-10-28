use adw::gdk;
use gtk::glib;
use gtk::prelude::*;
use std::cell::Cell;
use std::rc::Rc;

/// Returns the grid view of every open tab.
fn tab_grids(tab_view: &adw::TabView) -> Vec<gtk::GridView> {
    (0..tab_view.n_pages())
        .filter_map(|i| {
            tab_view
                .nth_page(i)
                .child()
                .downcast::<gtk::ScrolledWindow>()
                .ok()?
                .child()?
                .downcast::<gtk::GridView>()
                .ok()
        })
        .collect()
}

/// Toggles `single_click_activate` off on every tab's grid while any key is held,
/// and restores the value it had before on the release of the last modifier.
pub fn setup_click_toggle(window: &impl IsA<gtk::Widget>, tab_view: &adw::TabView) {
    crate::hit!("setup_click_toggle");
    let controller = gtk::EventControllerKey::new();
    controller.set_propagation_phase(gtk::PropagationPhase::Capture);

    let saved: Rc<Cell<Option<bool>>> = Rc::new(Cell::new(None));

    let tab_view_press = tab_view.clone();
    let saved_press = saved.clone();
    controller.connect_key_pressed(move |_, _keyval, _, _| {
        let grids = tab_grids(&tab_view_press);
        if saved_press.get().is_none() {
            saved_press.set(Some(grids.iter().any(|g| g.is_single_click_activate())));
        }
        for grid in &grids {
            grid.set_single_click_activate(false);
        }
        glib::Propagation::Proceed
    });

    let tab_view_release = tab_view.clone();
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

        if remaining.is_empty() {
            if let Some(previous) = saved.take() {
                for grid in tab_grids(&tab_view_release) {
                    grid.set_single_click_activate(previous);
                }
            }
        }
    });

    window.add_controller(controller);
}
