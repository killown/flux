use crate::i18n::tr;
use crate::model::{AppMsg, FluxApp};
use crate::services::extension_search::AdvancedSearchParams;
use adw::prelude::*;
use relm4::AsyncComponentSender;

/// Opens the advanced search dialog configured with segmented views.
pub fn show_advanced_search(app: &mut FluxApp, sender: AsyncComponentSender<FluxApp>) {
    let window = gtk::Application::default().active_window();

    let fallback_path = if app.current_path.to_string_lossy().starts_with("search://") {
        app.history
            .last()
            .cloned()
            .unwrap_or_else(|| std::path::PathBuf::from("/"))
    } else {
        app.current_path.clone()
    };

    let dialog = adw::Window::builder()
        .title(tr("Advanced Search"))
        .modal(true)
        .default_width(480)
        .default_height(520)
        .resizable(false)
        .build();

    if let Some(win) = &window {
        dialog.set_transient_for(Some(win));
    }

    let root = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .hexpand(true)
        .vexpand(true)
        .build();

    // ── Header Bar ───────────────────────────────────────────────────────────
    let header = adw::HeaderBar::builder()
        .show_end_title_buttons(false)
        .show_start_title_buttons(false)
        .build();

    let cancel_btn = gtk::Button::builder().label(tr("Cancel")).build();
    header.pack_start(&cancel_btn);

    let search_btn = gtk::Button::builder()
        .label(tr("Search"))
        .css_classes(["suggested-action"])
        .build();
    header.pack_end(&search_btn);

    let stack = gtk::Stack::builder()
        .transition_type(gtk::StackTransitionType::SlideLeftRight)
        .transition_duration(200)
        .hexpand(true)
        .vexpand(true)
        .build();

    let switcher = gtk::StackSwitcher::builder().stack(&stack).build();
    header.set_title_widget(Some(&switcher));

    root.append(&header);

    // ── Helper functions for rows ────────────────────────────────────────────
    fn make_entry_row(group: &adw::PreferencesGroup, title: &str, placeholder: &str) -> gtk::Entry {
        let entry = gtk::Entry::builder()
            .placeholder_text(placeholder)
            .hexpand(true)
            .valign(gtk::Align::Center)
            .build();
        let row = adw::ActionRow::builder().title(title).build();
        row.add_suffix(&entry);
        row.set_activatable_widget(Some(&entry));
        group.add(&row);
        entry
    }

    fn make_switch_row(group: &adw::PreferencesGroup, title: &str, active: bool) -> gtk::Switch {
        let sw = gtk::Switch::builder()
            .active(active)
            .valign(gtk::Align::Center)
            .build();
        let row = adw::ActionRow::builder()
            .title(title)
            .activatable_widget(&sw)
            .build();
        row.add_suffix(&sw);
        group.add(&row);
        sw
    }

    fn make_combo_row(
        group: &adw::PreferencesGroup,
        title: &str,
        items: &[&str],
        selected: u32,
    ) -> adw::ComboRow {
        let model = gtk::StringList::new(items);
        let row = adw::ComboRow::builder()
            .title(title)
            .model(&model)
            .selected(selected)
            .build();
        group.add(&row);
        row
    }

    // ── Tab 1: Criteria ──────────────────────────────────────────────────────
    let criteria_box = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(6)
        .margin_top(6)
        .margin_bottom(6)
        .margin_start(12)
        .margin_end(12)
        .build();

    let criteria_group = adw::PreferencesGroup::builder()
        .title(tr("Search Criteria"))
        .build();

    let name_entry = make_entry_row(
        &criteria_group,
        &tr("File name"),
        "report, draft*, document",
    );
    let content_entry = make_entry_row(
        &criteria_group,
        &tr("Inside files"),
        &tr("At least 3 characters"),
    );
    let fname_entry = make_entry_row(
        &criteria_group,
        &tr("Pattern(s)"),
        "*.rs, image/*, report??.pdf",
    );
    let ext_entry = make_entry_row(&criteria_group, &tr("Limit to extension"), "rs, py, txt");
    let tag_entry = make_entry_row(&criteria_group, &tr("Filter by Tag"), "#work, #project");
    criteria_box.append(&criteria_group);

    let scope_group = adw::PreferencesGroup::builder()
        .title(tr("Search Scope"))
        .description(fallback_path.to_string_lossy().as_ref())
        .build();

    let recursive_sw = make_switch_row(&scope_group, &tr("Search subdirectories"), true);
    let hidden_sw = make_switch_row(&scope_group, &tr("Include hidden files"), false);
    criteria_box.append(&scope_group);

    // ── Tab 2: Options ───────────────────────────────────────────────────────
    let options_box = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(6)
        .margin_top(6)
        .margin_bottom(6)
        .margin_start(12)
        .margin_end(12)
        .build();

    let filters_group = adw::PreferencesGroup::builder()
        .title(tr("Filters"))
        .build();

    let date_row = make_combo_row(
        &filters_group,
        &tr("Modified within"),
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
        &filters_group,
        &tr("Condition"),
        &[&tr("Any size"), &tr("Larger than"), &tr("Smaller than")],
        0,
    );

    let size_entry = gtk::Entry::builder()
        .placeholder_text("0")
        .input_purpose(gtk::InputPurpose::Digits)
        .hexpand(true)
        .valign(gtk::Align::Center)
        .sensitive(false)
        .build();
    let size_amount_row = adw::ActionRow::builder()
        .title(tr("Amount"))
        .sensitive(false)
        .build();
    size_amount_row.add_suffix(&size_entry);
    size_amount_row.set_activatable_widget(Some(&size_entry));
    filters_group.add(&size_amount_row);

    let size_unit_row = make_combo_row(&filters_group, &tr("Unit"), &["KB", "MB", "GB"], 1);
    size_unit_row.set_sensitive(false);
    options_box.append(&filters_group);

    {
        let size_entry = size_entry.clone();
        let size_amount_row = size_amount_row.clone();
        let size_unit_row = size_unit_row.clone();
        size_op_row.connect_selected_notify(move |row| {
            let active = row.selected() != 0;
            size_entry.set_sensitive(active);
            size_amount_row.set_sensitive(active);
            size_unit_row.set_sensitive(active);
        });
    }

    // ── Mount Tabs into Stack ────────────────────────────────────────────────
    let criteria_scroll = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .vscrollbar_policy(gtk::PolicyType::Never)
        .propagate_natural_height(true)
        .vexpand(true)
        .hexpand(true)
        .child(&criteria_box)
        .build();

    let options_scroll = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .vscrollbar_policy(gtk::PolicyType::Never)
        .propagate_natural_height(true)
        .vexpand(true)
        .hexpand(true)
        .child(&options_box)
        .build();

    stack.add_titled(&criteria_scroll, Some("criteria"), &tr("Criteria"));
    stack.add_titled(&options_scroll, Some("options"), &tr("Options"));

    root.append(&stack);
    dialog.set_content(Some(&root));

    let search_submitted = std::rc::Rc::new(std::cell::Cell::new(false));

    // ── Cancel ───────────────────────────────────────────────────────────────
    {
        let d = dialog.clone();
        cancel_btn.connect_clicked(move |_| d.close());
    }

    // ── Search ───────────────────────────────────────────────────────────────
    {
        let name_entry = name_entry.clone();
        let fname_entry = fname_entry.clone();
        let content_entry = content_entry.clone();
        let ext_entry = ext_entry.clone();
        let tag_entry = tag_entry.clone();
        let date_row = date_row.clone();
        let size_op_row = size_op_row.clone();
        let size_entry = size_entry.clone();
        let size_unit_row = size_unit_row.clone();
        let recursive_sw = recursive_sw.clone();
        let hidden_sw = hidden_sw.clone();
        let d = dialog.clone();
        let sender = sender.clone();
        let submitted = search_submitted.clone();

        search_btn.connect_clicked(move |_| {
            submitted.set(true);
            d.close();

            let name_text = name_entry.text().trim().to_string();
            let fname_text = fname_entry.text().trim().to_string();
            let content_text = content_entry.text().trim().to_string();
            let ext_text = ext_entry.text().trim().to_string();
            let tag_text = tag_entry.text().trim().to_string();
            let date_sel = date_row.selected();
            let size_op_sel = size_op_row.selected();
            let size_val: u64 = size_entry.text().trim().parse().unwrap_or(0);
            let size_unit_sel = size_unit_row.selected();
            let mut recursive = recursive_sw.is_active();
            let include_hidden = hidden_sw.is_active();

            let date_seconds: Option<u64> = match date_sel {
                1 => Some(3600),
                2 => Some(86400),
                3 => Some(7 * 86400),
                4 => Some(30 * 86400),
                5 => Some(365 * 86400),
                _ => None,
            };

            let size_bytes: Option<(bool, u64)> = if size_op_sel != 0 && size_val > 0 {
                let multiplier: u64 = match size_unit_sel {
                    0 => 1024,
                    1 => 1024 * 1024,
                    _ => 1024 * 1024 * 1024,
                };
                Some((size_op_sel == 1, size_val * multiplier))
            } else {
                None
            };

            if content_text.len() >= 3 {
                let ext_filter = if ext_text.is_empty() {
                    None
                } else {
                    Some(ext_text)
                };
                apply_flat_filters(&sender, date_seconds, size_bytes, tag_text);
                sender.input(AppMsg::StartContentSearch(content_text, ext_filter));
                return;
            }

            let mut patterns: Vec<String> = Vec::new();

            if !name_text.is_empty() {
                for item in name_text
                    .split(',')
                    .map(|s| s.trim())
                    .filter(|s| !s.is_empty())
                {
                    let mut term = item.to_string();
                    if term.ends_with('*') || term.contains('*') {
                        recursive = true;
                    }
                    if !term.starts_with('*') && !term.ends_with('*') {
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
                sender.input(AppMsg::StartAdvancedSearch(AdvancedSearchParams {
                    patterns,
                    date_seconds,
                    size_bytes,
                    include_hidden,
                    max_results: 0,
                }));
                return;
            }

            if !patterns.is_empty() {
                sender.input(AppMsg::SetExtensionFilter(patterns));
            }

            apply_flat_filters(&sender, date_seconds, size_bytes, tag_text);
        });
    }

    {
        let name = name_entry.clone();
        dialog.connect_map(move |_| {
            name.grab_focus();
        });
    }

    // Recover path on close if no search was executed
    {
        let sender = sender.clone();
        let fallback_path = fallback_path.clone();
        let submitted = search_submitted.clone();

        dialog.connect_close_request(move |_| {
            if !submitted.get() {
                sender.input(AppMsg::CancelContentSearch);
                sender.input(AppMsg::Navigate(fallback_path.clone()));
            }
            gtk::glib::Propagation::Proceed
        });
    }

    // Escape closes.
    {
        let d = dialog.clone();
        let key_ctrl = gtk::EventControllerKey::new();
        key_ctrl.connect_key_pressed(move |_, keyval, _, _| {
            if keyval == adw::gdk::Key::Escape {
                d.close();
                return gtk::glib::Propagation::Stop;
            }
            gtk::glib::Propagation::Proceed
        });
        dialog.add_controller(key_ctrl);
    }

    dialog.present();
}

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
        for tag in tag_text
            .split(',')
            .map(|t| t.trim())
            .filter(|t| !t.is_empty())
        {
            parts.push(format!(":tag:{}", tag.trim_start_matches('#')));
        }
    }

    if !parts.is_empty() {
        sender.input(AppMsg::UpdateFilter(parts.join(" ")));
    }
}
