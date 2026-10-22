use crate::i18n::tr;
use crate::ui::help::format::format_shortcut;
use adw::gdk;
use adw::prelude::*;
use gtk::glib;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

/// Click-to-capture keyboard shortcut input.
///
/// Renders the current accelerator as a keycap label. Clicking it enters
/// capture mode: the next key combination is turned into a GTK accelerator
/// string, validated with [`gtk::ShortcutTrigger::parse_string`] and reported
/// through the `on_changed` callback.
pub struct ShortcutPicker;

impl ShortcutPicker {
    /// Builds the picker button.
    ///
    /// While capturing, `Escape` cancels, `BackSpace`/`Delete` clear the
    /// shortcut (`on_changed(None)`) and any other non-modifier key is
    /// accepted if the resulting accelerator parses. Losing focus also
    /// cancels capture. Outside capture mode the button never consumes
    /// key events.
    ///
    /// # Arguments
    ///
    /// * `accel` - The currently configured accelerator, if any.
    /// * `on_changed` - Invoked with the new accelerator, or `None` when cleared.
    ///
    /// # Returns
    ///
    /// The configured [`gtk::Button`], ready to be used as a row suffix.
    // Factory for a ready-made `gtk::Button`, `ShortcutPicker` itself is only a
    // namespace, so returning something other than `Self` is intentional.
    #[allow(clippy::new_ret_no_self)]
    pub fn new(
        accel: Option<String>,
        default_accel: &str,
        on_changed: impl Fn(Option<String>) + 'static,
    ) -> gtk::Button {
        let current = Rc::new(RefCell::new(accel.filter(|a| !a.trim().is_empty())));
        let capturing = Rc::new(Cell::new(false));
        let default_accel = default_accel.to_string();

        let button = gtk::Button::builder()
            .valign(gtk::Align::Center)
            .css_classes(["keycap"])
            .focusable(true)
            .build();

        let refresh = {
            let button = button.clone();
            let current = current.clone();
            let capturing = capturing.clone();
            let default_accel = default_accel.clone();
            Rc::new(move || {
                if capturing.get() {
                    button.set_label(&tr("Press a key…"));
                    button.set_tooltip_text(Some(&tr("Press Esc to cancel, or Delete to clear")));
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
                        // No user override: show the actual default keycap so the
                        // user immediately sees what key is active.
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

        let key_ctrl = gtk::EventControllerKey::new();
        key_ctrl.set_propagation_phase(gtk::PropagationPhase::Capture);
        {
            let capturing = capturing.clone();
            let current = current.clone();
            let refresh = refresh.clone();
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
            focus_ctrl.connect_leave(move |_| {
                if capturing.replace(false) {
                    refresh();
                }
            });
        }
        button.add_controller(focus_ctrl);

        button
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
///
/// # Arguments
///
/// * `keyval` - The pressed key.
/// * `mods` - Modifier state, already masked with
///   [`gtk::accelerator_get_default_mod_mask`].
///
/// # Returns
///
/// `None` for modifier-only presses, bare `Escape`, keys without a GDK name,
/// or strings GTK cannot parse back into a [`gtk::ShortcutTrigger`].
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

    // Shift reports the upper-case keyval, the `<Shift>` prefix already
    // carries that information.
    accel.push_str(keyval.to_lower().name()?.as_str());

    gtk::ShortcutTrigger::parse_string(&accel).map(|_| accel)
}
