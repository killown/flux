//! Event-controller helpers: header menus, the right-click menu, Ctrl+scroll zoom,
//! drag-to-pan and the double-click guard.

use super::registry;
use super::zoom::ZOOM_STEP;
use crate::app::state::FluxApp;
use crate::domain::model::{ActionsMsg, AppMsg};
use adw::prelude::*;
use relm4::AsyncComponentSender;
use std::cell::Cell;
use std::rc::Rc;

/// True when the widget (or an ancestor) handles double-click itself: text, buttons, sliders, entries.
pub(super) fn double_click_reserved(widget: &gtk::Widget) -> bool {
    crate::hit!("double_click_reserved");
    let mut cur = Some(widget.clone());
    while let Some(w) = cur {
        if w.is::<gtk::TextView>()
            || w.is::<gtk::Button>()
            || w.is::<gtk::Range>()
            || w.is::<gtk::Scrollbar>()
            || w.is::<gtk::Editable>()
            || w.is::<gtk::GridView>()
        {
            return true;
        }
        cur = w.parent();
    }
    false
}

/// Opens the primary or secondary menu for the previewed file, anchored under `btn`.
pub(super) fn attach_menu_button(
    btn: &gtk::Button,
    sender: AsyncComponentSender<FluxApp>,
    secondary: bool,
) {
    btn.connect_clicked(move |b| {
        let (x, y) = crate::utils::helpers::to_popover_space(
            b.upcast_ref(),
            f64::from(b.width()) / 2.0,
            f64::from(b.height()),
        );
        // `None` makes the handler resolve the selected file, which the preview follows.
        sender.input(if secondary {
            AppMsg::Actions(ActionsMsg::PrepareSecondaryMenu { x, y, path: None })
        } else {
            AppMsg::Actions(ActionsMsg::PrepareContextMenu(x, y, None))
        });
    });
}

/// Right-click opens the primary menu for the previewed (selected) file.
pub(super) fn attach_context_menu(
    panel: &gtk::Box,
    text_view: &gtk::TextView,
    sender: AsyncComponentSender<FluxApp>,
) {
    crate::hit!("attach_context_menu");
    let gesture = gtk::GestureClick::new();
    gesture.set_button(gtk::gdk::BUTTON_SECONDARY);
    gesture.set_propagation_phase(gtk::PropagationPhase::Capture);

    let tv = text_view.clone();
    gesture.connect_pressed(move |g, _, x, y| {
        let Some(panel) = g.widget() else { return };

        // Ctrl+right-click belongs to the window-level secondary-menu gesture.
        let ctrl = g.current_event().is_some_and(|e| {
            e.modifier_state()
                .contains(gtk::gdk::ModifierType::CONTROL_MASK)
        });

        if ctrl || crate::utils::load_config().ui.panel_menu_requires_ctrl {
            return;
        }

        // Keep the native copy menu when right-clicking selected text.
        let on_text = panel.pick(x, y, gtk::PickFlags::DEFAULT).is_some_and(|w| {
            w.upcast_ref::<gtk::Widget>() == tv.upcast_ref::<gtk::Widget>() || w.is_ancestor(&tv)
        });
        if on_text && tv.buffer().has_selection() {
            return;
        }

        g.set_state(gtk::EventSequenceState::Claimed);
        let (rx, ry) = crate::utils::helpers::to_popover_space(&panel, x, y);
        // `None` makes the handler resolve the selected file, which the preview follows.
        sender.input(AppMsg::Actions(ActionsMsg::PrepareContextMenu(
            rx, ry, None,
        )));
    });
    panel.add_controller(gesture);
}

pub(super) fn attach_zoom_scroll(sw: &gtk::ScrolledWindow) {
    let ctl = gtk::EventControllerScroll::new(gtk::EventControllerScrollFlags::VERTICAL);
    ctl.set_propagation_phase(gtk::PropagationPhase::Capture);
    ctl.connect_scroll(|c, _dx, dy| {
        if !c
            .current_event_state()
            .contains(gtk::gdk::ModifierType::CONTROL_MASK)
        {
            return glib::Propagation::Proceed;
        }
        registry::with(|h| h.zoom_by(if dy < 0.0 { ZOOM_STEP } else { 1.0 / ZOOM_STEP }));
        glib::Propagation::Stop
    });
    sw.add_controller(ctl);
}

pub(super) fn attach_pan(sw: &gtk::ScrolledWindow) {
    let drag = gtk::GestureDrag::new();
    drag.set_button(gtk::gdk::BUTTON_PRIMARY);
    let origin = Rc::new(Cell::new((0.0_f64, 0.0_f64)));
    {
        let sw = sw.clone();
        let origin = origin.clone();
        drag.connect_drag_begin(move |_, _, _| {
            origin.set((sw.hadjustment().value(), sw.vadjustment().value()));
        });
    }
    {
        let sw = sw.clone();
        drag.connect_drag_update(move |_, dx, dy| {
            let (ox, oy) = origin.get();
            sw.hadjustment().set_value(ox - dx);
            sw.vadjustment().set_value(oy - dy);
        });
    }
    sw.add_controller(drag);
}
