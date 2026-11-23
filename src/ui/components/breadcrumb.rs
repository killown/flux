use crate::domain::model::PathSegment;
use crate::ui::constants;
use adw::prelude::*;
use relm4::prelude::*;
use std::path::PathBuf;

#[relm4::factory(pub)]
impl FactoryComponent for PathSegment {
    type Init = PathSegment;
    type Input = ();
    type Output = PathBuf;
    type ParentWidget = gtk::Box;
    type CommandOutput = ();

    fn init_model(init: Self::Init, _index: &DynamicIndex, _sender: FactorySender<Self>) -> Self {
        init
    }

    view! {
        #[root]
        gtk::Button {
            add_css_class: constants::BREADCRUMB_BTN_CLASS,
            // Header widgets never take keyboard focus (see lock_header_focus).
            set_focusable: false,
            set_focusable: false,
            set_focusable: false,
            #[wrap(Some)]
            set_child = &gtk::Label {
                #[watch]
                set_label: &self.name,
                // `max_chars` <= 0 means unlimited: show the whole name.
                set_ellipsize: if self.max_chars > 0 {
                    gtk::pango::EllipsizeMode::End
                } else {
                    gtk::pango::EllipsizeMode::None
                },
                set_max_width_chars: if self.max_chars > 0 { self.max_chars } else { -1 },
                set_wrap: false,
            },

            connect_clicked[sender, path = self.path.clone()] => move |_| {
                let _ = sender.output(path.clone());
            },

            // Middle-click: add this specific breadcrumb folder to Quick List
            add_controller = gtk::GestureClick {
                set_button: 2, // constants::MOUSE_MIDDLE
                connect_pressed[path = self.path.clone()] => move |gesture, _, _, _| {
                    gesture.set_state(gtk::EventSequenceState::Claimed);
                    if let Some(s) = crate::infra::SENDER.get() {
                        let _ = s.send(crate::domain::model::AppMsg::QuickList(crate::domain::model::QuickListMsg::AddExclusive(Some(path.clone()))));
                    }
                }
            }
        }
    }
}

/// Shows as many breadcrumb buttons as fit in `scroller`, dropping the oldest
/// parents first. Labels are never shortened here, and the last button (the
/// current folder) always stays visible. Run it whenever the bar is resized
/// or the segments are rebuilt.
///
/// The hidden buttons are replaced by a start margin of the same width, so the
/// bar keeps asking for the width of the whole path. That lets the header bar
/// move the bar off-center and use the free room on either side, and bring the
/// parents back when the window grows.
pub fn fit_to_width(scroller: &gtk::ScrolledWindow) {
    let available = scroller.hadjustment().page_size() as i32;
    if available <= 0 {
        return;
    }
    let Some(bar) = scroller
        .child()
        .and_then(|c| c.downcast::<gtk::Viewport>().ok())
        .and_then(|v| v.child())
    else {
        return;
    };
    let mut buttons = Vec::new();
    let mut child = bar.first_child();
    while let Some(w) = child {
        child = w.next_sibling();
        buttons.push(w);
    }
    let Some(last) = buttons.len().checked_sub(1) else {
        return;
    };
    // A hidden widget measures as 0, so show everything before measuring.
    bar.set_margin_start(0);
    for b in &buttons {
        b.set_visible(true);
    }
    let widths: Vec<i32> = buttons
        .iter()
        .map(|w| w.measure(gtk::Orientation::Horizontal, -1).1)
        .collect();
    let mut used = widths[last];
    let mut first = last;
    for i in (0..last).rev() {
        if used + widths[i] > available {
            break;
        }
        used += widths[i];
        first = i;
    }
    for (i, b) in buttons.iter().enumerate() {
        b.set_visible(i >= first);
    }
    bar.set_margin_start(widths[..first].iter().sum());
}
