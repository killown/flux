use crate::model::{AppMsg, FluxApp};
use crate::services::inspector::scan_directory_native;
use adw::prelude::*;
use relm4::prelude::*;
use std::path::PathBuf;

impl FluxApp {
    pub fn show_dir_inspector_dialog(
        &self,
        target_dir: PathBuf,
        sender: &AsyncComponentSender<Self>,
    ) {
        let window = gtk::Application::default().active_window();
        let s = sender.clone();

        let dir_name = target_dir
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| target_dir.display().to_string());

        let dialog = adw::Window::builder()
            .title(crate::i18n::tr("Directory Inspector"))
            .modal(true)
            .default_width(620)
            .default_height(680)
            .resizable(false)
            .build();

        if let Some(ref win) = window {
            dialog.set_transient_for(Some(win));
        }

        let root = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .hexpand(true)
            .vexpand(true)
            .build();

        let header = adw::HeaderBar::builder()
            .show_start_title_buttons(false)
            .show_end_title_buttons(false)
            .build();

        let title_widget = adw::WindowTitle::builder()
            .title(crate::i18n::tr("Directory Overview"))
            .subtitle(dir_name.as_str())
            .build();
        header.set_title_widget(Some(&title_widget));

        let close_btn = gtk::Button::builder()
            .label(crate::i18n::tr("Close"))
            .build();
        header.pack_end(&close_btn);
        root.append(&header);

        let spinner_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(12)
            .valign(gtk::Align::Center)
            .halign(gtk::Align::Center)
            .vexpand(true)
            .build();

        let spinner = gtk::Spinner::builder()
            .spinning(true)
            .width_request(32)
            .height_request(32)
            .build();
        let loading_label = gtk::Label::builder()
            .label(crate::i18n::tr("Scanning directory tree…"))
            .css_classes(["dim-label"])
            .build();

        spinner_box.append(&spinner);
        spinner_box.append(&loading_label);

        let stack = gtk::Stack::builder()
            .transition_type(gtk::StackTransitionType::Crossfade)
            .vexpand(true)
            .hexpand(true)
            .build();

        stack.add_named(&spinner_box, Some("loading"));

        let content_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(16)
            .margin_start(16)
            .margin_end(16)
            .margin_top(12)
            .margin_bottom(16)
            .build();

        let scrolled = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .vscrollbar_policy(gtk::PolicyType::Automatic)
            .child(&content_box)
            .vexpand(true)
            .build();

        stack.add_named(&scrolled, Some("content"));
        root.append(&stack);
        dialog.set_content(Some(&root));

        let d_close = dialog.clone();
        close_btn.connect_clicked(move |_| d_close.close());

        let d_esc = dialog.clone();
        let key_ctrl = gtk::EventControllerKey::new();
        key_ctrl.connect_key_pressed(move |_, keyval, _, _| {
            if keyval == adw::gdk::Key::Escape {
                d_esc.close();
                return gtk::glib::Propagation::Stop;
            }
            gtk::glib::Propagation::Proceed
        });
        dialog.add_controller(key_ctrl);

        dialog.present();

        let stats_slot = std::sync::Arc::new(std::sync::Mutex::new(None));
        let stats_slot_bg = stats_slot.clone();
        let target_scan = target_dir.clone();

        relm4::spawn_blocking(move || {
            let stats = scan_directory_native(&target_scan);
            *stats_slot_bg.lock().unwrap() = Some(stats);
        });

        let stack_clone = stack.clone();
        let target_clone = target_dir.clone();
        let dialog_jump = dialog.clone();
        let s_jump = s.clone();

        glib::timeout_add_local(std::time::Duration::from_millis(50), move || {
            let stats = match stats_slot.lock().unwrap().take() {
                Some(s) => s,
                None => return glib::ControlFlow::Continue,
            };

            let overview_group = adw::PreferencesGroup::builder()
                .title(crate::i18n::tr("Storage Summary"))
                .description(
                    crate::i18n::tr("Scanned in {} ms")
                        .replace("{}", &stats.duration_ms.to_string())
                        .as_str(),
                )
                .build();

            let size_row = adw::ActionRow::builder()
                .title(crate::i18n::tr("Total Size"))
                .subtitle(glib::format_size(stats.total_bytes).as_str())
                .build();
            overview_group.add(&size_row);

            let counts_row = adw::ActionRow::builder()
                .title(crate::i18n::tr("Items"))
                .subtitle(
                    crate::i18n::tr("{} files, {} subfolders")
                        .replacen("{}", &stats.total_files.to_string(), 1)
                        .replacen("{}", &stats.total_dirs.to_string(), 1)
                        .as_str(),
                )
                .build();
            overview_group.add(&counts_row);
            content_box.append(&overview_group);

            let search_entry = gtk::SearchEntry::builder()
                .placeholder_text(crate::i18n::tr(
                    "Filter by name, extension (.mp4), or size (>10M, <1G)...",
                ))
                .hexpand(true)
                .margin_bottom(4)
                .build();
            content_box.append(&search_entry);

            let files_group = adw::PreferencesGroup::builder()
                .title(crate::i18n::tr("Largest Files (Top 100)"))
                .build();

            let display_files: Vec<_> = stats.largest_files.into_iter().take(100).collect();
            let mut row_records: Vec<(adw::ActionRow, String, String, u64, String)> = Vec::new();

            for (path, size) in display_files {
                let fname = path
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string();
                let rel = path
                    .strip_prefix(&target_clone)
                    .unwrap_or(&path)
                    .to_string_lossy()
                    .to_string();
                let ext = path
                    .extension()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_lowercase();
                let size_str = glib::format_size(size).to_string();

                let row = adw::ActionRow::builder()
                    .title(fname.as_str())
                    .subtitle(rel.as_str())
                    .build();

                let gicon = crate::utils::icon::get_icon_for_path(&path, false);
                let icon_img = gtk::Image::from_gicon(&gicon);
                icon_img.set_pixel_size(24);
                row.add_prefix(&icon_img);

                if let Some(parent) = path.parent() {
                    let parent_dir = parent.to_path_buf();
                    let s_action = s_jump.clone();
                    let d_action = dialog_jump.clone();

                    let jump_btn = gtk::Button::builder()
                        .icon_name("folder-symbolic")
                        .tooltip_text(crate::i18n::tr("Open enclosing folder"))
                        .css_classes(["flat", "circular"])
                        .valign(gtk::Align::Center)
                        .build();

                    jump_btn.connect_clicked(move |_| {
                        s_action.input(AppMsg::Navigate(parent_dir.clone()));
                        d_action.close();
                    });

                    row.add_prefix(&jump_btn);
                }

                {
                    let target_file = path.clone();
                    let s_action = s_jump.clone();
                    let row_clone = row.clone();
                    let files_group_clone = files_group.clone();

                    let trash_btn = gtk::Button::builder()
                        .icon_name("user-trash-symbolic")
                        .tooltip_text(crate::i18n::tr("Move to trash"))
                        .css_classes(["flat", "circular"])
                        .valign(gtk::Align::Center)
                        .build();

                    trash_btn.connect_clicked(move |_| {
                        let gfile = gio::File::for_path(&target_file);
                        if let Err(e) = gfile.trash(gio::Cancellable::NONE) {
                            s_action.input(AppMsg::ShowToast(format!("Trash error: {}", e)));
                        } else {
                            s_action
                                .input(AppMsg::ShowToast(crate::i18n::tr("Item moved to trash.")));
                            files_group_clone.remove(&row_clone);
                        }
                    });

                    row.add_prefix(&trash_btn);
                }

                let size_label = gtk::Label::builder()
                    .label(size_str.as_str())
                    .valign(gtk::Align::Center)
                    .css_classes(["accent", "numeric"])
                    .build();
                row.add_suffix(&size_label);

                files_group.add(&row);

                row_records.push((
                    row,
                    fname.to_lowercase(),
                    ext,
                    size,
                    size_str.to_lowercase(),
                ));
            }

            let records_cell = std::rc::Rc::new(std::cell::RefCell::new(row_records));
            {
                let records = records_cell.clone();
                search_entry.connect_search_changed(move |entry| {
                    let query = entry.text().trim().to_lowercase();
                    let list = records.borrow();

                    if query.is_empty() {
                        for (row, _, _, _, _) in list.iter() {
                            row.set_visible(true);
                        }
                        return;
                    }

                    let parse_size_filter = |q: &str| -> Option<(bool, u64)> {
                        let is_gt = q.starts_with('>');
                        let is_lt = q.starts_with('<');
                        if !is_gt && !is_lt {
                            return None;
                        }
                        let val_part = q[1..].trim();
                        let (num_str, mult) = if val_part.ends_with('k') || val_part.ends_with("kb")
                        {
                            (
                                val_part.trim_end_matches(|c: char| c.is_alphabetic()),
                                1_024u64,
                            )
                        } else if val_part.ends_with('m') || val_part.ends_with("mb") {
                            (
                                val_part.trim_end_matches(|c: char| c.is_alphabetic()),
                                1024 * 1024u64,
                            )
                        } else if val_part.ends_with('g') || val_part.ends_with("gb") {
                            (
                                val_part.trim_end_matches(|c: char| c.is_alphabetic()),
                                1024 * 1024 * 1024u64,
                            )
                        } else {
                            (val_part, 1u64)
                        };
                        num_str
                            .parse::<u64>()
                            .ok()
                            .map(|bytes| (is_gt, bytes * mult))
                    };

                    let size_cond = parse_size_filter(&query);
                    let ext_target = query.trim_start_matches('.');

                    for (row, name_lc, ext, size_bytes, size_disp) in list.iter() {
                        let matches = if let Some((gt, thresh)) = size_cond {
                            if gt {
                                *size_bytes >= thresh
                            } else {
                                *size_bytes <= thresh
                            }
                        } else {
                            name_lc.contains(&query)
                                || ext == ext_target
                                || size_disp.contains(&query)
                        };
                        row.set_visible(matches);
                    }
                });
            }

            content_box.append(&files_group);

            stack_clone.set_visible_child_name("content");
            glib::ControlFlow::Break
        });
    }
}
