use crate::model::{AppMsg, FluxApp};
use adw::gdk;
use adw::prelude::*;
use relm4::prelude::*;
use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

pub fn setup_secondary_menu_gesture(
    widget: &impl IsA<gtk::Widget>,
    sender: relm4::AsyncComponentSender<FluxApp>,
) {
    let gesture = gtk::GestureClick::new();
    gesture.set_button(0);
    gesture.set_propagation_phase(gtk::PropagationPhase::Capture);

    let sender_g = sender.clone();
    let widget_weak = widget.as_ref().downgrade();

    gesture.connect_pressed(|g, _n_press, _x, _y| {
        if g.current_button() == 3 {
            let state = g
                .current_event()
                .map(|e| e.modifier_state())
                .unwrap_or(gdk::ModifierType::empty());

            if state.contains(gdk::ModifierType::CONTROL_MASK) {
                g.set_state(gtk::EventSequenceState::Claimed);
            }
        }
    });

    gesture.connect_released(move |g, _n_press, x, y| {
        if g.current_button() != 3 {
            return;
        }

        let state = g
            .current_event()
            .map(|e| e.modifier_state())
            .unwrap_or(gdk::ModifierType::empty());

        if !state.contains(gdk::ModifierType::CONTROL_MASK) {
            return;
        }

        g.set_state(gtk::EventSequenceState::Claimed);

        let mut path: Option<PathBuf> = None;
        let mut rel_x = x;
        let mut rel_y = y;

        if let Some(root) = widget_weak.upgrade() {
            if let Some(picked) = root.pick(x, y, gtk::PickFlags::DEFAULT) {
                // Translate window-relative coordinates (x, y) into GridView space
                if let Some(popover_parent) = picked.ancestor(gtk::GridView::static_type()) {
                    if let Some((tx, ty)) = root.translate_coordinates(&popover_parent, x, y) {
                        rel_x = tx;
                        rel_y = ty;
                    }
                }

                let mut cur: Option<gtk::Widget> = Some(picked);
                while let Some(w) = cur {
                    let data_path: Option<PathBuf> = unsafe {
                        w.data::<Rc<RefCell<Option<PathBuf>>>>("active_path_cell")
                            .map(|ptr| ptr.as_ref().clone())
                            .and_then(|rc| rc.borrow().clone())
                    };
                    if let Some(p) = data_path {
                        path = Some(p);
                        break;
                    }

                    let name = w.widget_name().to_string();
                    if name.starts_with('/')
                        || name.starts_with("trash://")
                        || name.starts_with("smb://")
                        || name.starts_with("sftp://")
                        || name.starts_with("ftp://")
                        || name.starts_with("nfs://")
                        || name.starts_with("archive://")
                    {
                        path = Some(PathBuf::from(name));
                        break;
                    }
                    cur = w.parent();
                }
            }
        }

        sender_g.input(AppMsg::PrepareSecondaryMenu {
            x: rel_x,
            y: rel_y,
            path,
        });
    });

    widget.as_ref().add_controller(gesture);
}
