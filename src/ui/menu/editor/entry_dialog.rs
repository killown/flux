use super::builtin_dialog::show_builtin_dialog;
use super::{Msg, Shared};
use crate::i18n::tr;
use crate::model::MenuEntry;
use adw::prelude::*;
use gtk::glib;
use relm4::prelude::*;

pub(super) fn show_dialog(shared: &Shared, replace: Option<usize>, entry: &MenuEntry) {
    let dialog = adw::Window::new();
    let dialog_title = if replace.is_some() {
        tr("Edit Entry")
    } else {
        tr("Add Entry")
    };
    dialog.set_title(Some(dialog_title.as_str()));
    dialog.set_modal(true);
    dialog.set_transient_for(Some(&shared.root));
    dialog.set_default_size(680, -1);
    dialog.set_resizable(false);

    let outer = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .build();
    outer.append(&adw::HeaderBar::new());

    let scroller = gtk::ScrolledWindow::builder()
        .vexpand(true)
        .propagate_natural_height(true)
        .build();

    let page = adw::PreferencesPage::new();
    let g_id = adw::PreferencesGroup::builder()
        .title(tr("Identity").as_str())
        .build();
    let g_act = adw::PreferencesGroup::builder()
        .title(tr("Action").as_str())
        .build();

    // ── Stacked Entry Row helper ──────────────────────────────────────────────
    let make_stacked_entry_row = |title: &str, value: &str| -> (adw::PreferencesRow, gtk::Entry) {
        let entry = gtk::Entry::builder()
            .text(value)
            .hexpand(true)
            .margin_top(4)
            .margin_bottom(8)
            .margin_start(12)
            .margin_end(12)
            .build();

        let vbox = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .margin_top(8)
            .margin_bottom(4)
            .build();

        let label = gtk::Label::builder()
            .label(title)
            .halign(gtk::Align::Start)
            .margin_start(12)
            .css_classes(["heading"])
            .build();

        vbox.append(&label);
        vbox.append(&entry);

        let pref_row = adw::PreferencesRow::builder().build();
        pref_row.set_child(Some(&vbox));

        (pref_row, entry)
    };

    let total_entries = shared.entries.borrow().len();
    let max_line = if replace.is_some() {
        total_entries.max(1)
    } else {
        total_entries + 1
    };

    let initial_line = replace.map(|idx| idx + 1).unwrap_or(max_line);

    // ── Line Position Spinner Row ─────────────────────────────────────────────
    let menu_name_val = shared.current_menu.borrow().clone();
    let spin_btn = gtk::SpinButton::builder()
        .adjustment(&gtk::Adjustment::new(
            initial_line as f64,
            1.0,
            max_line as f64,
            1.0,
            5.0,
            0.0,
        ))
        .numeric(true)
        .valign(gtk::Align::Center)
        .margin_end(12)
        .build();

    let line_row_title = format!("Line Position in {}", menu_name_val);
    let line_row = adw::ActionRow::builder()
        .title(tr(&line_row_title))
        .subtitle(tr("Set exact line order (1-based)").as_str())
        .activatable_widget(&spin_btn)
        .build();
    line_row.add_suffix(&spin_btn);

    let label_entry = gtk::Entry::builder()
        .text(&entry.label)
        .hexpand(true)
        .build();

    let icon_button = gtk::Button::builder()
        .label(tr("Pick Icon").as_str())
        .valign(gtk::Align::Center)
        .build();

    let label_entry_clone = label_entry.clone();
    let window_weak = dialog.downgrade();

    icon_button.connect_clicked(move |_| {
        let entry_ref = label_entry_clone.clone();
        let parent_win = window_weak.upgrade();

        crate::ui::dialog::menu_icon_picker::show_menu_icon_picker(
            parent_win.as_ref().map(|w| w.upcast_ref()),
            move |glyph| {
                let current = entry_ref.text().to_string();
                let cleaned = current
                    .chars()
                    .skip_while(|c| !c.is_alphanumeric() && *c != '(')
                    .collect::<String>();

                entry_ref.set_text(&format!(
                    "{}{}{}",
                    glyph,
                    crate::ui::dialog::menu_icon_picker::MENU_ICON_PADDING,
                    cleaned
                ));
            },
        );
    });

    let label_input_box = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(8)
        .margin_top(4)
        .margin_bottom(8)
        .margin_start(12)
        .margin_end(12)
        .build();
    label_input_box.append(&label_entry);
    label_input_box.append(&icon_button);

    let label_vbox = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .margin_top(8)
        .margin_bottom(4)
        .build();

    let label_header = gtk::Label::builder()
        .label(tr("Label").as_str())
        .halign(gtk::Align::Start)
        .margin_start(12)
        .css_classes(["heading"])
        .build();

    label_vbox.append(&label_header);
    label_vbox.append(&label_input_box);

    let label_row = adw::PreferencesRow::builder().build();
    label_row.set_child(Some(&label_vbox));

    let sub_entry = gtk::Entry::builder()
        .text(entry.submenu.as_deref().unwrap_or(""))
        .hexpand(true)
        .build();

    let sub_icon_button = gtk::Button::builder()
        .label(tr("Pick Icon").as_str())
        .valign(gtk::Align::Center)
        .build();

    let sub_entry_clone = sub_entry.clone();
    let sub_window_weak = dialog.downgrade();

    sub_icon_button.connect_clicked(move |_| {
        let entry_ref = sub_entry_clone.clone();
        let parent_win = sub_window_weak.upgrade();

        crate::ui::dialog::menu_icon_picker::show_menu_icon_picker(
            parent_win.as_ref().map(|w| w.upcast_ref()),
            move |glyph| {
                let current = entry_ref.text().to_string();
                let cleaned = current
                    .chars()
                    .skip_while(|c| !c.is_alphanumeric() && *c != '(')
                    .collect::<String>();

                entry_ref.set_text(&format!(
                    "{}{}{}",
                    glyph,
                    crate::ui::dialog::menu_icon_picker::MENU_ICON_PADDING,
                    cleaned
                ));
            },
        );
    });

    let sub_input_box = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(8)
        .margin_top(4)
        .margin_bottom(8)
        .margin_start(12)
        .margin_end(12)
        .build();
    sub_input_box.append(&sub_entry);
    sub_input_box.append(&sub_icon_button);

    let sub_vbox = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .margin_top(8)
        .margin_bottom(4)
        .build();

    let sub_header = gtk::Label::builder()
        .label(tr("Submenu (blank = top-level)").as_str())
        .halign(gtk::Align::Start)
        .margin_start(12)
        .css_classes(["heading"])
        .build();

    sub_vbox.append(&sub_header);
    sub_vbox.append(&sub_input_box);

    let sub_row = adw::PreferencesRow::builder().build();
    sub_row.set_child(Some(&sub_vbox));

    let (mime_row, mime_entry) =
        make_stacked_entry_row(tr("MIME Types").as_str(), &entry.mime_types);
    let mime_hint = adw::ActionRow::builder()
        .title("all │ file │ directory │ trash │ image/all │ video/all │ audio/ │ text/all, application/all")
        .css_classes(["property"])
        .build();

    // ── Command Row with Pick Built-in Button ─────────────────────────────────
    let cmd_entry = gtk::Entry::builder()
        .text(&entry.command)
        .hexpand(true)
        .build();

    let builtin_btn = gtk::Button::builder()
        .label(tr("Pick Built-in").as_str())
        .valign(gtk::Align::Center)
        .build();

    let cmd_entry_clone = cmd_entry.clone();
    let mime_entry_clone = mime_entry.clone();
    let cmd_window_weak = dialog.downgrade();

    builtin_btn.connect_clicked(move |_| {
        let cmd_target = cmd_entry_clone.clone();
        let mime_target = mime_entry_clone.clone();
        let parent_win = cmd_window_weak.upgrade();

        show_builtin_dialog(
            parent_win.as_ref().map(|w| w.upcast_ref()),
            move |name, scope| {
                cmd_target.set_text(name);
                if mime_target.text().trim().is_empty() {
                    mime_target.set_text(scope);
                }
            },
        );
    });

    let cmd_input_box = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(8)
        .margin_top(4)
        .margin_bottom(8)
        .margin_start(12)
        .margin_end(12)
        .build();
    cmd_input_box.append(&cmd_entry);
    cmd_input_box.append(&builtin_btn);

    let cmd_vbox = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .margin_top(8)
        .margin_bottom(4)
        .build();

    let cmd_header = gtk::Label::builder()
        .label(tr("Command (%p = path · %d = dir · %f = filename · %l = line)").as_str())
        .halign(gtk::Align::Start)
        .margin_start(12)
        .css_classes(["heading"])
        .build();

    cmd_vbox.append(&cmd_header);
    cmd_vbox.append(&cmd_input_box);

    let cmd_row = adw::PreferencesRow::builder().build();
    cmd_row.set_child(Some(&cmd_vbox));

    let (toast_row, toast_entry) = make_stacked_entry_row(
        tr("Notification (optional)").as_str(),
        entry.toast.as_deref().unwrap_or(""),
    );

    // ── Checkbox / Switch for no_command_dialog ──────────────────────────────
    let no_transfer_switch = gtk::Switch::builder()
        .active(entry.no_command_dialog)
        .valign(gtk::Align::Center)
        .margin_end(12)
        .build();

    let no_transfer_row = adw::ActionRow::builder()
        .title(tr("Suppress Transfer Dialog").as_str())
        .subtitle(tr("Do not track execution progress or open transfer dialog").as_str())
        .activatable_widget(&no_transfer_switch)
        .build();
    no_transfer_row.add_suffix(&no_transfer_switch);

    g_id.add(&line_row);
    g_id.add(&label_row);
    g_id.add(&sub_row);
    g_act.add(&mime_row);
    g_act.add(&mime_hint);
    g_act.add(&cmd_row);
    g_act.add(&toast_row);
    g_act.add(&no_transfer_row);
    page.add(&g_id);
    page.add(&g_act);
    scroller.set_child(Some(&page));
    outer.append(&scroller);

    // ── Buttons ───────────────────────────────────────────────────────────────
    let btn_bar = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .halign(gtk::Align::End)
        .spacing(8)
        .margin_top(12)
        .margin_bottom(20)
        .margin_end(20)
        .build();
    let cancel_btn = gtk::Button::builder().label(tr("Cancel").as_str()).build();
    let commit_label = if replace.is_some() {
        tr("Save")
    } else {
        tr("Add")
    };
    let commit_btn = gtk::Button::builder()
        .label(commit_label.as_str())
        .css_classes(["suggested-action"])
        .build();
    btn_bar.append(&cancel_btn);
    btn_bar.append(&commit_btn);
    outer.append(&btn_bar);

    dialog.set_content(Some(&outer));

    // ── Cancel ────────────────────────────────────────────────────────────────
    {
        let d = dialog.clone();
        cancel_btn.connect_clicked(move |_| d.close());
    }

    // ── Escape ────────────────────────────────────────────────────────────────
    {
        let d = dialog.clone();
        let esc = gtk::ShortcutController::new();
        esc.add_shortcut(gtk::Shortcut::new(
            gtk::ShortcutTrigger::parse_string("Escape"),
            Some(gtk::CallbackAction::new(move |_, _| {
                d.close();
                glib::Propagation::Stop
            })),
        ));
        dialog.add_controller(esc);
    }

    // ── Commit ────────────────────────────────────────────────────────────────
    {
        let d = dialog.clone();
        let sender = shared.sender.clone();
        commit_btn.connect_clicked(move |_| {
            let label = label_entry.text().to_string();
            if label.trim().is_empty() {
                label_entry.add_css_class("error");
                return;
            }
            label_entry.remove_css_class("error");

            let target_line = spin_btn.value_as_int() as usize;

            let new_entry = MenuEntry {
                label,
                submenu: {
                    let v = sub_entry.text().to_string();
                    if v.trim().is_empty() {
                        None
                    } else {
                        Some(v)
                    }
                },
                mime_types: mime_entry.text().to_string(),
                command: cmd_entry.text().to_string(),
                toast: {
                    let v = toast_entry.text().to_string();
                    if v.trim().is_empty() {
                        None
                    } else {
                        Some(v)
                    }
                },
                no_command_dialog: no_transfer_switch.is_active(),
            };

            sender.input(Msg::Commit {
                entry: new_entry,
                replace,
                target_line,
            });
            d.close();
        });
    }

    dialog.present();
}
