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
                        "image/all" | "image/*" => mime.starts_with("image/"),
                        "video/all" | "video/*" => mime.starts_with("video/"),
                        "audio/all" | "audio/*" => mime.starts_with("audio/"),
                        "font/all" | "font/*" => mime.starts_with("font/"),
                        "model/all" | "model/*" => mime.starts_with("model/"),
                        "message/all" | "message/*" => mime.starts_with("message/"),
                        "chemical/all" | "chemical/*" => mime.starts_with("chemical/"),
                        "multipart/all" | "multipart/*" => mime.starts_with("multipart/"),
                        "x-content/all" | "x-content/*" => mime.starts_with("x-content/"),
                        "application/all" | "application/*" => mime.starts_with("application/"),
                        "text/all" | "text/*" => {
                            mime.starts_with("text/")
                                || gio::content_type_is_a(&mime, constants::MIME_TEXT)
                                || mime == constants::MIME_EMPTY
                        }
                        constants::FILTER_FOLDER | "directory" => mime == constants::MIME_DIR,
                        constants::FILTER_FILE => mime != constants::MIME_DIR,
                        t if t.ends_with('/') => mime.starts_with(t),
                        t => t == mime,
                    });
                    if matches {
                        break;
                    }
                }
            }

            // --- MENU ASSEMBLY & BUILTIN MAPPING ---
            if matches {
                let (full_action_name, lookup_name) = match action.command.as_str() {
                    "builtin::copy" => ("win.copy".to_string(), "copy"),
                    "builtin::cut" => ("win.cut".to_string(), "cut"),
                    "builtin::paste" => ("win.paste".to_string(), "paste"),
                    "builtin::add_to_quick_list" => {
                        if let Some(ref target) = path {
                            let quick_action = gio::SimpleAction::new("add-to-quick-list", None);
                            let target_clone = target.clone();
                            let sender_q = sender.clone();
                            quick_action.connect_activate(move |_, _| {
                                sender_q.input(AppMsg::AddExclusive(Some(target_clone.clone())));
                            });
                            self.action_group.add_action(&quick_action);
                        }
                        ("win.add-to-quick-list".to_string(), "add-to-quick-list")
                    }
                    "builtin::delete" => {
                        let action = gio::SimpleAction::new("delete-selection", None);
                        let s = sender.clone();
                        action.connect_activate(move |_, _| {
                            s.input(AppMsg::Delete);
                        });
                        self.action_group.add_action(&action);
                        ("win.delete-selection".to_string(), "delete-selection")
                    }
                    "builtin::new_folder" => {
                        let action = gio::SimpleAction::new("new-folder", None);
                        let s = sender.clone();
                        action.connect_activate(move |_, _| {
                            s.input(AppMsg::PromptNewFolder);
                        });
                        self.action_group.add_action(&action);
                        ("win.new-folder".to_string(), "new-folder")
                    }
                    "builtin::new_file" => {
                        let action = gio::SimpleAction::new("new-file", None);
                        let s = sender.clone();
                        action.connect_activate(move |_, _| {
                            s.input(AppMsg::PromptNewFile);
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
                        if let Some(toast_msg) = &action.toast {
                            self.pending_toasts
                                .insert(action.action_name.clone(), toast_msg.clone());
                        }
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
