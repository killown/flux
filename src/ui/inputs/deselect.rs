use crate::model::FluxApp;
use crate::ui::inputs::util::find_flux_path_ancestor;
use adw::gdk;
use gtk::prelude::*;
use relm4::prelude::*;

/// Clears the `MultiSelection` when the user clicks empty background space,
/// unless Ctrl/Shift is held (multi-selection modifiers) or the click landed
/// on a file/dir item.
pub fn setup_deselect_on_background_click(
    grid_view: &gtk::GridView,
    _sender: relm4::AsyncComponentSender<FluxApp>,
) {
    let deselect = gtk::GestureClick::new();
    deselect.set_button(1);
    // Bubble phase so individual GridView child cells get first refusal.
    deselect.set_propagation_phase(gtk::PropagationPhase::Bubble);

    let grid_view_weak = grid_view.downgrade();

    deselect.connect_pressed(move |gesture, _, x, y| {
        let modifiers = gesture
            .current_event()
            .map(|e| e.modifier_state())
            .unwrap_or(gdk::ModifierType::empty());

        let multi_select =
            modifiers.intersects(gdk::ModifierType::CONTROL_MASK | gdk::ModifierType::SHIFT_MASK);
        if multi_select {
            return;
        }

        let Some(grid_view) = grid_view_weak.upgrade() else {
            return;
        };

        let hit_item = grid_view
            .pick(x, y, gtk::PickFlags::DEFAULT)
            .and_then(find_flux_path_ancestor)
            .is_some();

        if !hit_item {
            if let Some(model) = grid_view.model().and_downcast::<gtk::MultiSelection>() {
                model.unselect_all();
            }
            gesture.set_state(gtk::EventSequenceState::Claimed);
        }
    });

    grid_view.add_controller(deselect);
}
