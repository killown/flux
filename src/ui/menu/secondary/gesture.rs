use crate::domain::model::{ActionsMsg, AppMsg, FluxApp};
use adw::gdk;
use adw::prelude::*;
use relm4::prelude::*;
use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

/// Returns whether `(x, y)` in `root` lands inside the left sidebar, which owns its own context menus.
fn is_over_sidebar(root: &gtk::Widget, x: f64, y: f64) -> bool {
    let mut cur = root.pick(x, y, gtk::PickFlags::DEFAULT);
    while let Some(w) = cur {
        if w.has_css_class(crate::ui::constants::SIDEBAR_ROOT_CSS_CLASS) {
            return true;
        }
        cur = w.parent();
    }
    false
}

pub fn setup_secondary_menu_gesture(
    widget: &impl IsA<gtk::Widget>,
    sender: relm4::AsyncComponentSender<FluxApp>,
) {
    crate::hit!("setup_secondary_menu_gesture");
    let gesture = gtk::GestureClick::new();
    gesture.set_button(0);
    gesture.set_propagation_phase(gtk::PropagationPhase::Capture);

    let sender_g = sender.clone();
    let widget_weak = widget.as_ref().downgrade();
    let press_weak = widget_weak.clone();

    gesture.connect_pressed(move |g, _n_press, x, y| {
        if g.current_button() == 3 {
            if press_weak
                .upgrade()
                .is_some_and(|root| is_over_sidebar(&root, x, y))
            {
                return;
            }

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

        if widget_weak
            .upgrade()
            .is_some_and(|root| is_over_sidebar(&root, x, y))
        {
            return;
        }

        g.set_state(gtk::EventSequenceState::Claimed);

        let mut path: Option<PathBuf> = None;
        let mut rel_x = x;
        let mut rel_y = y;

        if let Some(root) = widget_weak.upgrade() {
            if let Some(picked) = root.pick(x, y, gtk::PickFlags::DEFAULT) {
                // Translate window-relative coordinates (x, y) into popover-parent space
                if let Some(popover_parent) = picked.ancestor(gtk::GridView::static_type()) {
                    if let Some((tx, ty)) =
                        crate::utils::helpers::translate_point(&root, &popover_parent, x, y)
                    {
                        rel_x = tx;
                        rel_y = ty;
                    }
                } else {
                    // Outside the grid (e.g. the preview panel): map into the overlay instead.
                    (rel_x, rel_y) = crate::utils::helpers::to_popover_space(&root, x, y);
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

        sender_g.input(AppMsg::Actions(ActionsMsg::PrepareSecondaryMenu {
            x: rel_x,
            y: rel_y,
            path,
        }));
    });

    widget.as_ref().add_controller(gesture);
}
