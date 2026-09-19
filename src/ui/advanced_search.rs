use crate::i18n::tr;
use crate::model::{AppMsg, FluxApp};
use crate::services::extension_search::AdvancedSearchParams;
use adw::prelude::*;
use relm4::AsyncComponentSender;

fn apply_flat_filters(
    sender: &AsyncComponentSender<FluxApp>,
    date_seconds: Option<u64>,
    size_bytes: Option<(bool, u64)>,
    tag_text: String,
) {
    let mut parts: Vec<String> = Vec::new();

    if let Some(secs) = date_seconds {
        let boundary = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0)
            .saturating_sub(secs);
        parts.push(format!(">date:{}", boundary));
    }

    if let Some((larger, bytes)) = size_bytes {
        let op = if larger { ">" } else { "<" };
        parts.push(format!("size:{}{}", op, bytes));
    }

    if !tag_text.is_empty() {
        for tag in tag_text.split(',').map(str::trim).filter(|t| !t.is_empty()) {
            parts.push(format!(":tag:{}", tag.trim_start_matches('#')));
        }
    }

    if !parts.is_empty() {
        sender.input(AppMsg::UpdateFilter(parts.join(" ")));
    }
}

/// Builds and returns the lazy-initialized right search sidebar panel widget tree.
pub fn build_search_panel(sender: AsyncComponentSender<FluxApp>) -> gtk::Box {
    let panel = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(0)
        .build();

    panel.set_width_request(300);
    panel.add_css_class("sidebar");

    // ── Header: [search icon] "Search" ... [Search Button] [X close button] ──
    let header_box = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(8)
        .margin_start(12)
        .margin_end(12)
        .margin_top(8)
        .margin_bottom(8)
        .build();

    let search_icon = gtk::Image::from_icon_name("system-search-symbolic");

    let title_label = gtk::Label::builder()
        .label(tr("Search"))
        .css_classes(["heading"])
        .hexpand(true)
        .xalign(0.0)
        .build();

    let search_btn = gtk::Button::builder()
        .label(tr("Search"))
        .css_classes(["suggested-action", "pill"])
        .valign(gtk::Align::Center)
        .build();

    let close_btn = gtk::Button::builder()
        .icon_name("window-close-symbolic")
        .css_classes(["flat", "circular"])
        .valign(gtk::Align::Center)
        .tooltip_text(tr("Cancel"))
        .build();

    {
        let s = sender.clone();
        close_btn.connect_clicked(move |_| {
            s.input(AppMsg::ToggleSearchPanel);
        });
    }

    header_box.append(&search_icon);
    header_box.append(&title_label);
    header_box.append(&search_btn);
    header_box.append(&close_btn);
    panel.append(&header_box);

    // ── Escape Key Handling ──────────────────────────────────────────────────
    let esc_controller = gtk::EventControllerKey::new();
    {
        let s = sender.clone();
        esc_controller.connect_key_pressed(move |_, keyval, _, _| {
            if keyval == adw::gdk::Key::Escape {
                s.input(AppMsg::ToggleSearchPanel);
                return gtk::glib::Propagation::Stop;
            }
            gtk::glib::Propagation::Proceed
        });
    }
    panel.add_controller(esc_controller);

    // ── Panel Body ───────────────────────────────────────────────────────────
    let content_box = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(12)
        .margin_start(12)
        .margin_end(12)
        .margin_top(4)
        .margin_bottom(12)
        .build();

    let what_group = adw::PreferencesGroup::builder()
        .title(tr("What to find"))
        .build();

    let make_entry_row =
        |group: &adw::PreferencesGroup, title: &str, placeholder: &str| -> gtk::Entry {
            let entry = gtk::Entry::builder()
                .placeholder_text(placeholder)
                .valign(gtk::Align::Center)
                .build();
            let row = adw::ActionRow::builder()
                .title(title)
                .activatable(true)
                .build();
            row.add_suffix(&entry);
            row.set_activatable_widget(Some(&entry));
            group.add(&row);
            entry
        };

    let make_switch_row =
        |group: &adw::PreferencesGroup, title: &str, subtitle: &str, active: bool| -> gtk::Switch {
            let sw = gtk::Switch::builder()
                .active(active)
                .valign(gtk::Align::Center)
                .build();
            let row = adw::ActionRow::builder()
                .title(title)
                .subtitle(subtitle)
                .activatable_widget(&sw)
                .build();
            row.add_suffix(&sw);
            group.add(&row);
            sw
        };

    let make_combo_row = |title: &str, items: &[&str], selected: u32| -> adw::ComboRow {
        let model = gtk::StringList::new(items);
        adw::ComboRow::builder()
            .title(title)
            .model(&model)
            .selected(selected)
            .build()
    };

    let name_entry = make_entry_row(&what_group, &tr("File name"), "invoice, draft*, photo");
    let exact_match_sw = make_switch_row(
        &what_group,
        &tr("Exact match"),
        &tr("Match exact filename without wildcards"),
        false,
    );
    let content_entry = make_entry_row(
        &what_group,
        &tr("Inside files"),
        &tr("Requires 3+ characters"),
    );
    let fname_entry = make_entry_row(&what_group, &tr("Glob pattern"), "*.rs, image/*, *.pdf");
    let ext_entry = make_entry_row(&what_group, &tr("Extension"), "rs, py, txt");
    let tag_entry = make_entry_row(&what_group, &tr("Tag"), "#work, #project");

    content_box.append(&what_group);

    let scope_group = adw::PreferencesGroup::builder()
        .title(tr("Where to look"))
        .build();

    let recursive_sw = make_switch_row(&scope_group, &tr("Search inside subfolders"), "", true);
    let hidden_sw = make_switch_row(
        &scope_group,
        &tr("Include hidden files"),
        &tr("Files beginning with a dot"),
        false,
    );

    content_box.append(&scope_group);

    // ── Additional Filters (Date & Size) ─────────────────────────────────────
    let filters_expander = adw::ExpanderRow::builder()
        .title(tr("Narrow results"))
        .subtitle(tr("Filter by date and file size"))
        .build();

    let date_row = make_combo_row(
        &tr("Modified"),
        &[
            &tr("Any time"),
            &tr("Last hour"),
            &tr("Today"),
            &tr("Last 7 days"),
            &tr("Last 30 days"),
            &tr("Last year"),
        ],
        0,
    );

    let size_op_row = make_combo_row(
        &tr("File size"),
        &[&tr("Any size"), &tr("Larger than"), &tr("Smaller than")],
        0,
    );

    let size_box = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(6)
        .hexpand(true)
        .valign(gtk::Align::Center)
        .build();

    let size_entry = gtk::Entry::builder()
        .placeholder_text("0")
        .input_purpose(gtk::InputPurpose::Digits)
        .width_chars(6)
        .max_width_chars(8)
        .sensitive(false)
        .build();

    let size_unit_store = gtk::StringList::new(&["KB", "MB", "GB"]);
    let size_unit_combo = gtk::DropDown::builder()
        .model(&size_unit_store)
        .selected(1)
        .sensitive(false)
        .valign(gtk::Align::Center)
        .build();

    size_box.append(&size_entry);
    size_box.append(&size_unit_combo);

    let size_amount_row = adw::ActionRow::builder()
        .title(tr("Amount"))
        .sensitive(false)
        .build();
    size_amount_row.add_suffix(&size_box);
    size_amount_row.set_activatable_widget(Some(&size_entry));

    {
        let size_entry_c = size_entry.clone();
        let amount_row_c = size_amount_row.clone();
        let unit_combo_c = size_unit_combo.clone();
        size_op_row.connect_selected_notify(move |row| {
            let active = row.selected() != 0;
            size_entry_c.set_sensitive(active);
            amount_row_c.set_sensitive(active);
            unit_combo_c.set_sensitive(active);
        });
    }

    filters_expander.add_row(&date_row);
    filters_expander.add_row(&size_op_row);
    filters_expander.add_row(&size_amount_row);

    let filters_group = adw::PreferencesGroup::builder().build();
    filters_group.add(&filters_expander);
    content_box.append(&filters_group);

    // ── Explicit Search Trigger Execution ────────────────────────────────────
    let execute_search = {
        let s = sender.clone();
        let name_e = name_entry.clone();
        let exact_sw = exact_match_sw.clone();
        let content_e = content_entry.clone();
        let fname_e = fname_entry.clone();
        let ext_e = ext_entry.clone();
        let tag_e = tag_entry.clone();
        let rec_sw = recursive_sw.clone();
        let hid_sw = hidden_sw.clone();
        let date_r = date_row.clone();
        let size_op_r = size_op_row.clone();
        let size_e = size_entry.clone();
        let size_unit_c = size_unit_combo.clone();

        move || {
            let name_text = name_e.text().trim().to_string();
            let fname_text = fname_e.text().trim().to_string();
            let content_text = content_e.text().trim().to_string();
            let ext_text = ext_e.text().trim().to_string();
            let tag_text = tag_e.text().trim().to_string();
            let mut recursive = rec_sw.is_active();
            let include_hidden = hid_sw.is_active();
            let exact_match = exact_sw.is_active();

            let date_sel = date_r.selected();
            let size_op_sel = size_op_r.selected();
            let size_val: u64 = size_e.text().trim().parse().unwrap_or(0);
            let size_unit_sel = size_unit_c.selected();

            let date_seconds: Option<u64> = match date_sel {
                1 => Some(3_600),
                2 => Some(86_400),
                3 => Some(7 * 86_400),
                4 => Some(30 * 86_400),
                5 => Some(365 * 86_400),
                _ => None,
            };

            let size_bytes: Option<(bool, u64)> = if size_op_sel != 0 && size_val > 0 {
                let multiplier: u64 = match size_unit_sel {
                    0 => 1_024,
                    1 => 1_024 * 1_024,
                    _ => 1_024 * 1_024 * 1_024,
                };
                Some((size_op_sel == 1, size_val * multiplier))
            } else {
                None
            };

            // Content search takes priority when the field has ≥3 chars
            if content_text.len() >= 3 {
                let ext_filter = if ext_text.is_empty() {
                    None
                } else {
                    Some(ext_text)
                };
                apply_flat_filters(&s, date_seconds, size_bytes, tag_text);
                s.input(AppMsg::StartContentSearch(content_text, ext_filter));
                return;
            }

            let mut patterns: Vec<String> = Vec::new();

            if !name_text.is_empty() {
                for item in name_text
                    .split(',')
                    .map(str::trim)
                    .filter(|slice| !slice.is_empty())
                {
                    let mut term = item.to_string();
                    if term.contains('*') {
                        recursive = true;
                    }
                    if !exact_match && !term.starts_with('*') && !term.ends_with('*') {
                        term = format!("*{}*", term);
                    }
                    patterns.push(term.to_lowercase());
                }
            }

            if !fname_text.is_empty() {
                for p in fname_text
                    .split(',')
                    .map(|p| p.trim().to_lowercase())
                    .filter(|p| !p.is_empty())
                {
                    patterns.push(p);
                }
            } else if !ext_text.is_empty() {
                for e in ext_text
                    .split(',')
                    .map(|e| e.trim().trim_start_matches('.'))
                    .filter(|e| !e.is_empty())
                {
                    patterns.push(format!("*.{}", e));
                }
            }

            if recursive {
                if patterns.is_empty() {
                    patterns.push("*".to_string());
                }
                s.input(AppMsg::StartAdvancedSearch(AdvancedSearchParams {
                    patterns,
                    date_seconds,
                    size_bytes,
                    include_hidden,
                    max_results: 0,
                }));
                return;
            }

            if !patterns.is_empty() {
                s.input(AppMsg::SetExtensionFilter(patterns));
            } else {
                s.input(AppMsg::ClearExtensionFilter);
            }

            apply_flat_filters(&s, date_seconds, size_bytes, tag_text);
        }
    };

    // ── Wire Search Button & Enter Key Handlers ──────────────────────────────
    {
        let run = execute_search.clone();
        search_btn.connect_clicked(move |_| {
            run();
        });
    }

    for entry in [
        &name_entry,
        &content_entry,
        &fname_entry,
        &ext_entry,
        &tag_entry,
        &size_entry,
    ] {
        let run = execute_search.clone();
        entry.connect_activate(move |_| {
            run();
        });
    }

    let scrolled = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .vscrollbar_policy(gtk::PolicyType::Automatic)
        .hexpand(true)
        .vexpand(true)
        .child(&content_box)
        .build();

    panel.append(&scrolled);

    let first_focus = name_entry.clone();
    panel.connect_map(move |_| {
        first_focus.grab_focus();
    });

    panel
}
