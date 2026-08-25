use crate::model::{AppMsg, FluxApp};
use crate::ui::constants;
use adw::gdk;
use adw::gio::prelude::*;
use adw::prelude::*;
use gtk::gio;
use relm4::prelude::*;
use std::path::PathBuf;

impl FluxApp {
    /// Constructs and displays the context menu popover based on target path and MIME type.
    pub fn build_and_show_context_menu(
        &mut self,
        x: f64,
        y: f64,
        path: Option<PathBuf>,
        mime: String,
        sender: &AsyncComponentSender<Self>,
    ) {
        self.active_item_path = path.clone();
        let is_in_trash = self
            .current_path
            .to_string_lossy()
            .starts_with(constants::TRASH_URI);
        let root_menu = gio::Menu::new();
        let main_section = gio::Menu::new();

        let mut open_with_item: Option<gio::MenuItem> = None;
        let mut submenu_map: indexmap::IndexMap<String, gio::Menu> = indexmap::IndexMap::new();

        for action in &self.menu_actions {
            let mut matches = false;

            // --- MIME & TRASH MATCHING LOGIC ---
            if is_in_trash {
                if action
                    .mime_types
                    .contains(&constants::FILTER_TRASH.to_string())
                {
                    matches = true;
                }
            } else {
                for allowed_mime in &action.mime_types {
                    if allowed_mime == constants::FILTER_TRASH {
                        continue;
                    }

                    let requirements: Vec<&str> = allowed_mime.split('+').collect();
                    matches = requirements.iter().any(|req| match req.trim() {
                        constants::FILTER_ALL | "all" => true,
                        "image/all" | "image/*" => {
                            mime.starts_with("image/") || mime.starts_with("image-")
                        }
                        "video/all" | "video/*" => {
                            mime.starts_with("video/") || mime.starts_with("video-")
                        }
                        "audio/all" | "audio/*" => {
                            mime.starts_with("audio/") || mime.starts_with("audio-")
                        }
                        "font/all" | "font/*" => {
                            mime.starts_with("font/") || mime.starts_with("font-")
                        }
                        "model/all" | "model/*" => {
                            mime.starts_with("model/") || mime.starts_with("model-")
                        }
                        "message/all" | "message/*" => {
                            mime.starts_with("message/") || mime.starts_with("message-")
                        }
                        "chemical/all" | "chemical/*" => {
                            mime.starts_with("chemical/") || mime.starts_with("chemical-")
                        }
                        "multipart/all" | "multipart/*" => {
                            mime.starts_with("multipart/") || mime.starts_with("multipart-")
                        }
                        "x-content/all" | "x-content/*" => {
                            mime.starts_with("x-content/") || mime.starts_with("x-content-")
                        }
                        "application/all" | "application/*" => {
                            mime.starts_with("application/") || mime.starts_with("application-")
                        }
                        "text/all" | "text/*" => {
                            mime.starts_with("text/")
                                || mime.starts_with("text-")
                                || gio::content_type_is_a(&mime, constants::MIME_TEXT)
                                || mime == constants::MIME_EMPTY
                        }
                        constants::FILTER_FOLDER | "directory" => mime == constants::MIME_DIR,
                        constants::FILTER_FILE => mime != constants::MIME_DIR,
                        t if t.ends_with('/') => {
                            let prefix_dash = format!("{}-", t.trim_end_matches('/'));
                            mime.starts_with(t) || mime.starts_with(&prefix_dash)
                        }
                        t => {
                            t == mime
                                || (t.contains('/') && t.replace('/', "-") == mime)
                                || (t.contains('-') && t.replace('-', "/") == mime)
                        }
                    });
                    if matches {
                        break;
                    }
                }
            }

            // --- MENU ASSEMBLY & BUILTIN MAPPING ---
            if matches {
                // Capture toast before the match so builtin connect_activate closures can
                // emit ShowToast directly.
                let action_toast = action.toast.clone();

                let (full_action_name, lookup_name) = match action.command.as_str() {
                    "builtin::copy" => ("win.copy".to_string(), "copy"),
                    "builtin::cut" => ("win.cut".to_string(), "cut"),
                    "builtin::paste" => ("win.paste".to_string(), "paste"),
                    "builtin::rename" => {
                        let action = gio::SimpleAction::new("rename-item", None);
                        let s = sender.clone();
                        let target = path.clone();
                        let toast = action_toast.clone();
                        action.connect_activate(move |_, _| {
                            if let Some(ref p) = target {
                                s.input(AppMsg::StartRename(p.clone()));
                            }
                            if let Some(ref msg) = toast {
                                s.input(AppMsg::ShowToast(msg.clone()));
                            }
                        });
                        self.action_group.add_action(&action);
                        ("win.rename-item".to_string(), "rename-item")
                    }
                    "builtin::add_to_quick_list" => {
                        let quick_action = gio::SimpleAction::new("add-to-quick-list", None);
                        let sender_q = sender.clone();
                        let toast = action_toast.clone();

                        quick_action.connect_activate(move |_, _| {
                            sender_q.input(AppMsg::AddExclusive(None));
                            if let Some(ref msg) = toast {
                                sender_q.input(AppMsg::ShowToast(msg.clone()));
                            }
                        });
                        self.action_group.add_action(&quick_action);
                        ("win.add-to-quick-list".to_string(), "add-to-quick-list")
                    }
                    "builtin::reset_custom_icon" => {
                        if let Some(ref target) = path {
                            let action = gio::SimpleAction::new("reset-custom-icon", None);
                            let target_clone = target.clone();
                            let s = sender.clone();
                            let toast = action_toast.clone();
                            action.connect_activate(move |_, _| {
                                s.input(AppMsg::ResetFileIcon(target_clone.clone()));
                                if let Some(ref msg) = toast {
                                    s.input(AppMsg::ShowToast(msg.clone()));
                                }
                            });
                            self.action_group.add_action(&action);
                        }
                        ("win.reset-custom-icon".to_string(), "reset-custom-icon")
                    }
                    "builtin::set_custom_icon" => {
                        if let Some(ref target) = path {
                            let set_icon_action = gio::SimpleAction::new("set-custom-icon", None);
                            let target_clone = target.clone();
                            let sender_ic = sender.clone();
                            let toast = action_toast.clone();
                            set_icon_action.connect_activate(move |_, _| {
                                let filter = gtk::FileFilter::new();
                                filter.set_name(Some("Images"));
                                filter.add_mime_type("image/png");
                                filter.add_mime_type("image/jpeg");
                                filter.add_mime_type("image/webp");
                                filter.add_mime_type("image/svg+xml");

                                let toplevels = gtk::Window::list_toplevels();
                                let parent = toplevels
                                    .first()
                                    .and_then(|w| w.downcast_ref::<gtk::Window>())
                                    .cloned();

                                let chooser = gtk::FileChooserNative::builder()
                                    .title("Select Custom Icon Image")
                                    .action(gtk::FileChooserAction::Open)
                                    .accept_label("Set Icon")
                                    .cancel_label("Cancel")
                                    .build();

                                if let Some(ref win) = parent {
                                    chooser.set_transient_for(Some(win));
                                }
                                chooser.add_filter(&filter);

                                let target_path = target_clone.clone();
                                let s = sender_ic.clone();
                                let toast_inner = toast.clone();

                                // Keep `chooser` alive across the async response by cloning
                                // the Arc-like GObject ref into the closure.
                                let chooser_ref = chooser.clone();
                                chooser.connect_response(move |_, response| {
                                    if response == gtk::ResponseType::Accept {
                                        if let Some(file) = chooser_ref.file() {
                                            if let Some(image_path) = file.path() {
                                                s.input(AppMsg::SetFileIcon {
                                                    path: target_path.clone(),
                                                    image_path,
                                                });
                                                if let Some(ref msg) = toast_inner {
                                                    s.input(AppMsg::ShowToast(msg.clone()));
                                                }
                                            }
                                        }
                                    }
                                });
                                chooser.show();
                            });
                            self.action_group.add_action(&set_icon_action);
                        }
                        ("win.set-custom-icon".to_string(), "set-custom-icon")
                    }
                    "builtin::toggle_pin" => {
                        let action = gio::SimpleAction::new("toggle-pin", None);
                        let s = sender.clone();
                        let toast = action_toast.clone();
                        action.connect_activate(move |_, _| {
                            s.input(AppMsg::AddToSidebarPermanent);
                            if let Some(ref msg) = toast {
                                s.input(AppMsg::ShowToast(msg.clone()));
                            }
                        });
                        self.action_group.add_action(&action);
                        ("win.toggle-pin".to_string(), "toggle-pin")
                    }
                    "builtin::delete" => {
                        let action = gio::SimpleAction::new("delete-selection", None);
                        let s = sender.clone();
                        let toast = action_toast.clone();
                        action.connect_activate(move |_, _| {
                            s.input(AppMsg::Delete);
                            if let Some(ref msg) = toast {
                                s.input(AppMsg::ShowToast(msg.clone()));
                            }
                        });
                        self.action_group.add_action(&action);
                        ("win.delete-selection".to_string(), "delete-selection")
                    }
                    "builtin::new_folder" => {
                        let action = gio::SimpleAction::new("new-folder", None);
                        let s = sender.clone();
                        let toast = action_toast.clone();
                        action.connect_activate(move |_, _| {
                            s.input(AppMsg::PromptNewFolder);
                            if let Some(ref msg) = toast {
                                s.input(AppMsg::ShowToast(msg.clone()));
                            }
                        });
                        self.action_group.add_action(&action);
                        ("win.new-folder".to_string(), "new-folder")
                    }
                    "builtin::new_file" => {
                        let action = gio::SimpleAction::new("new-file", None);
                        let s = sender.clone();
                        let toast = action_toast.clone();
                        action.connect_activate(move |_, _| {
                            s.input(AppMsg::PromptNewFile);
                            if let Some(ref msg) = toast {
                                s.input(AppMsg::ShowToast(msg.clone()));
                            }
                        });
                        self.action_group.add_action(&action);
                        ("win.new-file".to_string(), "new-file")
                    }
                    "builtin::open_with" => {
                        let open_with_menu = gio::Menu::new();
                        let apps = gio::AppInfo::all_for_type(&mime);
                        for app in apps {
                            let label = app.display_name();
                            let app_id = app
                                .id()
                                .map(|id| id.to_string())
                                .unwrap_or_else(|| app.name().to_string());
                            let item = gio::MenuItem::new(Some(&label), Some("win.launch-with"));
                            item.set_action_and_target_value(
                                Some("win.launch-with"),
                                Some(&app_id.to_variant()),
                            );
                            open_with_menu.append_item(&item);
                        }

                        let menu_item = gio::MenuItem::new_submenu(None, &open_with_menu);
                        let spaced_label = "󰱝\u{a0} \u{a0} \u{a0} Open With...".to_string();
                        menu_item.set_label(Some(&spaced_label));

                        open_with_item = Some(menu_item);
                        continue;
                    }
                    _ => (
                        format!("win.{}", action.action_name),
                        action.action_name.as_str(),
                    ),
                };

                // --- ENABLE ACTION IN GROUP ---
                if let Some(g_action) = self.action_group.lookup_action(lookup_name) {
                    if let Some(simple) = g_action.downcast_ref::<gio::SimpleAction>() {
                        simple.set_enabled(true);
                    }
                }

                // Route to Submenu or Main Section
                if let Some(group_name) = &action.submenu {
                    let menu = submenu_map.entry(group_name.clone()).or_default();
                    menu.append(Some(&action.label), Some(&full_action_name));
                } else {
                    main_section.append(Some(&action.label), Some(&full_action_name));
                }
            }
        }

        // Assemble root popover menu
        root_menu.append_section(None, &main_section);
        if let Some(item) = open_with_item {
            root_menu.append_item(&item);
        }

        for (name, menu) in submenu_map {
            root_menu.append_submenu(Some(&name), &menu);
        }

        self.context_menu_popover.set_menu_model(Some(&root_menu));
        self.context_menu_popover
            .set_pointing_to(Some(&gdk::Rectangle::new(x as i32, y as i32, 1, 1)));
        self.context_menu_popover.popup();
    }
}
