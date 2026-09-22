use crate::i18n::tr;
use crate::model::{AppMsg, FluxApp};
use crate::services::extension_search::AdvancedSearchParams;
use adw::prelude::*;
use relm4::AsyncComponentSender;

fn apply_flat_filters(
    sender: &AsyncComponentSender<FluxApp>,
    date_seconds: Option<u64>,
    size_bytes: Option<(bool, u64)>,
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

    if !parts.is_empty() {
        sender.input(AppMsg::UpdateFilter(parts.join(" ")));
    }
}

/// Builds and returns the lazy-initialized right search sidebar panel widget tree.
pub fn build_search_panel(initial_width: i32, sender: AsyncComponentSender<FluxApp>) -> gtk::Box {
    let effective_width = if initial_width <= 0 {
        350
    } else {
        initial_width.clamp(250, 800)
    };

    let panel = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(0)
        .width_request(effective_width)
        .hexpand(false)
        .build();

    panel.add_css_class("sidebar");

    // ── Drag Handle (Left Edge) using a decoupled EventControllerMotion ─────────
    // Using root coordinates instead of local widget delta avoids coordinate shifts
    let resize_handle = gtk::Separator::builder()
        .orientation(gtk::Orientation::Vertical)
        .css_classes(["sidebar-resize-handle"])
        .build();

    let drag_gesture = gtk::GestureDrag::new();
    let start_width = std::rc::Rc::new(std::cell::Cell::new(effective_width));
    let start_root_x = std::rc::Rc::new(std::cell::Cell::new(0.0));
    let hover_timer = std::rc::Rc::new(std::cell::Cell::new(None::<gtk::glib::SourceId>));
    let is_ready = std::rc::Rc::new(std::cell::Cell::new(false));

    let motion_ctrl = gtk::EventControllerMotion::new();

    {
        let timer_c = hover_timer.clone();
        let ready_c = is_ready.clone();
        motion_ctrl.connect_enter(move |ctrl, _, _| {
            if let Some(id) = timer_c.take() {
                id.remove();
            }
            ready_c.set(false);

            let ctrl_weak = ctrl.downgrade();
            let timer_inner = timer_c.clone();
            let ready_inner = ready_c.clone();

            let id = gtk::glib::timeout_add_local_once(
                std::time::Duration::from_millis(100),
                move || {
                    timer_inner.set(None);
                    ready_inner.set(true);
                    if let Some(c) = ctrl_weak.upgrade() {
                        if let Some(widget) = c.widget() {
                            widget.set_cursor_from_name(Some("col-resize"));
                        }
                    }
                },
            );
            timer_c.set(Some(id));
        });
    }

    {
        let timer_c = hover_timer;
        let ready_c = is_ready.clone();
        motion_ctrl.connect_leave(move |ctrl| {
            if let Some(id) = timer_c.take() {
                id.remove();
            }
            ready_c.set(false);
            if let Some(widget) = ctrl.widget() {
                widget.set_cursor(None);
            }
        });
    }

    resize_handle.add_controller(motion_ctrl);

    {
        let panel_weak = panel.downgrade();
        let start_width_c = start_width.clone();
        let start_root_x_c = start_root_x.clone();
        let ready_c = is_ready.clone();

        drag_gesture.connect_drag_begin(move |gesture, x, _| {
            if !ready_c.get() {
                gesture.set_state(gtk::EventSequenceState::Denied);
                return;
            }
            if let Some(p) = panel_weak.upgrade() {
                start_width_c.set(p.width());
                // Translate the initial click point to root/window coordinate space
                // Root coordinates remain completely static while children resize!
                if let Some(root) = p.root() {
                    if let Some(handle) = gesture.widget() {
                        let (rx, _) = handle
                            .translate_coordinates(&root, x, 0.0)
                            .unwrap_or((x, 0.0));
                        start_root_x_c.set(rx);
                    }
                }
            }
        });
    }

    {
        let panel_weak = panel.downgrade();
        let start_width_c = start_width.clone();
        let start_root_x_c = start_root_x.clone();
        let ready_c = is_ready.clone();

        drag_gesture.connect_drag_update(move |gesture, _, _| {
            if !ready_c.get() {
                return;
            }
            if let Some(p) = panel_weak.upgrade() {
                if let Some(root) = p.root() {
                    if let Some(handle) = gesture.widget() {
                        // Query the current point and translate directly to root coordinates
                        if let Some((curr_x, _)) = gesture.point(None) {
                            if let Some((curr_root_x, _)) =
                                handle.translate_coordinates(&root, curr_x, 0.0)
                            {
                                // Real delta = how much the pointer moved in global window space
                                let delta_x = curr_root_x - start_root_x_c.get();
                                let new_w = (start_width_c.get() - delta_x as i32).clamp(250, 800);

                                if p.width_request() != new_w {
                                    p.set_width_request(new_w);
                                }
                            }
                        }
                    }
                }
            }
        });
    }

    {
        let panel_weak = panel.downgrade();
        let s = sender.clone();
        let ready_c = is_ready;

        drag_gesture.connect_drag_end(move |gesture, _, _| {
            if !ready_c.get() {
                return;
            }
            if let Some(p) = panel_weak.upgrade() {
                let final_width = p.width().clamp(250, 800);
                p.set_width_request(final_width);
                s.input(AppMsg::SetSearchPanelWidth(final_width));
            }
            if let Some(widget) = gesture.widget() {
                widget.set_cursor(None);
            }
        });
    }

    resize_handle.add_controller(drag_gesture);

    let root_container = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(0)
        .hexpand(false)
        .halign(gtk::Align::End)
        .build();

    root_container.append(&resize_handle);
    root_container.append(&panel);

    // ── Header: [search icon] "Search" ... [Search Button] [X close button] ──
    let header_box = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(8)
        .margin_start(12)
        .margin_end(12)
        .margin_top(8)
        .margin_bottom(4)
        .build();

    let search_icon = gtk::Image::from_icon_name("system-search-symbolic");

    let title_label = gtk::Label::builder()
        .label(tr("Search"))
        .css_classes(["heading"])
        .hexpand(true)
        .xalign(0.0)
        .build();

    let reset_btn = gtk::Button::builder()
        .icon_name("edit-clear-symbolic")
        .css_classes(["flat", "circular"])
        .valign(gtk::Align::Center)
        .tooltip_text(tr("Clear filter"))
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
    header_box.append(&reset_btn);
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
    root_container.add_controller(esc_controller);

    // ── Panel Body ───────────────────────────────────────────────────────────
    let content_box = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(12)
        .margin_start(12)
        .margin_end(12)
        .margin_top(4)
        .margin_bottom(12)
        .build();

    // Error label starts hidden and unallocated so it doesn't take up any vertical space by default
    let error_box = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(8)
        .css_classes(["error"])
        .margin_start(2)
        .margin_end(2)
        .visible(false)
        .build();

    let error_icon = gtk::Image::from_icon_name("dialog-warning-symbolic");
    let error_label = gtk::Label::builder()
        .label("")
        .css_classes(["dim-label"])
        .halign(gtk::Align::Start)
        .wrap(true)
        .hexpand(true)
        .build();

    error_box.append(&error_icon);
    error_box.append(&error_label);
    content_box.append(&error_box);

    let what_group = adw::PreferencesGroup::builder()
        .title(tr("What to find"))
        .build();

    let make_stacked_entry =
        |group: &adw::PreferencesGroup, title: &str, placeholder: &str| -> gtk::Entry {
            let container = gtk::Box::builder()
                .orientation(gtk::Orientation::Vertical)
                .spacing(4)
                .margin_start(10)
                .margin_end(10)
                .margin_top(8)
                .margin_bottom(8)
                .build();

            let label = gtk::Label::builder()
                .label(title)
                .halign(gtk::Align::Start)
                .css_classes(["caption", "dim-label"])
                .build();

            let entry = gtk::Entry::builder()
                .placeholder_text(placeholder)
                .hexpand(true)
                .build();

            container.append(&label);
            container.append(&entry);

            let row = adw::PreferencesRow::builder().child(&container).build();
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

    let name_entry = make_stacked_entry(&what_group, &tr("File name"), "invoice, draft, photo");
    let exact_match_sw = make_switch_row(
        &what_group,
        &tr("Exact match"),
        &tr("Match exact filename without wildcards"),
        false,
    );
    let regex_sw = make_switch_row(
        &what_group,
        &tr("Regular expression"),
        &tr("Use regular expressions for file name matching"),
        false,
    );
    let content_entry = make_stacked_entry(
        &what_group,
        &tr("Inside files"),
        &tr("Requires 3+ characters"),
    );
    let fname_entry = make_stacked_entry(&what_group, &tr("Glob pattern"), "rs, image/*, pdf");
    let ext_entry = make_stacked_entry(&what_group, &tr("Extension"), "rs, py, txt");

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

    let size_container = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(4)
        .margin_start(10)
        .margin_end(10)
        .margin_top(8)
        .margin_bottom(8)
        .build();

    let size_label = gtk::Label::builder()
        .label(tr("Amount"))
        .halign(gtk::Align::Start)
        .css_classes(["caption", "dim-label"])
        .sensitive(false)
        .build();

    let size_box = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(6)
        .hexpand(true)
        .build();

    let size_entry = gtk::Entry::builder()
        .placeholder_text("0")
        .input_purpose(gtk::InputPurpose::Digits)
        .hexpand(true)
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
    size_container.append(&size_label);
    size_container.append(&size_box);

    let size_amount_row = adw::PreferencesRow::builder()
        .child(&size_container)
        .sensitive(false)
        .build();

    {
        let size_entry_c = size_entry.clone();
        let amount_row_c = size_amount_row.clone();
        let size_label_c = size_label.clone();
        let unit_combo_c = size_unit_combo.clone();
        size_op_row.connect_selected_notify(move |row| {
            let active = row.selected() != 0;
            size_entry_c.set_sensitive(active);
            size_label_c.set_sensitive(active);
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
        let regex_sw_clone = regex_sw.clone();
        let content_e = content_entry.clone();
        let fname_e = fname_entry.clone();
        let ext_e = ext_entry.clone();
        let rec_sw = recursive_sw.clone();
        let hid_sw = hidden_sw.clone();
        let date_r = date_row.clone();
        let size_op_r = size_op_row.clone();
        let size_e = size_entry.clone();
        let size_unit_c = size_unit_combo.clone();
        let error_label_c = error_label.clone();
        let error_box_c = error_box.clone();

        move || {
            let name_text = name_e.text().trim().to_string();
            let fname_text = fname_e.text().trim().to_string();
            let content_text = content_e.text().trim().to_string();
            let ext_text = ext_e.text().trim().to_string();
            let mut recursive = rec_sw.is_active();
            let include_hidden = hid_sw.is_active();
            let exact_match = exact_sw.is_active();
            let use_regex = regex_sw_clone.is_active();

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
                error_box_c.set_visible(false);
                apply_flat_filters(&s, date_seconds, size_bytes);
                s.input(AppMsg::StartContentSearch(content_text, ext_filter));
                return;
            }

            let mut patterns: Vec<String> = Vec::new();
            let mut regex_error = None;

            let mut filter_globs: Vec<String> = Vec::new();
            if !fname_text.is_empty() {
                for p in fname_text
                    .split(',')
                    .map(str::trim)
                    .filter(|p| !p.is_empty())
                {
                    let mut pat = p.to_lowercase();
                    if !pat.contains('*') && !pat.contains('?') {
                        pat = format!("*.{}", pat.trim_start_matches('.'));
                    }
                    filter_globs.push(pat);
                }
            } else if !ext_text.is_empty() {
                for e in ext_text.split(',').map(str::trim).filter(|e| !e.is_empty()) {
                    let ext = e.trim_start_matches('.');
                    filter_globs.push(format!("*.{}", ext.to_lowercase()));
                }
            }

            let mut name_patterns: Vec<String> = Vec::new();
            if !name_text.is_empty() {
                for item in name_text
                    .split(',')
                    .map(str::trim)
                    .filter(|slice| !slice.is_empty())
                {
                    let term = item.to_string();
                    if use_regex {
                        recursive = true;
                        match regex::Regex::new(&term) {
                            Ok(_) => {
                                name_patterns.push(format!("regex:{}", term));
                            }
                            Err(e) => {
                                regex_error = Some(format!("Invalid regex: {}", e));
                            }
                        }
                    } else if exact_match {
                        name_patterns.push(term.to_lowercase());
                    } else {
                        if term.contains('*') {
                            recursive = true;
                        }
                        let words: Vec<&str> = term.split_whitespace().collect();
                        let formatted_term = if words.len() > 1 {
                            format!("*{}*", words.join("*"))
                        } else if !term.starts_with('*') && !term.ends_with('*') {
                            format!("*{}*", term)
                        } else {
                            term
                        };
                        name_patterns.push(formatted_term.to_lowercase());
                    }
                }
            }

            if let Some(err_msg) = regex_error {
                error_label_c.set_label(&err_msg);
                error_box_c.set_visible(true);
                return;
            } else {
                error_box_c.set_visible(false);
            }

            if !name_patterns.is_empty() && !filter_globs.is_empty() {
                for np in &name_patterns {
                    if np.starts_with("regex:") {
                        patterns.push(np.clone());
                    } else {
                        for fg in &filter_globs {
                            let base_np = if np.ends_with('*') {
                                np.clone()
                            } else {
                                format!("{}*", np)
                            };
                            let base_fg = if fg.starts_with('*') {
                                fg.trim_start_matches('*').to_string()
                            } else {
                                fg.clone()
                            };
                            patterns.push(format!("{}*{}", base_np.trim_end_matches('*'), base_fg));
                        }
                    }
                }
            } else if !name_patterns.is_empty() {
                patterns = name_patterns;
            } else if !filter_globs.is_empty() {
                patterns = filter_globs;
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

            apply_flat_filters(&s, date_seconds, size_bytes);
        }
    };

    // ── Reset Button Action ──────────────────────────────────────────────────
    {
        let s = sender.clone();
        let name_e = name_entry.clone();
        let content_e = content_entry.clone();
        let fname_e = fname_entry.clone();
        let ext_e = ext_entry.clone();
        let exact_sw = exact_match_sw.clone();
        let regex_sw_c = regex_sw.clone();
        let rec_sw = recursive_sw.clone();
        let hid_sw = hidden_sw.clone();
        let date_r = date_row.clone();
        let size_op_r = size_op_row.clone();
        let size_e = size_entry.clone();
        let error_box_c = error_box.clone();

        reset_btn.connect_clicked(move |_| {
            name_e.set_text("");
            content_e.set_text("");
            fname_e.set_text("");
            ext_e.set_text("");
            size_e.set_text("");
            exact_sw.set_active(false);
            regex_sw_c.set_active(false);
            rec_sw.set_active(true);
            hid_sw.set_active(false);
            date_r.set_selected(0);
            size_op_r.set_selected(0);
            error_box_c.set_visible(false);

            s.input(AppMsg::ClearExtensionFilter);
            s.input(AppMsg::UpdateFilter(String::new()));
            s.input(AppMsg::Refresh);
            name_e.grab_focus();
        });
    }

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
        .propagate_natural_width(false)
        .hexpand(false)
        .vexpand(true)
        .child(&content_box)
        .build();

    panel.append(&scrolled);

    let first_focus = name_entry.clone();
    panel.connect_map(move |_| {
        first_focus.grab_focus();
    });

    root_container
}
