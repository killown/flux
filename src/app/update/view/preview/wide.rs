//! Wide-preview and full-screen-preview mode: the state machine on `FluxApp`
//! plus the GTK layout helpers that resize the panel and hide the main column.

use crate::app::state::FluxApp;
use crate::ui::panels::preview::registry;
use adw::prelude::*;
use relm4::prelude::*;

impl FluxApp {
    /// Toggles wide-preview mode (sidebar hidden, panel fills the remaining width).
    pub fn handle_toggle_wide_preview(&mut self, sender: &AsyncComponentSender<Self>) {
        crate::hit!("handle_toggle_wide_preview");
        if self.preview.wide_sidebar.is_some() {
            self.set_wide_preview_default(false);
            self.exit_wide_preview();
            return;
        }

        if !self.panels.preview.visible {
            self.handle_toggle_preview(sender);
        }
        self.set_wide_preview_default(true);
        self.enter_wide_preview();
    }

    /// Header-button handler: stores the permanent wide setting and applies it right away.
    pub fn handle_set_wide_preview_default(&mut self, on: bool) {
        crate::hit!("handle_set_wide_preview_default");
        if self.config.ui.wide_preview == on {
            return;
        }
        self.set_wide_preview_default(on);
        if on {
            self.enter_wide_preview();
        } else {
            self.exit_wide_preview();
        }
    }

    /// Saves `config.ui.wide_preview` and syncs the panel's header button.
    fn set_wide_preview_default(&mut self, on: bool) {
        if self.config.ui.wide_preview != on {
            self.config.ui.wide_preview = on;
            crate::utils::save_config(&self.config);
        }
        let _ = registry::with(|h| h.set_wide_default_active(on));
    }

    /// Enters wide-preview mode when the panel is open, no-op if already wide.
    pub(super) fn enter_wide_preview(&mut self) {
        crate::hit!("enter_wide_preview");
        if self.preview.wide_sidebar.is_some() || !self.panels.preview.visible {
            return;
        }
        let Some(revealer) = self.panels.preview.revealer.clone() else {
            return;
        };

        self.preview.wide_sidebar = Some(self.sidebar_visible);
        self.apply_sidebar_visibility(false);
        set_wide_preview_layout(
            &revealer,
            true,
            self.is_list_mode,
            self.config.ui.preview_panel_width,
        );
    }

    /// Toggles full-screen preview: the main column (header bar, tab bar, file grid, status bar)
    /// is hidden, the window goes full screen and the preview panel fills it.
    pub fn handle_toggle_preview_fullscreen(&mut self) {
        crate::hit!("handle_toggle_preview_fullscreen");
        use gtk::prelude::*;

        if self.preview.fullscreen.is_some() {
            self.leave_preview_fullscreen(true);
            return;
        }
        if !self.panels.preview.visible {
            return;
        }
        let Some(revealer) = self.panels.preview.revealer.clone() else {
            return;
        };

        let was_wide = self.preview.wide_sidebar.is_some();
        self.enter_wide_preview();

        let window = revealer.root().and_downcast::<gtk::Window>();
        let window_was_fullscreen = window.as_ref().is_some_and(|w| w.is_fullscreen());
        set_main_column_visible(&revealer, false);
        if let Some(w) = window.as_ref() {
            w.fullscreen();
        }
        self.preview.fullscreen = Some((was_wide, window_was_fullscreen));
        let _ = registry::with(|h| h.set_fullscreen_active(true));
    }

    /// Esc: leaves full-screen preview when active.
    pub fn handle_exit_preview_fullscreen(&mut self) {
        self.leave_preview_fullscreen(true);
    }

    /// Restores the main column and the window state, no-op when full-screen preview is off.
    ///
    /// `restore_wide` also drops the wide layout if full-screen had to turn it on, callers that
    /// are leaving wide mode themselves pass `false`.
    pub(crate) fn leave_preview_fullscreen(&mut self, restore_wide: bool) {
        crate::hit!("leave_preview_fullscreen");
        use gtk::prelude::*;

        let Some((was_wide, window_was_fullscreen)) = self.preview.fullscreen.take() else {
            return;
        };
        if let Some(revealer) = self.panels.preview.revealer.as_ref() {
            set_main_column_visible(revealer, true);
            if !window_was_fullscreen {
                if let Some(w) = revealer.root().and_downcast::<gtk::Window>() {
                    w.unfullscreen();
                }
            }
        }
        let _ = registry::with(|h| h.set_fullscreen_active(false));
        if restore_wide && !was_wide {
            self.exit_wide_preview();
        }
    }

    /// Re-applies the wide-preview split after a view-mode change.
    pub fn refresh_wide_preview_layout(&self) {
        crate::hit!("refresh_wide_preview_layout");
        if self.preview.wide_sidebar.is_none() {
            return;
        }
        if let Some(revealer) = self.panels.preview.revealer.as_ref() {
            set_wide_preview_layout(
                revealer,
                true,
                self.is_list_mode,
                self.config.ui.preview_panel_width,
            );
        }
    }

    /// Leaves wide-preview mode, restoring the sidebar and the normal panel layout, no-op when inactive.
    pub fn exit_wide_preview(&mut self) {
        crate::hit!("exit_wide_preview");
        // Leaving wide mode also ends full-screen preview, otherwise the grid would stay hidden.
        self.leave_preview_fullscreen(false);
        let Some(sidebar_was_visible) = self.preview.wide_sidebar.take() else {
            return;
        };
        self.apply_sidebar_visibility(sidebar_was_visible);
        if let Some(revealer) = self.panels.preview.revealer.clone() {
            set_wide_preview_layout(&revealer, false, false, self.config.ui.preview_panel_width);
        }
    }

    /// Shows or hides the sidebar for this session only, without touching the saved config.
    fn apply_sidebar_visibility(&mut self, visible: bool) {
        use gtk::prelude::*;
        self.sidebar_visible = visible;
        if let Some(ref widget) = self.sidebar_widget {
            widget.set_visible(visible);
        }
    }
}

/// Minimum width kept for the file list in wide-preview mode, so names are not trimmed.
const WIDE_PREVIEW_LIST_MIN_WIDTH: i32 = 480;
/// Minimum width always left to the preview panel in wide-preview mode.
const WIDE_PREVIEW_PANEL_MIN_WIDTH: i32 = 360;

/// Width in logical pixels of the monitor the widget is on, or 0 when unknown.
fn monitor_width(widget: &impl IsA<gtk::Widget>) -> i32 {
    use gtk::prelude::*;

    let Some(native) = widget.native() else {
        return 0;
    };
    let Some(surface) = native.surface() else {
        return 0;
    };
    gtk::gdk::Display::default()
        .and_then(|d| d.monitor_at_surface(&surface))
        .map_or(0, |m| m.geometry().width())
}

/// Shows or hides the main column (header bar, tab bar, file grid, status bar) next to the preview.
fn set_main_column_visible(revealer: &gtk::Revealer, visible: bool) {
    crate::hit!("set_main_column_visible");
    use gtk::prelude::*;

    let mut sibling = revealer.parent().and_then(|row| row.first_child());
    while let Some(widget) = sibling {
        if widget.is::<adw::ToolbarView>() {
            widget.set_visible(visible);
        }
        sibling = widget.next_sibling();
    }
}

/// Switches the preview between its capped fixed width and filling the space the list leaves.
fn set_wide_preview_layout(
    revealer: &gtk::Revealer,
    wide: bool,
    list_mode: bool,
    saved_width: i32,
) {
    crate::hit!("set_wide_preview_layout");
    use gtk::prelude::*;

    let row = revealer.parent();
    let row_width = row.as_ref().map_or(0, |r| r.width());

    // The window can never be wider than the monitor, so half of it caps the fixed panel.
    let max_window = match monitor_width(revealer) {
        0 => row_width,
        w => w,
    };
    // A row that already overshot the monitor must not feed its own width back into the
    // split, otherwise every list/grid toggle would grow the window further.
    let total = if max_window > 0 {
        row_width.min(max_window)
    } else {
        row_width
    };
    let max_panel = (max_window / 2).max(WIDE_PREVIEW_PANEL_MIN_WIDTH);

    // Measure the file column's real, CSS-aware minimum width with our own request cleared,
    // so list rows that cannot shrink are accounted for before the panel gets its share.
    let mut files_measured_min = 0;
    let mut files_measured_nat = 0;
    let mut sibling = row.as_ref().and_then(|row| row.first_child());
    while let Some(widget) = sibling {
        if widget.is::<adw::ToolbarView>() {
            widget.set_width_request(-1);
            if widget.is_visible() {
                let (min, nat, _, _) = widget.measure(gtk::Orientation::Horizontal, -1);
                files_measured_min = files_measured_min.max(min);
                files_measured_nat = files_measured_nat.max(nat);
            }
        }
        sibling = widget.next_sibling();
    }

    let list_wide = wide && list_mode;
    let files_min_width = if list_wide && total > 0 {
        (total / 3)
            .max(WIDE_PREVIEW_LIST_MIN_WIDTH)
            .max(files_measured_min)
            .min((total - WIDE_PREVIEW_PANEL_MIN_WIDTH).max(0))
    } else {
        -1
    };

    revealer.set_hexpand(wide);
    if let Some(wrapper) = revealer.child() {
        wrapper.set_hexpand(wide);
        wrapper.set_halign(if wide {
            gtk::Align::Fill
        } else {
            gtk::Align::End
        });
        // The wrapper holds `gtk::Overlay { adw::Clamp { panel } }` (see `panels/resize.rs`),
        // so the clamp is the overlay's child, not the wrapper's last child.
        let clamp = wrapper
            .last_child()
            .and_then(|w| match w.downcast::<gtk::Overlay>() {
                Ok(overlay) => overlay.child(),
                Err(w) => Some(w),
            })
            .and_downcast::<adw::Clamp>();
        if let Some(clamp) = clamp {
            // Only list mode bounds the panel: its natural width (folder grid, text, table)
            // can otherwise exceed the screen and drag the window with it. The cap is the
            // monitor width minus the list's width. Grid mode keeps the panel unbounded,
            // so it fills the space the grid leaves instead of being centred.
            let cap = if list_wide && max_window > 0 {
                let left = files_min_width
                    .max(files_measured_min)
                    .max(files_measured_nat);
                (max_window - left).max(WIDE_PREVIEW_PANEL_MIN_WIDTH)
            } else {
                i32::MAX
            };
            clamp.set_maximum_size(cap);
            clamp.set_tightening_threshold(cap);
            clamp.set_hexpand(wide);
            if let Some(panel) = clamp.child() {
                panel.set_hexpand(wide);
                let width = if list_wide {
                    WIDE_PREVIEW_PANEL_MIN_WIDTH
                } else if max_window > 0 {
                    saved_width.clamp(WIDE_PREVIEW_PANEL_MIN_WIDTH, max_panel)
                } else {
                    saved_width
                };
                panel.set_width_request(width);
            }
        }
    }

    let mut sibling = row.and_then(|row| row.first_child());
    while let Some(widget) = sibling {
        if widget.is::<adw::ToolbarView>() {
            widget.set_hexpand(!wide);
            widget.set_width_request(files_min_width);
        }
        sibling = widget.next_sibling();
    }
}
