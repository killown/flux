use super::{MenuEditor, Msg, Shared};
use crate::i18n::tr;
use crate::model::MenuEntry;
use adw::prelude::*;
use gtk::glib;
use relm4::prelude::*;

pub(super) fn rebuild_list(shared: &Shared) {
    let list_box = &shared.list_box;
    while let Some(child) = list_box.first_child() {
        list_box.remove(&child);
    }
    let entries = shared.entries.borrow();
    let query = shared.search_query.borrow();

    let needle = query.trim().to_lowercase();
    let visible: Vec<(usize, &MenuEntry)> = entries
        .iter()
        .enumerate()
        .filter(|(_, e)| {
            if needle.is_empty() {
                return true;
            }
            let label_lc = e.label.to_lowercase();
            let sub_lc = e.submenu.as_deref().unwrap_or("").to_lowercase();
            let mime_lc = e.mime_types.to_lowercase();
            let cmd_lc = e.command.to_lowercase();
            label_lc.contains(&needle)
                || sub_lc.contains(&needle)
                || mime_lc.contains(&needle)
                || cmd_lc.contains(&needle)
        })
        .collect();

    if entries.is_empty() {
        list_box.append(
            &adw::ActionRow::builder()
                .title(tr("No entries yet").as_str())
                .subtitle(tr("Press + or Ctrl+N to add your first menu action").as_str())
                .build(),
        );
        return;
    }

    if visible.is_empty() {
        list_box.append(
            &adw::ActionRow::builder()
                .title(tr("No results").as_str())
                .subtitle(tr("Try a different search term").as_str())
                .build(),
        );
        return;
    }

    let total = entries.len();
    for (idx, entry) in &visible {
        list_box.append(&build_row(*idx, entry, total, &shared.sender));
    }
}

// ─── Row builder ─────────────────────────────────────────────────────────────
fn build_row(
    idx: usize,
    entry: &MenuEntry,
    total: usize,
    sender: &ComponentSender<MenuEditor>,
) -> adw::ActionRow {
    let raw_title = match &entry.submenu {
        Some(sub) => format!("{} › {}", sub, entry.label),
        None => entry.label.clone(),
    };

    let mut raw_subtitle = format!("{} │ {}", entry.mime_types, entry.command);
    if entry.no_command_dialog {
        raw_subtitle.push_str(" │ [no transfer dialog]");
    }

    let safe_title = glib::markup_escape_text(&raw_title);
    let safe_subtitle = glib::markup_escape_text(&raw_subtitle);

    let row = adw::ActionRow::builder()
        .title(safe_title.as_str())
        .subtitle(safe_subtitle.as_str())
        .build();

    // ── Dedicated Prefix Column Box (Line Numbers & Submenu Status) ───────────
    let prefix_grid = gtk::Grid::builder()
        .column_spacing(10)
        .valign(gtk::Align::Center)
        .margin_end(8)
        .build();

    // Column 0: Line Number (fixed width, right-aligned)
    let line_label = gtk::Label::builder()
        .label(format!("L{:02}", idx + 1))
        .css_classes(["caption", "dim-label", "numeric"])
        .halign(gtk::Align::End)
        .width_request(32)
        .build();
    prefix_grid.attach(&line_label, 0, 0, 1, 1);

    // Column 1: Submenu Column (fixed width, left-aligned)
    let sub_badge = if entry.submenu.is_some() {
        gtk::Label::builder()
            .label("sub")
            .css_classes(["caption", "accent"])
            .halign(gtk::Align::Start)
            .width_request(28)
            .build()
    } else {
        gtk::Label::builder().width_request(28).build()
    };
    prefix_grid.attach(&sub_badge, 1, 0, 1, 1);

    row.add_prefix(&prefix_grid);

    let btn_row = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(2)
        .valign(gtk::Align::Center)
        .build();

    let mk = |icon: &str, tip: &str| {
        gtk::Button::builder()
            .icon_name(icon)
            .tooltip_text(tip)
            .css_classes(["flat"])
            .build()
    };
    let up = mk("go-up-symbolic", tr("Move up").as_str());
    let down = mk("go-down-symbolic", tr("Move down").as_str());
    let edit = mk("document-edit-symbolic", tr("Edit").as_str());
    let del = mk("user-trash-symbolic", tr("Delete").as_str());

    up.set_sensitive(idx > 0);
    down.set_sensitive(idx + 1 < total);
    del.add_css_class("destructive-action");

    for w in [&up, &down, &edit, &del] {
        btn_row.append(w);
    }
    row.add_suffix(&btn_row);

    macro_rules! wire {
        ($btn:expr, $msg:expr) => {{
            let s = sender.clone();
            $btn.connect_clicked(move |_| s.input($msg));
        }};
    }
    wire!(up, Msg::MoveUp(idx));
    wire!(down, Msg::MoveDown(idx));
    wire!(edit, Msg::EditEntry(idx));
    wire!(del, Msg::DeleteEntry(idx));

    row
}
