use crate::i18n::tr;
use crate::model::{AppMsg, FluxApp};
use crate::services::extension_search::AdvancedSearchParams;
use adw::prelude::*;
use relm4::AsyncComponentSender;

#[allow(clippy::too_many_arguments)]
fn submit_search(
    sender: &AsyncComponentSender<FluxApp>,
    dialog: &adw::Window,
    name_entry: &gtk::Entry,
    fname_entry: &gtk::Entry,
    content_entry: &gtk::Entry,
    ext_entry: &gtk::Entry,
    tag_entry: &gtk::Entry,
    date_row: &adw::ComboRow,
    size_op_row: &adw::ComboRow,
    size_entry: &gtk::Entry,
    size_unit_dd: &gtk::DropDown,
    recursive_sw: &gtk::Switch,
    hidden_sw: &gtk::Switch,
    exact_match_sw: &gtk::Switch,
    search_submitted: &std::rc::Rc<std::cell::Cell<bool>>,
) {
    search_submitted.set(true);
    dialog.close();

    let name_text = name_entry.text().trim().to_string();
    let fname_text = fname_entry.text().trim().to_string();
    let content_text = content_entry.text().trim().to_string();
    let ext_text = ext_entry.text().trim().to_string();
    let tag_text = tag_entry.text().trim().to_string();
    let date_sel = date_row.selected();
    let size_op_sel = size_op_row.selected();
    let size_val: u64 = size_entry.text().trim().parse().unwrap_or(0);
    let size_unit_sel = size_unit_dd.selected();
    let mut recursive = recursive_sw.is_active();
    let include_hidden = hidden_sw.is_active();
    let exact_match = exact_match_sw.is_active();

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

    // Content search takes priority when the field has ≥3 chars.
    if content_text.len() >= 3 {
        let ext_filter = if ext_text.is_empty() {
            None
        } else {
            Some(ext_text)
        };
        apply_flat_filters(sender, date_seconds, size_bytes, tag_text);
        sender.input(AppMsg::StartContentSearch(content_text, ext_filter));
        return;
    }

    let mut patterns: Vec<String> = Vec::new();

    if !name_text.is_empty() {
        for item in name_text
            .split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
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

    apply_flat_filters(sender, date_seconds, size_bytes, tag_text);
}

/// Opens the advanced search dialog following GNOME Human Interface Guidelines.
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

    // ── Window ───────────────────────────────────────────────────────────────
    let dialog = adw::Window::builder()
        .title(tr("Search"))
        .modal(true)
        .default_width(500)
        .default_height(650)
        .resizable(false)
        .build();

    if let Some(win) = &window {
        dialog.set_transient_for(Some(win));
    }

    // ── Root layout ──────────────────────────────────────────────────────────
    let root = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .build();

    // ── Header Bar ───────────────────────────────────────────────────────────
    let header = adw::HeaderBar::builder()
        .show_end_title_buttons(false)
        .show_start_title_buttons(false)
        .build();

    let cancel_btn = gtk::Button::builder().label(tr("Cancel")).build();
    cancel_btn.add_css_class("flat");
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

    // ── Helper closures ──────────────────────────────────────────────────────
    let make_entry_row =
        |group: &adw::PreferencesGroup, title: &str, placeholder: &str| -> gtk::Entry {
            let entry = gtk::Entry::builder()
                .placeholder_text(placeholder)
                .width_chars(30)
                .max_width_chars(30)
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

    let make_combo_row = |group: &adw::PreferencesGroup,
                          title: &str,
                          items: &[&str],
                          selected: u32|
     -> adw::ComboRow {
        let model = gtk::StringList::new(items);
        let row = adw::ComboRow::builder()
            .title(title)
            .model(&model)
            .selected(selected)
            .build();
        group.add(&row);
        row
    };

    // ── Page 1: Search ───────────────────────────────────────────────────────
    let search_page = adw::PreferencesPage::builder()
        .title(tr("Search"))
        .icon_name("system-search-symbolic")
        .build();

    let what_group = adw::PreferencesGroup::builder()
        .title(tr("What to find"))
        .build();

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

    search_page.add(&what_group);

    let scope_group = adw::PreferencesGroup::builder()
        .title(tr("Where to look"))
        .description(fallback_path.to_string_lossy().as_ref())
        .build();

    let recursive_sw = make_switch_row(&scope_group, &tr("Search inside subfolders"), "", true);
    let hidden_sw = make_switch_row(
        &scope_group,
        &tr("Include hidden files"),
        &tr("Files beginning with a dot"),
        false,
    );

    search_page.add(&scope_group);

    // ── Page 2: Filters ──────────────────────────────────────────────────────
    let filters_page = adw::PreferencesPage::builder()
        .title(tr("Filters"))
        .icon_name("funnel-symbolic")
        .build();

    let filters_group = adw::PreferencesGroup::builder()
        .title(tr("Narrow results"))
        .build();

    let date_row = make_combo_row(
        &filters_group,
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
        &filters_group,
        &tr("File size"),
        &[&tr("Any size"), &tr("Larger than"), &tr("Smaller than")],
        0,
    );

    // Amount + unit on the same row only when a size operator is chosen.
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
        .selected(1) // default MB
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
    filters_group.add(&size_amount_row);

    filters_page.add(&filters_group);

    // Enable/disable amount row when size operator changes.
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

    // ── Wrap pages in the stack ──────────────────────────────────────────────
    stack.add_titled(&search_page, Some("search"), &tr("Search"));
    stack.add_titled(&filters_page, Some("filters"), &tr("Filters"));

    stack
        .page(&search_page)
        .set_icon_name("system-search-symbolic");
    stack
        .page(&filters_page)
        .set_icon_name("view-filter-symbolic");

    stack.set_visible_child_name("search");

    root.append(&stack);
    dialog.set_content(Some(&root));

    // ── Shared state ─────────────────────────────────────────────────────────
    let search_submitted = std::rc::Rc::new(std::cell::Cell::new(false));

    // ── Helper macro: build a closure that fires submit_search ───────────────
    macro_rules! make_submit {
        ($dialog:expr, $sender:expr, $submitted:expr) => {{
            let dialog = $dialog.clone();
            let sender = $sender.clone();
            let submitted = $submitted.clone();
            let name_entry = name_entry.clone();
            let fname_entry = fname_entry.clone();
            let content_entry = content_entry.clone();
            let ext_entry = ext_entry.clone();
            let tag_entry = tag_entry.clone();
            let date_row = date_row.clone();
            let size_op_row = size_op_row.clone();
            let size_entry = size_entry.clone();
            let size_unit_combo = size_unit_combo.clone();
            let recursive_sw = recursive_sw.clone();
            let hidden_sw = hidden_sw.clone();
            let exact_match_sw = exact_match_sw.clone();
            move || {
                submit_search(
                    &sender,
                    &dialog,
                    &name_entry,
                    &fname_entry,
                    &content_entry,
                    &ext_entry,
                    &tag_entry,
                    &date_row,
                    &size_op_row,
                    &size_entry,
                    &size_unit_combo,
                    &recursive_sw,
                    &hidden_sw,
                    &exact_match_sw,
                    &submitted,
                );
            }
        }};
    }

    // ── Search button ────────────────────────────────────────────────────────
    {
        let do_search = make_submit!(dialog, sender, search_submitted);
        search_btn.connect_clicked(move |_| do_search());
    }

    // ── Enter on any Entry ────────────────────────────────────────────────────
    // HIG: pressing Enter in any field should immediately execute the primary
    // action - the user should never have to reach for the mouse.
    for entry in [
        &name_entry,
        &fname_entry,
        &content_entry,
        &ext_entry,
        &tag_entry,
        &size_entry,
    ] {
        let do_search = make_submit!(dialog, sender, search_submitted);
        entry.connect_activate(move |_| do_search());
    }

    // ── Cancel ────────────────────────────────────────────────────────────────
    {
        let d = dialog.clone();
        cancel_btn.connect_clicked(move |_| d.close());
    }

    // ── Escape ────────────────────────────────────────────────────────────────
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

    // ── Recover path on close if no search was executed ───────────────────────
    {
        let sender = sender.clone();
        let fallback = fallback_path.clone();
        let submitted = search_submitted.clone();
        dialog.connect_close_request(move |_| {
            if !submitted.get() {
                sender.input(AppMsg::CancelContentSearch);
                sender.input(AppMsg::Navigate(fallback.clone()));
            }
            gtk::glib::Propagation::Proceed
        });
    }

    // ── Auto-focus first field ─────────────────────────────────────────────
    {
        let name = name_entry.clone();
        dialog.connect_map(move |_| {
            name.grab_focus();
        });
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
        for tag in tag_text.split(',').map(str::trim).filter(|t| !t.is_empty()) {
            parts.push(format!(":tag:{}", tag.trim_start_matches('#')));
        }
    }

    if !parts.is_empty() {
        sender.input(AppMsg::UpdateFilter(parts.join(" ")));
    }
}
