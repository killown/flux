//! Resize guard: hides the sidebar, header bar or status bar when one of them
//! is what stops the window from being made smaller, and shows it again once
//! the window is big enough to hold it.
//!
//! The guard only watches the size of the window. When the window sits at its
//! minimum size, it works out which visible part would free the most room by
//! being hidden and reports it with `WindowMsg::SetAutoHidden`, the view then
//! hides it. When the window grows past the size it had when a part was hidden,
//! that part is reported as shown again. Parts the user has hidden themselves
//! are never touched.

use super::component::FluxAppWidgets;
use crate::app::state::FluxApp;
use crate::domain::model::{AppMsg, WindowMsg};
use adw::prelude::*;
use gtk::glib;
use relm4::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

/// Pixels of tolerance when comparing the window size with its minimum size.
const SLACK: i32 = 2;
/// Time given to the view to apply a visibility change before measuring again.
const SETTLE_MS: u64 = 80;

/// A part of the window the guard can hide.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Part {
    Sidebar,
    Header,
    Statusbar,
}

/// The window dimension that was too small for a hidden part.
#[derive(Clone, Copy)]
enum Axis {
    Width,
    Height,
}

/// A part hidden by the guard, with the window size it needs to be shown again.
#[derive(Clone, Copy)]
struct Hidden {
    part: Part,
    axis: Axis,
    need: i32,
}

/// Weak handles to the window and the parts the guard can hide, plus its state.
struct Guard {
    window: glib::WeakRef<adw::Window>,
    sidebar: glib::WeakRef<gtk::Box>,
    header: glib::WeakRef<adw::HeaderBar>,
    status: glib::WeakRef<gtk::Box>,
    toolbar: glib::WeakRef<adw::ToolbarView>,
    tabs: glib::WeakRef<adw::TabBar>,
    /// Parts hidden by the guard, in the order they were hidden.
    hidden: Vec<Hidden>,
    /// Last window size reported by the surface.
    size: (i32, i32),
    /// True while a visibility change is still being applied by the view.
    settling: bool,
    sender: AsyncComponentSender<FluxApp>,
}

/// Minimum size of `widget` along `orientation`, or 0 when it is hidden.
fn min_size(widget: &impl IsA<gtk::Widget>, orientation: gtk::Orientation) -> i32 {
    if widget.is_visible() {
        widget.measure(orientation, -1).0
    } else {
        0
    }
}

fn is_hidden(hidden: &[Hidden], part: Part) -> bool {
    hidden.iter().any(|h| h.part == part)
}

/// Picks the visible part whose removal lowers the minimum window width the most.
fn widest_part(
    hidden: &[Hidden],
    sidebar: &gtk::Box,
    header: &adw::HeaderBar,
    status: &gtk::Box,
    toolbar: &adw::ToolbarView,
    tabs: &adw::TabBar,
) -> Option<Part> {
    let h = gtk::Orientation::Horizontal;
    let content = toolbar.content().map_or(0, |c| min_size(&c, h));
    let tabs_w = min_size(tabs, h);
    let bar_w = min_size(toolbar, h);
    // The sidebar sits beside the toolbar view, so its width is added to the
    // window. The header and status bar share the toolbar view's width, so
    // hiding one only helps when it is the widest of them.
    let gains = [
        (Part::Sidebar, min_size(sidebar, h)),
        (
            Part::Header,
            bar_w - tabs_w.max(content).max(min_size(status, h)),
        ),
        (
            Part::Statusbar,
            bar_w - tabs_w.max(content).max(min_size(header, h)),
        ),
    ];
    gains
        .into_iter()
        .filter(|(part, gain)| *gain > 0 && !is_hidden(hidden, *part))
        .max_by_key(|(_, gain)| *gain)
        .map(|(part, _)| part)
}

/// Picks the visible part whose removal lowers the minimum window height the most.
fn tallest_part(hidden: &[Hidden], header: &adw::HeaderBar, status: &gtk::Box) -> Option<Part> {
    let v = gtk::Orientation::Vertical;
    [
        (Part::Header, min_size(header, v)),
        (Part::Statusbar, min_size(status, v)),
    ]
    .into_iter()
    .filter(|(part, gain)| *gain > 0 && !is_hidden(hidden, *part))
    .max_by_key(|(_, gain)| *gain)
    .map(|(part, _)| part)
}

/// Checks the window size once: shows the last hidden part if there is room
/// again, otherwise hides one part if the window is stuck at its minimum size.
fn evaluate(guard: &Rc<RefCell<Guard>>) {
    let mut g = guard.borrow_mut();
    if g.settling {
        return;
    }
    let (w, h) = g.size;
    if w <= 0 || h <= 0 {
        return;
    }
    let (Some(window), Some(sidebar), Some(header), Some(status), Some(toolbar), Some(tabs)) = (
        g.window.upgrade(),
        g.sidebar.upgrade(),
        g.header.upgrade(),
        g.status.upgrade(),
        g.toolbar.upgrade(),
        g.tabs.upgrade(),
    ) else {
        return;
    };

    let room_again = g.hidden.last().is_some_and(|top| {
        let size = match top.axis {
            Axis::Width => w,
            Axis::Height => h,
        };
        size > top.need + SLACK
    });

    if room_again {
        g.hidden.pop();
    } else {
        let min_w = window.measure(gtk::Orientation::Horizontal, -1).0;
        let min_h = window.measure(gtk::Orientation::Vertical, w).0;
        eprintln!(
            "[resize-guard] surface={w}x{h} win_min={min_w}x{min_h} sidebar_min={} header_min={} status_min={} hidden={}",
            min_size(&sidebar, gtk::Orientation::Horizontal),
            min_size(&header, gtk::Orientation::Horizontal),
            min_size(&status, gtk::Orientation::Horizontal),
            g.hidden.len()
        );
        let by_width = (w <= min_w + SLACK)
            .then(|| widest_part(&g.hidden, &sidebar, &header, &status, &toolbar, &tabs))
            .flatten()
            .map(|part| Hidden {
                part,
                axis: Axis::Width,
                need: min_w,
            });
        let by_height = || {
            (h <= min_h + SLACK)
                .then(|| tallest_part(&g.hidden, &header, &status))
                .flatten()
                .map(|part| Hidden {
                    part,
                    axis: Axis::Height,
                    need: min_h,
                })
        };
        let next = by_width.or_else(by_height);
        eprintln!(
            "[resize-guard] decision={}",
            next.map_or("none", |n| match n.part {
                Part::Sidebar => "hide sidebar",
                Part::Header => "hide header",
                Part::Statusbar => "hide statusbar",
            })
        );
        let Some(next) = next else {
            return;
        };
        g.hidden.push(next);
    }

    let sidebar_hidden = is_hidden(&g.hidden, Part::Sidebar);
    let header_hidden = is_hidden(&g.hidden, Part::Header);
    let statusbar_hidden = is_hidden(&g.hidden, Part::Statusbar);
    g.sender.input(AppMsg::Window(WindowMsg::SetAutoHidden {
        sidebar: sidebar_hidden,
        header: header_hidden,
        statusbar: statusbar_hidden,
    }));

    // Measure again only after the view has applied the change, so a second
    // part is hidden only when the first one was not enough.
    g.settling = true;
    let again = guard.clone();
    glib::timeout_add_local_once(Duration::from_millis(SETTLE_MS), move || {
        again.borrow_mut().settling = false;
        evaluate(&again);
    });
}

/// Re-checks the window whenever its size changes. The frame clock only exists
/// once the window is realized, and runs the layout phase on every resize.
fn attach(guard: &Rc<RefCell<Guard>>, win: &adw::Window) {
    let Some(clock) = win.frame_clock() else {
        return;
    };
    let guard = guard.clone();
    let window = win.downgrade();
    clock.connect_layout(move |_| {
        let Some(window) = window.upgrade() else {
            return;
        };
        let size = (window.width(), window.height());
        eprintln!("[resize-guard] frame-clock layout {}x{}", size.0, size.1);
        {
            let mut g = guard.borrow_mut();
            if g.size == size {
                return;
            }
            g.size = size;
        }
        evaluate(&guard);
    });
}

impl FluxApp {
    /// Watches the window size and hides or shows the sidebar, header bar and
    /// status bar when one of them blocks the window from being resized.
    pub(super) fn setup_resize_guard(
        model: &FluxApp,
        widgets: &FluxAppWidgets,
        root: &adw::Window,
        sender: &AsyncComponentSender<Self>,
    ) {
        crate::hit!("setup_resize_guard");
        let guard = Rc::new(RefCell::new(Guard {
            window: root.downgrade(),
            sidebar: widgets.sidebar_box.downgrade(),
            header: widgets.header_bar.downgrade(),
            status: widgets.status_bar.downgrade(),
            toolbar: widgets.toolbar_view.downgrade(),
            tabs: model.tab_bar.downgrade(),
            hidden: Vec::new(),
            size: (0, 0),
            settling: false,
            sender: sender.clone(),
        }));

        if root.is_realized() {
            attach(&guard, root);
        }
        root.connect_realize(move |win| attach(&guard, win));
    }
}
