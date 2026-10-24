use crate::i18n::tr;
use crate::ui::help::format::format_shortcut;
use crate::ui::keymap::DISABLED;
use adw::gdk;
use adw::prelude::*;
use gtk::glib;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

/// Click-to-capture keyboard shortcut input, plus a toggle that disables the
/// shortcut entirely.
///
/// Renders the current accelerator as a keycap label. Clicking the keycap
/// enters capture mode: the next key combination is turned into a GTK
/// accelerator string, validated with [`gtk::ShortcutTrigger::parse_string`]
/// and reported through the `on_changed` callback. Clicking the adjacent
/// toggle instead writes the [`DISABLED`] sentinel (`on_changed(Some("disabled"))`),
/// or clears the override back to the compiled-in default (`on_changed(None)`)
/// when the shortcut is already disabled.
pub struct ShortcutPicker;

impl ShortcutPicker {
    /// Builds the picker row: the capture button followed by the disable
    /// toggle.
    ///
    /// While capturing, `Escape` cancels, `BackSpace`/`Delete` clear the
    /// shortcut (`on_changed(None)`) and any other non-modifier key is
    /// accepted if the resulting accelerator parses. Losing focus also
    /// cancels capture. Outside capture mode the button never consumes key
    /// events. A value of [`DISABLED`] renders as a plain `Disabled` keycap
    /// and the toggle shows as pressed and labelled `Restore`.
    ///
    /// # Arguments
    ///
    /// * `accel` - The currently configured accelerator, if any. `Some(DISABLED)`
    ///   means the shortcut is disabled.
    /// * `default_accel` - The compiled-in fallback, used for the keycap
    ///   label and tooltip.
    /// * `on_changed` - Invoked with the new accelerator, `Some(DISABLED)`
    ///   to disable, or `None` to fall back to the default.
    ///
    /// # Returns
    ///
    /// A horizontal [`gtk::Box`] holding both buttons, ready to be used as a
    /// row suffix.
    #[allow(clippy::new_ret_no_self)]
    pub fn new(
        accel: Option<String>,
        default_accel: &str,
        on_changed: impl Fn(Option<String>) + 'static,
    ) -> gtk::Box {
        let current = Rc::new(RefCell::new(accel.filter(|a| !a.trim().is_empty())));
        let capturing = Rc::new(Cell::new(false));
        let default_accel = default_accel.to_string();

        // Both the toggle and the key controller need `on_changed`. It is
        // `Fn`, not `Clone`, so wrap it once and clone the `Rc` into each
        // closure instead of moving the value twice.
        let on_changed: Rc<dyn Fn(Option<String>)> = Rc::new(on_changed);

        let button = gtk::Button::builder()
            .valign(gtk::Align::Center)
            .css_classes(["keycap"])
            .focusable(true)
            .build();

        // The toggle shows its own state as text: `Disable` when the
        // shortcut is active, `Restore` when it is disabled. This does not
        // depend on any icon-theme name and survives every locale, which
        // `action-unavailable-symbolic` did not.
        let disable_button = gtk::ToggleButton::builder()
            .label(tr("Disable"))
            .tooltip_text(tr("Disable this shortcut"))
            .valign(gtk::Align::Center)
            .css_classes(["flat"])
            .build();

        // Keep the toggle's visual state in sync with the stored value.
        let is_disabled = {
            let current = current.clone();
            Rc::new(move || current.borrow().as_deref().map(str::trim) == Some(DISABLED))
        };
        let refresh: Rc<dyn Fn()> = {
            let button = button.clone();
            let disable_button = disable_button.clone();
            let current = current.clone();
            let capturing = capturing.clone();
            let default_accel = default_accel.clone();
            let is_disabled = is_disabled.clone();
            Rc::new(move || {
                let disabled = is_disabled();
                disable_button.set_active(disabled);
                if disabled {
                    disable_button.set_label(&tr("Restore"));
                    disable_button.set_tooltip_text(Some(&tr("Restore the default shortcut")));
                } else {
                    disable_button.set_label(&tr("Disable"));
                    disable_button.set_tooltip_text(Some(&tr("Disable this shortcut")));
                }

                if capturing.get() {
                    button.set_label(&tr("Press a key…"));
                    button.set_tooltip_text(Some(&tr("Press Esc to cancel, or Delete to clear")));
                    return;
                }

                if disabled {
                    button.set_label(&tr("Disabled"));
                    button.set_tooltip_text(Some(&format!(
                        "{}\n{}: {}",
                        tr("Click the toggle to restore the default"),
                        tr("Default"),
                        format_shortcut(Some(default_accel.clone()), ""),
                    )));
                    return;
                }

                match current.borrow().as_deref() {
                    Some(accel) => {
                        let label = accel_to_label(accel);
                        button.set_label(&label);
                        button.set_tooltip_text(Some(&format!(
                            "{}\n{}: {}",
                            tr("Click to change"),
                            tr("Default"),
                            accel_to_label(&default_accel),
                        )));
                    }
                    None => {
                        let default_label = accel_to_label(&default_accel);
                        button.set_label(&default_label);
                        button.set_tooltip_text(Some(&format!(
                            "{}\n{}: {}",
                            tr("Click to change"),
                            tr("Default"),
                            default_label,
                        )));
                    }
                }
            })
        };
        refresh();

        {
            let capturing = capturing.clone();
            let refresh = refresh.clone();
            button.connect_clicked(move |btn| {
                if capturing.get() {
                    capturing.set(false);
                } else {
                    capturing.set(true);
                    btn.grab_focus();
                }
                refresh();
            });
        }

        {
            let current = current.clone();
            let capturing = capturing.clone();
            let refresh = refresh.clone();
            let is_disabled = is_disabled.clone();
            let on_changed = on_changed.clone();
            disable_button.connect_toggled(move |btn| {
                // Ignore the notify that `refresh()` itself triggers when it
                // sets the toggle's state from the stored value.
                if btn.is_active() == is_disabled() {
                    return;
                }
                if btn.is_active() {
                    *current.borrow_mut() = Some(DISABLED.to_string());
                    capturing.set(false);
                    on_changed(Some(DISABLED.to_string()));
                } else {
                    *current.borrow_mut() = None;
                    on_changed(None);
                }
                refresh();
            });
        }

        let key_ctrl = gtk::EventControllerKey::new();
        key_ctrl.set_propagation_phase(gtk::PropagationPhase::Capture);
        {
            let capturing = capturing.clone();
            let current = current.clone();
            let refresh = refresh.clone();
            let on_changed = on_changed.clone();
            key_ctrl.connect_key_pressed(move |_, keyval, _keycode, state| {
                if !capturing.get() {
                    return glib::Propagation::Proceed;
                }

                let mods = state & gtk::accelerator_get_default_mod_mask();

                if is_modifier_key(keyval) {
                    return glib::Propagation::Stop;
                }

                if keyval == gdk::Key::Escape && mods.is_empty() {
                    capturing.set(false);
                    refresh();
                    return glib::Propagation::Stop;
                }

                if mods.is_empty() && matches!(keyval, gdk::Key::BackSpace | gdk::Key::Delete) {
                    *current.borrow_mut() = None;
                    capturing.set(false);
                    refresh();
                    on_changed(None);
                    return glib::Propagation::Stop;
                }

                if let Some(accel) = keyval_to_accel(keyval, mods) {
                    *current.borrow_mut() = Some(accel.clone());
                    on_changed(Some(accel));
                }
                capturing.set(false);
                refresh();
                glib::Propagation::Stop
            });
        }
        button.add_controller(key_ctrl);

        let focus_ctrl = gtk::EventControllerFocus::new();
        {
            let capturing = capturing.clone();
            let refresh = refresh.clone();
            focus_ctrl.connect_leave(move |_| {
                if capturing.replace(false) {
                    refresh();
                }
            });
        }
        button.add_controller(focus_ctrl);

        let row = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(4)
            .valign(gtk::Align::Center)
            .build();
        row.append(&button);
        row.append(&disable_button);
        row
    }
}

/// Formats an accelerator string as a keycap label, e.g. `<Primary><Shift>f`
/// becomes `Ctrl + Shift + F`.
fn accel_to_label(accel: &str) -> String {
    format_shortcut(Some(accel.to_string()), "")
}

/// Returns `true` for keys that are modifiers on their own and therefore
/// cannot form a shortcut.
fn is_modifier_key(keyval: gdk::Key) -> bool {
    matches!(
        keyval,
        gdk::Key::Shift_L
            | gdk::Key::Shift_R
            | gdk::Key::Control_L
            | gdk::Key::Control_R
            | gdk::Key::Alt_L
            | gdk::Key::Alt_R
            | gdk::Key::Super_L
            | gdk::Key::Super_R
            | gdk::Key::Meta_L
            | gdk::Key::Meta_R
            | gdk::Key::Hyper_L
            | gdk::Key::Hyper_R
            | gdk::Key::ISO_Level3_Shift
            | gdk::Key::Caps_Lock
            | gdk::Key::Num_Lock
    )
}

/// Builds a GTK accelerator string such as `<Primary><Shift>f`.
fn keyval_to_accel(keyval: gdk::Key, mods: gdk::ModifierType) -> Option<String> {
    if is_modifier_key(keyval) || (keyval == gdk::Key::Escape && mods.is_empty()) {
        return None;
    }

    let mut accel = String::new();
    for (mask, prefix) in [
        (gdk::ModifierType::CONTROL_MASK, "<Primary>"),
        (gdk::ModifierType::SHIFT_MASK, "<Shift>"),
        (gdk::ModifierType::ALT_MASK, "<Alt>"),
        (gdk::ModifierType::SUPER_MASK, "<Super>"),
    ] {
        if mods.contains(mask) {
            accel.push_str(prefix);
        }
    }

    accel.push_str(keyval.to_lower().name()?.as_str());

    gtk::ShortcutTrigger::parse_string(&accel).map(|_| accel)
}
