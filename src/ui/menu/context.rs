use crate::domain::model::{
    ActionsMsg, AppMsg, AppSidebarMsg, ArchiveMsg, FileOpsMsg, FluxApp, IconsMsg, PropertiesMsg,
    QuickListMsg, SettingsMsg, TabsMsg, TagsMsg, WindowMsg,
};
use crate::ui::constants;
use adw::gdk;
use adw::gio::prelude::*;
use adw::prelude::*;
use gtk::gio;
use relm4::prelude::*;
use std::path::PathBuf;

// ─── Builtin dispatch helpers ────────────────────────────────────────────────

/// Registers a stateless `SimpleAction` on the window action group and returns
/// the fully-qualified action name (`win.<name>`) plus the bare lookup name.
///
/// The closure receives the message sender and the resolved toast message,
/// so every builtin does not have to repeat the same `clone` boilerplate.
fn register_simple<F>(
    group: &gio::SimpleActionGroup,
    name: &'static str,
    sender: &AsyncComponentSender<FluxApp>,
    toast: Option<String>,
    f: F,
) -> (String, &'static str)
where
    F: Fn(&AsyncComponentSender<FluxApp>, Option<&str>) + 'static,
{
    let action = gio::SimpleAction::new(name, None);
    let s = sender.clone();
    action.connect_activate(move |_, _| {
        f(&s, toast.as_deref());
    });
    group.add_action(&action);
    (format!("win.{name}"), name)
}

/// Registers a `SimpleAction` that carries a single `PathBuf` target, plus an
/// optional toast. Covers the majority of the builtins in the DSL.
fn register_path_action<F>(
    group: &gio::SimpleActionGroup,
    name: &'static str,
    sender: &AsyncComponentSender<FluxApp>,
    target: PathBuf,
    toast: Option<String>,
    f: F,
) -> (String, &'static str)
where
    F: Fn(&AsyncComponentSender<FluxApp>, PathBuf, Option<&str>) + 'static,
{
    let action = gio::SimpleAction::new(name, None);
    let s = sender.clone();
    action.connect_activate(move |_, _| {
        f(&s, target.clone(), toast.as_deref());
    });
    group.add_action(&action);
    (format!("win.{name}"), name)
}

/// Convenience: send a toast through the sender if the DSL entry supplied one.
#[inline]
fn emit_toast(sender: &AsyncComponentSender<FluxApp>, toast: Option<&str>) {
    if let Some(msg) = toast {
        sender.input(AppMsg::Window(WindowMsg::ShowToast(msg.to_string())));
    }
}

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
        crate::hit!("context_menu::build_and_show");
        let resolved_path = path
            .clone()
            .or_else(|| self.get_selected_path())
            .or_else(|| self.active_item_path.clone())
            .or_else(|| {
                self.tabs.get(self.active_tab_index).and_then(|t| {
                    let active_files = &t.files;
                    if !active_files.is_empty() {
                        active_files.get(0).map(|w| w.borrow().path.clone())
                    } else {
                        None
                    }
                })
            })
            .or_else(|| Some(self.current_path.clone()));

        self.active_item_path = resolved_path.clone();
        self.active_item_line = resolved_path
            .as_ref()
            .and_then(|p| {
                let active_files = self.tabs.get(self.active_tab_index).map(|t| &t.files)?;
                (0..active_files.len()).find_map(|i| {
                    active_files.get(i).and_then(|w| {
                        let item = w.borrow();
                        if item.path == *p {
                            Some(item.line_number)
                        } else {
                            None
                        }
                    })
                })
            })
            .unwrap_or(0);

        let is_in_trash = self
            .current_path
            .to_string_lossy()
            .starts_with(constants::TRASH_URI);
        let root_menu = gio::Menu::new();
        let main_section = gio::Menu::new();

        let mut submenu_map: indexmap::IndexMap<String, gio::Menu> = indexmap::IndexMap::new();
        let mut quick_list_transfer_item: Option<gio::MenuItem> = None;

        // Clone the action group so the per-item closures below can register
        // into it while the loop body still holds a borrow of `self`.
        let action_group = self.action_group.clone();

        for action in &self.menu_actions {
            let mut matches = false;

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

            if matches {
                if action.command.contains("%l") && self.active_item_line == 0 {
                    continue;
                }
                // Capture toast before the match so builtin connect_activate closures can
                // emit ShowToast directly.
                let action_toast = action.toast.clone();

                if action.command.as_str() == "builtin::quick_list_transfer" {
                    if self.quick.list.is_empty() {
                        continue;
                    }

                    let transfer_menu = gio::Menu::new();
                    let move_menu = gio::Menu::new();
                    let copy_menu = gio::Menu::new();

                    for (idx, target_path) in self.quick.list.iter().enumerate() {
                        let raw_name = target_path
                            .file_name()
                            .map(|n| n.to_string_lossy().to_string())
                            .unwrap_or_else(|| target_path.to_string_lossy().to_string());

                        // Truncate to a max visible width to avoid ballooning the popup menu
                        let max_chars = 26;
                        let char_count = raw_name.chars().count();
                        let label = if char_count > max_chars {
                            let truncated: String = raw_name.chars().take(max_chars - 1).collect();
                            format!("{}…", truncated)
                        } else {
                            raw_name
                        };

                        let target_str = format!("{}:{}", idx, target_path.display());

                        let move_item =
                            gio::MenuItem::new(Some(&label), Some("win.quick-transfer-move"));
                        move_item.set_action_and_target_value(
                            Some("win.quick-transfer-move"),
                            Some(&target_str.to_variant()),
                        );
                        move_menu.append_item(&move_item);

                        let copy_item =
                            gio::MenuItem::new(Some(&label), Some("win.quick-transfer-copy"));
                        copy_item.set_action_and_target_value(
                            Some("win.quick-transfer-copy"),
                            Some(&target_str.to_variant()),
                        );
                        copy_menu.append_item(&copy_item);
                    }

                    transfer_menu.append_submenu(Some(&crate::i18n::tr("Move to")), &move_menu);
                    transfer_menu.append_submenu(Some(&crate::i18n::tr("Copy to")), &copy_menu);

                    let menu_label = format!("󰪶   {}", crate::i18n::tr("Send to Quick List"));
                    quick_list_transfer_item = Some(gio::MenuItem::new_submenu(
                        Some(&menu_label),
                        &transfer_menu,
                    ));
                    continue;
                }

                if action.command.as_str() == "builtin::open_with" {
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

                    let menu_item =
                        gio::MenuItem::new_submenu(Some(&action.label), &open_with_menu);

                    if let Some(group_name) = &action.submenu {
                        let menu = submenu_map.entry(group_name.clone()).or_default();
                        menu.append_item(&menu_item);
                    } else {
                        main_section.append_item(&menu_item);
                    }
                    continue;
                }

                // Resolve the target path once, reused by all path-bearing builtins.
                let target_path = resolved_path
                    .clone()
                    .unwrap_or_else(|| self.current_path.clone());

                let reg = match action.command.as_str() {
                    // Static action names, no per-invocation target
                    "builtin::copy" => ("win.copy".to_string(), "copy"),
                    "builtin::cut" => ("win.cut".to_string(), "cut"),
                    "builtin::paste" => ("win.paste".to_string(), "paste"),

                    // Selection-based, structurally different
                    "builtin::open_new_tab" => {
                        let selected_dirs: Vec<PathBuf> = self
                            .get_selection_with_meta()
                            .into_iter()
                            .filter_map(|(p, is_dir)| if is_dir { Some(p) } else { None })
                            .collect();

                        let targets: Vec<PathBuf> = if !selected_dirs.is_empty() {
                            selected_dirs
                        } else if let Some(ref p) = resolved_path {
                            vec![p.clone()]
                        } else {
                            vec![self.current_path.clone()]
                        };

                        let selection_model = self
                            .tabs
                            .get(self.active_tab_index)
                            .and_then(|t| t.files.view.model())
                            .and_then(|m| m.downcast::<gtk::MultiSelection>().ok());

                        register_simple(
                            &action_group,
                            "open-new-tab",
                            sender,
                            action_toast.clone(),
                            move |s, toast| {
                                if let Some(ref model) = selection_model {
                                    model.unselect_all();
                                }
                                s.input(AppMsg::Tabs(TabsMsg::OpenTabs(targets.clone())));
                                emit_toast(s, toast);
                            },
                        )
                    }

                    // Path-bearing builtins, uniform shape
                    "builtin::rename" => register_path_action(
                        &action_group,
                        "rename-item",
                        sender,
                        target_path,
                        action_toast.clone(),
                        |s, target, toast| {
                            s.input(AppMsg::FileOps(FileOpsMsg::StartRename(target)));
                            emit_toast(s, toast);
                        },
                    ),
                    "builtin::set_bg_window" => register_path_action(
                        &action_group,
                        "set-bg-window",
                        sender,
                        target_path,
                        action_toast.clone(),
                        |s, target, toast| {
                            s.input(AppMsg::Settings(SettingsMsg::SetFluxBackground {
                                target,
                                slot: crate::domain::model::BackgroundSlot::Window,
                            }));
                            emit_toast(s, toast);
                        },
                    ),
                    "builtin::set_bg_sidebar_left" => register_path_action(
                        &action_group,
                        "set-bg-sidebar-left",
                        sender,
                        target_path,
                        action_toast.clone(),
                        |s, target, toast| {
                            s.input(AppMsg::Settings(SettingsMsg::SetFluxBackground {
                                target,
                                slot: crate::domain::model::BackgroundSlot::SidebarLeft,
                            }));
                            emit_toast(s, toast);
                        },
                    ),
                    "builtin::set_bg_sidebar_right" => register_path_action(
                        &action_group,
                        "set-bg-sidebar-right",
                        sender,
                        target_path,
                        action_toast.clone(),
                        |s, target, toast| {
                            s.input(AppMsg::Settings(SettingsMsg::SetFluxBackground {
                                target,
                                slot: crate::domain::model::BackgroundSlot::SidebarRight,
                            }));
                            emit_toast(s, toast);
                        },
                    ),
                    "builtin::select_folder_icon" => register_path_action(
                        &action_group,
                        "select-folder-icon",
                        sender,
                        target_path,
                        action_toast.clone(),
                        |s, target, toast| {
                            s.input(AppMsg::Icons(IconsMsg::ShowIconPicker(target)));
                            emit_toast(s, toast);
                        },
                    ),
                    "builtin::inspect_dir" => register_path_action(
                        &action_group,
                        "inspect-dir",
                        sender,
                        target_path,
                        action_toast.clone(),
                        |s, target, _| {
                            s.input(AppMsg::Properties(PropertiesMsg::InspectDirectory(target)))
                        },
                    ),
                    "builtin::open_with_dialog" => register_path_action(
                        &action_group,
                        "open-with-dialog",
                        sender,
                        target_path,
                        action_toast.clone(),
                        |s, target, toast| {
                            s.input(AppMsg::Actions(ActionsMsg::ShowOpenWithDialog(target)));
                            emit_toast(s, toast);
                        },
                    ),
                    "builtin::reset_custom_icon" => register_path_action(
                        &action_group,
                        "reset-custom-icon",
                        sender,
                        target_path,
                        action_toast.clone(),
                        |s, target, toast| {
                            s.input(AppMsg::Icons(IconsMsg::ResetFileIcon(target)));
                            emit_toast(s, toast);
                        },
                    ),
                    "builtin::set_custom_icon" => register_path_action(
                        &action_group,
                        "set-custom-icon",
                        sender,
                        target_path,
                        action_toast.clone(),
                        |s, target, toast| {
                            FluxApp::show_custom_icon_file_chooser(
                                target,
                                toast.map(str::to_owned),
                                s.clone(),
                            );
                        },
                    ),

                    // Stateless, no target, optional toast
                    "builtin::clear_backgrounds" => register_simple(
                        &action_group,
                        "clear-backgrounds",
                        sender,
                        action_toast.clone(),
                        |s, toast| {
                            s.input(AppMsg::Settings(SettingsMsg::ClearFluxBackgrounds));
                            emit_toast(s, toast);
                        },
                    ),
                    "builtin::forget_archive_password" => register_path_action(
                        &action_group,
                        "forget-archive-password",
                        sender,
                        target_path,
                        action_toast.clone(),
                        |s, target, _| {
                            s.input(AppMsg::Archive(ArchiveMsg::ForgetArchivePassword(target)))
                        },
                    ),
                    "builtin::add_to_quick_list" => register_simple(
                        &action_group,
                        "add-to-quick-list",
                        sender,
                        action_toast.clone(),
                        |s, toast| {
                            s.input(AppMsg::QuickList(QuickListMsg::AddExclusive(None)));
                            emit_toast(s, toast);
                        },
                    ),
                    "builtin::toggle_pin" => register_simple(
                        &action_group,
                        "toggle-pin",
                        sender,
                        action_toast.clone(),
                        |s, toast| {
                            s.input(AppMsg::Sidebar(AppSidebarMsg::AddToSidebarPermanent));
                            emit_toast(s, toast);
                        },
                    ),
                    // No target: the panel follows the grid selection, so the
                    // command needs no path.
                    "builtin::toggle_properties_panel" => register_simple(
                        &action_group,
                        "toggle-properties-panel",
                        sender,
                        action_toast.clone(),
                        |s, toast| {
                            s.input(AppMsg::Properties(PropertiesMsg::ToggleFilePropertiesPanel));
                            emit_toast(s, toast);
                        },
                    ),
                    "builtin::delete" => register_simple(
                        &action_group,
                        "delete-selection",
                        sender,
                        action_toast.clone(),
                        |s, toast| {
                            s.input(AppMsg::FileOps(FileOpsMsg::Delete));
                            emit_toast(s, toast);
                        },
                    ),
                    "builtin::new_folder" => register_simple(
                        &action_group,
                        "new-folder",
                        sender,
                        action_toast.clone(),
                        |s, toast| {
                            s.input(AppMsg::FileOps(FileOpsMsg::PromptNewFolder));
                            emit_toast(s, toast);
                        },
                    ),
                    "builtin::new_file" => register_simple(
                        &action_group,
                        "new-file",
                        sender,
                        action_toast.clone(),
                        |s, toast| {
                            s.input(AppMsg::FileOps(FileOpsMsg::PromptNewFile));
                            emit_toast(s, toast);
                        },
                    ),

                    // Extension-scoped: skip when target has no extension
                    "builtin::set_extension_icon" => {
                        let Some(ext) = resolved_path.as_ref().and_then(|p| {
                            p.extension().and_then(|e| e.to_str()).map(str::to_owned)
                        }) else {
                            continue;
                        };
                        register_simple(
                            &action_group,
                            "set-extension-icon",
                            sender,
                            action_toast.clone(),
                            move |s, toast| {
                                FluxApp::show_extension_icon_file_chooser(
                                    ext.clone(),
                                    toast.map(str::to_owned),
                                    s.clone(),
                                );
                            },
                        )
                    }
                    "builtin::reset_extension_icon" => {
                        let Some(ext) = resolved_path.as_ref().and_then(|p| {
                            p.extension().and_then(|e| e.to_str()).map(str::to_owned)
                        }) else {
                            continue;
                        };
                        register_simple(
                            &action_group,
                            "reset-extension-icon",
                            sender,
                            action_toast.clone(),
                            move |s, toast| {
                                s.input(AppMsg::Icons(IconsMsg::ResetExtensionIcon(ext.clone())));
                                emit_toast(s, toast);
                            },
                        )
                    }

                    // Font install: every selected font, else the clicked one
                    "builtin::install_fonts" => {
                        let mut targets: Vec<PathBuf> = self
                            .get_selection()
                            .into_iter()
                            .filter(|p| crate::infra::fonts::is_font_path(p))
                            .collect();
                        if targets.is_empty() {
                            if let Some(p) = resolved_path
                                .as_ref()
                                .filter(|p| crate::infra::fonts::is_font_path(p))
                            {
                                targets.push(p.clone());
                            }
                        }
                        if targets.is_empty() {
                            continue;
                        }
                        register_simple(
                            &action_group,
                            "install-fonts",
                            sender,
                            action_toast.clone(),
                            move |s, _| {
                                s.input(AppMsg::FileOps(FileOpsMsg::InstallFonts(targets.clone())))
                            },
                        )
                    }

                    // File-only builtins
                    "builtin::tagfile" => {
                        if mime == constants::MIME_DIR {
                            continue;
                        }
                        register_simple(
                            &action_group,
                            "tag-file",
                            sender,
                            action_toast.clone(),
                            |s, toast| {
                                s.input(AppMsg::Tags(TagsMsg::OpenTagPicker));
                                emit_toast(s, toast);
                            },
                        )
                    }

                    // User-defined shell action from menu.rs
                    _ => (
                        format!("win.{}", action.action_name),
                        action.action_name.as_str(),
                    ),
                };

                let (full_action_name, _lookup_name) = reg;

                if let Some(group_name) = &action.submenu {
                    let menu = submenu_map.entry(group_name.clone()).or_default();
                    menu.append(Some(&action.label), Some(&full_action_name));
                } else {
                    main_section.append(Some(&action.label), Some(&full_action_name));
                }
            }
        }

        // Assemble root popover menu: main items first, then named submenus
        root_menu.append_section(None, &main_section);

        for (name, menu) in submenu_map {
            let item = gio::MenuItem::new_submenu(Some(&name), &menu);
            root_menu.append_item(&item);
        }

        // Place Quick List Transfer at the very bottom
        if let Some(item) = quick_list_transfer_item {
            root_menu.append_item(&item);
        }

        self.context_menu_popover.set_menu_model(Some(&root_menu));
        self.context_menu_popover
            .set_pointing_to(Some(&gdk::Rectangle::new(x as i32, y as i32, 1, 1)));
        self.context_menu_popover.popup();
    }
}
