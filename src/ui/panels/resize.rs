use super::spec::PanelSpec;
use gtk::prelude::*;

/// Returns the workarea width of the monitor the widget's surface is on,
/// or `0` when it cannot be determined (callers should treat `0` as "skip clamp").
fn monitor_workarea_width(widget: &impl IsA<gtk::Widget>) -> i32 {
    let widget = widget.upcast_ref::<gtk::Widget>();
    let Some(native) = widget.native() else {
        return 0;
    };
    let Some(surface) = native.surface() else {
        return 0;
    };
    let Ok(display) = surface.display().downcast::<gtk::gdk::Display>() else {
        return 0;
    };
    display
        .monitor_at_surface(&surface)
        .map_or(0, |m| m.geometry().width())
}

/// Effective upper bound for a panel: `spec.max_width`, hard-clamped to the
/// monitor workarea so the drag gesture can never inflate the toplevel window
/// past the physical screen edge (trivially possible on multi-monitor setups
/// where the compositor has room to grow the window).
///
/// The `0.80` factor is a tunable, not a hard requirement: it guarantees the
/// file grid keeps at least ~20% of the monitor so the main view stays
/// interactive while the panel is dragged open. Raise it if the grid should
/// stay wider, lower it if the panel needs more room on very wide monitors.
/// The panel never goes below `spec.min_width` regardless.
fn max_panel_width(widget: &impl IsA<gtk::Widget>, spec: &PanelSpec) -> i32 {
    let monitor_w = monitor_workarea_width(widget);
    if monitor_w <= 0 {
        return spec.max_width;
    }
    let hard_cap = (monitor_w as f64 * 0.80).round() as i32;
    spec.max_width.min(hard_cap).max(spec.min_width)
}

/// Build `<handle><panel>` with full hover + drag-to-resize behavior.
/// `on_resize_end` fires once on drag release with the final clamped width.
pub fn resizable_panel(
    spec: &PanelSpec,
    panel: &gtk::Box,
    on_resize_end: impl Fn(i32) + 'static,
) -> gtk::Box {
    crate::hit!("resizable_panel");
    let resize_handle = gtk::Separator::builder()
        .orientation(gtk::Orientation::Vertical)
        .css_classes(["sidebar-resize-handle"])
        .build();

    let drag_gesture = gtk::GestureDrag::new();
    let start_width = std::rc::Rc::new(std::cell::Cell::new(spec.effective()));
    let start_root_x = std::rc::Rc::new(std::cell::Cell::new(0.0));
    let hover_timer = std::rc::Rc::new(std::cell::Cell::new(None::<gtk::glib::SourceId>));
    let is_ready = std::rc::Rc::new(std::cell::Cell::new(false));

    let motion_ctrl = gtk::EventControllerMotion::new();
    {
        let timer_c = hover_timer.clone();
        let ready_c = is_ready.clone();
        motion_ctrl.connect_enter(move |ctrl, _, _| {
            if let Some(id) = timer_c.take() {
                id.remove();
            }
            ready_c.set(false);
            let ctrl_weak = ctrl.downgrade();
            let timer_inner = timer_c.clone();
            let ready_inner = ready_c.clone();
            let id = gtk::glib::timeout_add_local_once(
                std::time::Duration::from_millis(100),
                move || {
                    timer_inner.set(None);
                    ready_inner.set(true);
                    if let Some(c) = ctrl_weak.upgrade() {
                        if let Some(widget) = c.widget() {
                            widget.set_cursor_from_name(Some("col-resize"));
                        }
                    }
                },
            );
            timer_c.set(Some(id));
        });
    }
    {
        let timer_c = hover_timer;
        let ready_c = is_ready.clone();
        motion_ctrl.connect_leave(move |ctrl| {
            if let Some(id) = timer_c.take() {
                id.remove();
            }
            ready_c.set(false);
            if let Some(widget) = ctrl.widget() {
                widget.set_cursor(None);
            }
        });
    }
    resize_handle.add_controller(motion_ctrl);

    {
        let panel_weak = panel.downgrade();
        let start_width_c = start_width.clone();
        let start_root_x_c = start_root_x.clone();
        let ready_c = is_ready.clone();
        drag_gesture.connect_drag_begin(move |gesture, x, _| {
            if !ready_c.get() {
                gesture.set_state(gtk::EventSequenceState::Denied);
                return;
            }
            if let Some(p) = panel_weak.upgrade() {
                start_width_c.set(p.width());
                if let Some(root) = p.root() {
                    if let Some(handle) = gesture.widget() {
                        let (rx, _) =
                            crate::utils::helpers::translate_point(&handle, &root, x, 0.0)
                                .unwrap_or((x, 0.0));
                        start_root_x_c.set(rx);
                    }
                }
            }
        });
    }
    {
        let panel_weak = panel.downgrade();
        let start_width_c = start_width.clone();
        let start_root_x_c = start_root_x.clone();
        let ready_c = is_ready.clone();
        let spec = *spec;
        drag_gesture.connect_drag_update(move |gesture, _, _| {
            if !ready_c.get() {
                return;
            }
            if let Some(p) = panel_weak.upgrade() {
                if let Some(root) = p.root() {
                    if let Some(handle) = gesture.widget() {
                        if let Some((curr_x, _)) = gesture.point(None) {
                            if let Some((curr_root_x, _)) =
                                crate::utils::helpers::translate_point(&handle, &root, curr_x, 0.0)
                            {
                                let delta_x = curr_root_x - start_root_x_c.get();
                                let raw = start_width_c.get() - delta_x as i32;
                                let cap = max_panel_width(&p, &spec);
                                let new_w = raw.clamp(spec.min_width, cap);
                                if p.width_request() != new_w {
                                    p.set_width_request(new_w);
                                }
                            }
                        }
                    }
                }
            }
        });
    }
    {
        let panel_weak = panel.downgrade();
        let ready_c = is_ready;
        let spec = *spec;
        drag_gesture.connect_drag_end(move |gesture, _, _| {
            if !ready_c.get() {
                return;
            }
            if let Some(p) = panel_weak.upgrade() {
                let cap = max_panel_width(&p, &spec);
                let final_width = p.width().clamp(spec.min_width, cap);
                p.set_width_request(final_width);
                on_resize_end(final_width);
            }
            if let Some(widget) = gesture.widget() {
                widget.set_cursor(None);
            }
        });
    }
    resize_handle.add_controller(drag_gesture);

    let clamp = adw::Clamp::builder()
        .orientation(gtk::Orientation::Horizontal)
        .maximum_size(i32::MAX)
        .tightening_threshold(i32::MAX)
        .child(panel)
        .build();

    let root_container = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(0)
        .hexpand(false)
        .halign(gtk::Align::End)
        .build();
    root_container.append(&resize_handle);
    root_container.append(&clamp);
    root_container
}
