use crate::i18n::tr;
use crate::model::FluxApp;
use crate::ui::constants;
use crate::ui::constants::MOUSE_RIGHT_CLICK;
use crate::utils;
use adw::gdk;
use adw::prelude::*;
use gtk::gio;
use gtk::glib;
use relm4::prelude::*;
use std::path::PathBuf;

/// Output events emitted by a sidebar row.
#[derive(Debug)]
pub enum SidebarMsg {
    Navigate(PathBuf),
    Remove(PathBuf),
    ChangeIcon(PathBuf),
    OpenInNewTab(PathBuf),
    OpenInNewWindow(PathBuf),
    Rename {
        path: PathBuf,
        current_name: String,
    },
    Reorder {
        from: PathBuf,
        to: PathBuf,
    },
    /// A folder was dragged from the grid and dropped onto the row at `before`.
    ///
    /// `path` is the folder being pinned, `before` is the path of the row it
    /// was dropped on, used by the update handler to determine insertion index.
    PinAt {
        path: PathBuf,
        before: PathBuf,
        label_name: Option<String>,
    },
    /// Files were dragged from the grid and dropped onto a sidebar folder row.
    DropMove {
        source_paths: Vec<PathBuf>,
        dest_path: PathBuf,
    },
    // ── Section-label actions ────────────────────────────────────────────────
    /// User requested to rename a section label.
    RenameSection(String),
    /// User chose "Remove Section" from the label context menu.
    RemoveSection(String),
}

/// Simple model for a pinned sidebar location.
#[derive(Debug)]
pub struct SidebarPlace {
    pub name: String,
    pub icon: String,
    pub path: PathBuf,
    pub is_mount: bool,
    /// When true, renders as a non-interactive section header instead of a navigation row.
    pub is_section_label: bool,
}

#[relm4::factory(pub)]
impl FactoryComponent for SidebarPlace {
    type Init = SidebarPlace;
    type Input = ();
    type Output = SidebarMsg;
    type ParentWidget = gtk::ListBox;
    type CommandOutput = ();

    view! {
        #[root]
        gtk::ListBoxRow {
            #[watch]
            set_selectable: !self.is_section_label,
            #[watch]
            set_activatable: !self.is_section_label,
            #[watch] set_class_active: (constants::SIDEBAR_SECTION_ROW_CLASS, self.is_section_label),
            #[watch] set_class_active: (constants::SIDEBAR_ROW_CLASS, !self.is_section_label),
            #[watch] set_class_active: ("sidebar-mount", self.is_mount),
            connect_realize[is_label = self.is_section_label] => move |w| {
                FluxApp::set_cursor_pointer(w.as_ref(), !is_label);
            },

            add_controller = gtk::GestureClick {
                connect_released[sender, path = self.path.clone(), is_label = self.is_section_label] => move |gesture, _, _, _| {
                    if !is_label && gesture.current_button() == 1 {
                        if let Some(row) = gesture.widget().and_downcast::<gtk::ListBoxRow>() {
                            if let Some(lb) = row.parent().and_downcast::<gtk::ListBox>() {
                                lb.select_row(Some(&row));
                            }
                        }
                        let _ = sender.output(SidebarMsg::Navigate(path.clone()));
                    }
                }
            },

           add_controller = gtk::GestureClick {
                set_button: 2,
                connect_pressed[sender, path = self.path.clone(), is_label = self.is_section_label] => move |gesture, _, _, _| {
                    if is_label { return; }
                    gesture.set_state(gtk::EventSequenceState::Claimed);

                    let target = path.clone();
                    let modifiers = gesture.current_event_state();
                    if modifiers.contains(adw::gdk::ModifierType::CONTROL_MASK) {
                        let _ = sender.output(SidebarMsg::OpenInNewWindow(target));
                    } else {
                        let _ = sender.output(SidebarMsg::OpenInNewTab(target));
                    }
                }
            },

            add_controller = gtk::GestureClick {
                set_button: MOUSE_RIGHT_CLICK,
                connect_pressed[sender, path = self.path.clone(), name = self.name.clone(), is_label = self.is_section_label] => move |gesture, _, x, y| {
                    gesture.set_state(gtk::EventSequenceState::Claimed);

                    let menu = gtk::PopoverMenu::builder()
                        .has_arrow(false)
                        .build();

                    let menu_model = gio::Menu::new();
                    let action_group = gio::SimpleActionGroup::new();

                    if is_label {
                        menu_model.append(Some(&tr("Rename")), Some("sidebar.rename_section"));
                        menu_model.append(Some(&tr("Remove section")), Some("sidebar.remove_section"));

                        let rename_action = gio::SimpleAction::new("rename_section", None);
                        let sender_rn = sender.clone();
                        let name_rn = name.clone();
                        rename_action.connect_activate(move |_, _| {
                            let _ = sender_rn.output(SidebarMsg::RenameSection(name_rn.clone()));
                        });
                        action_group.add_action(&rename_action);

                        let remove_action = gio::SimpleAction::new("remove_section", None);
                        let sender_rm = sender.clone();
                        let name_rm = name.clone();
                        remove_action.connect_activate(move |_, _| {
                            let _ = sender_rm.output(SidebarMsg::RemoveSection(name_rm.clone()));
                        });
                        action_group.add_action(&remove_action);
                    } else {
                        menu_model.append(Some(&tr("Open in New Tab")), Some("sidebar.open_new_tab"));
                        menu_model.append(Some(&tr("Rename")), Some("sidebar.rename"));
                        menu_model.append(Some(&tr("Change icon")), Some("sidebar.change_icon"));
                        menu_model.append(Some(&tr("Remove from sidebar")), Some("sidebar.remove"));

                        let open_tab_action = gio::SimpleAction::new("open_new_tab", None);
                        let sender_ot = sender.clone();
                        let path_ot = path.clone();
                        open_tab_action.connect_activate(move |_, _| {
                            let _ = sender_ot.output(SidebarMsg::OpenInNewTab(path_ot.clone()));
                        });
                        action_group.add_action(&open_tab_action);

                        let rename_action = gio::SimpleAction::new("rename", None);
                        let sender_rn = sender.clone();
                        let path_rn = path.clone();
                        let name_rn = name.clone();
                        rename_action.connect_activate(move |_, _| {
                            let _ = sender_rn.output(SidebarMsg::Rename {
                                path: path_rn.clone(),
                                current_name: name_rn.clone(),
                            });
                        });
                        action_group.add_action(&rename_action);

                        let change_icon_action = gio::SimpleAction::new("change_icon", None);
                        let sender_ci = sender.clone();
                        let path_ci = path.clone();
                        change_icon_action.connect_activate(move |_, _| {
                            let _ = sender_ci.output(SidebarMsg::ChangeIcon(path_ci.clone()));
                        });
                        action_group.add_action(&change_icon_action);

                        let remove_action = gio::SimpleAction::new("remove", None);
                        let sender_c = sender.clone();
                        let path_c = path.clone();
                        remove_action.connect_activate(move |_, _| {
                            let _ = sender_c.output(SidebarMsg::Remove(path_c.clone()));
                        });
                        action_group.add_action(&remove_action);
                    }

                    menu.set_menu_model(Some(&menu_model));

                    if let Some(widget) = gesture.widget() {
                        menu.set_parent(&widget);

                        let rect = gdk::Rectangle::new(x as i32, y as i32, 1, 1);
                        menu.set_pointing_to(Some(&rect));

                        widget.insert_action_group("sidebar", Some(&action_group));
                        menu.popup();
                    }
                }
            },

            // Drag source: carry this row's path or label identifier as a plain string
            add_controller = gtk::DragSource {
                set_actions: if utils::load_config().ui.disable_drag_and_drop { gdk::DragAction::empty() } else { gdk::DragAction::MOVE },
                connect_prepare[path = self.path.clone(), name = self.name.clone(), is_label = self.is_section_label] => move |src, _, _| {
                    if utils::load_config().ui.disable_drag_and_drop {
                        return None;
                    }
                    if let Some(w) = src.widget() {
                        w.add_css_class("sidebar-dragging");
                    }
                    let payload = if is_label {
                        format!("label:{}", name)
                    } else {
                        path.to_string_lossy().to_string()
                    };
                    Some(gdk::ContentProvider::for_value(&payload.to_value()))
                },
                connect_drag_end => |src, _, _| {
                    if let Some(w) = src.widget() {
                        w.remove_css_class("sidebar-dragging");
                    }
                },
            },

            // Drop target: accept a path string or label dropped from another sidebar row
            add_controller = gtk::DropTarget {
                set_actions: if utils::load_config().ui.disable_drag_and_drop { gdk::DragAction::empty() } else { gdk::DragAction::MOVE },
                set_types: &[glib::types::Type::STRING],
                connect_drop[sender, path = self.path.clone(), name = self.name.clone(), is_label = self.is_section_label] => move |_, value, _, _| {
                    if utils::load_config().ui.disable_drag_and_drop {
                        return false;
                    }
                    if let Ok(from_str) = value.get::<String>() {
                        let to = if is_label {
                            PathBuf::from(format!("label:{}", name))
                        } else {
                            path.clone()
                        };
                        let from = PathBuf::from(&from_str);
                        if from != to {
                            let _ = sender.output(SidebarMsg::Reorder {
                                from,
                                to,
                            });
                        }
                    }
                    true
                },
            },

            // Drop target: accept any files/folders dragged from the file grid.
            add_controller = gtk::DropTarget {
                set_actions: if utils::load_config().ui.disable_drag_and_drop { gdk::DragAction::empty() } else { gdk::DragAction::COPY | gdk::DragAction::MOVE },
                set_types: &[gdk::FileList::static_type()],

                connect_enter[path = self.path.clone(), is_label = self.is_section_label, hover_timer = std::rc::Rc::<std::cell::Cell::<Option<glib::SourceId>>>::default()] => move |target, _, _| {
                    if utils::load_config().ui.disable_drag_and_drop || is_label || !path.is_dir() {
                        return gdk::DragAction::empty();
                    }

                    if let Some(widget) = target.widget() {
                        widget.add_css_class("sidebar-drop-hover");
                    }

                    // Auto-navigate after 750 ms while the drag idles over the row.
                    let nav_path = path.clone();
                    let timer_ref = hover_timer.clone();
                    let id = gtk::glib::timeout_add_local_once(
                        std::time::Duration::from_millis(750),
                        move || {
                            timer_ref.set(None);
                            if let Some(s) = crate::model::SENDER.get() {
                                let _ = s.send(crate::model::AppMsg::Navigate(nav_path.clone()));
                            }
                        },
                    );
                    hover_timer.set(Some(id));

                    gdk::DragAction::MOVE
                },

                connect_leave[is_label = self.is_section_label, hover_timer = std::rc::Rc::<std::cell::Cell::<Option<glib::SourceId>>>::default()] => move |target| {
                    if is_label { return; }

                    if let Some(widget) = target.widget() {
                        widget.remove_css_class("sidebar-drop-hover");
                    }

                    if let Some(id) = hover_timer.take() {
                        id.remove();
                    }
                },

                connect_drop[sender, path = self.path.clone(), name = self.name.clone(), is_label = self.is_section_label, hover_timer = std::rc::Rc::<std::cell::Cell::<Option<glib::SourceId>>>::default()] => move |target, value, _, _| {
                    if utils::load_config().ui.disable_drag_and_drop {
                        return false;
                    }
                    // Cancel any pending auto-navigate on release.
                    if let Some(id) = hover_timer.take() {
                        id.remove();
                    }
                    if let Some(widget) = target.widget() {
                        widget.remove_css_class("sidebar-drop-hover");
                    }

                    if let Ok(file_list) = value.get::<gdk::FileList>() {
                        let paths: Vec<PathBuf> = file_list
                            .files()
                            .into_iter()
                            .filter_map(|f| f.path())
                            .collect();

                        let all_dirs = paths.iter().all(|p| p.is_dir());

                        if all_dirs && !is_label {
                            // Pin every dragged directory into the sidebar before this row.
                            for folder in paths {
                                let _ = sender.output(SidebarMsg::PinAt {
                                    path: folder,
                                    before: path.clone(),
                                    label_name: if is_label { Some(name.clone()) } else { None },
                                });
                            }
                        } else if path.is_dir() {
                            // Move mixed or file-only payloads into the destination folder.
                            let _ = sender.output(SidebarMsg::DropMove {
                                source_paths: paths,
                                dest_path: path.clone(),
                            });
                        }

                        return true;
                    }
                    false
                },
            },

            gtk::Box {
                set_orientation: gtk::Orientation::Horizontal,

                gtk::Label {
                    #[watch]
                    set_visible: self.is_section_label,
                    #[watch]
                    set_label: &self.name,
                    set_halign: gtk::Align::Start,
                    add_css_class: constants::SIDEBAR_SECTION_LABEL_CLASS,
                },

                gtk::Box {
                    #[watch]
                    set_visible: !self.is_section_label,
                    set_orientation: gtk::Orientation::Horizontal,
                    set_spacing: constants::SIDEBAR_SPACING,
                    set_hexpand: true,
                    gtk::Image {
                        #[watch]
                        set_icon_name: Some(&self.icon),
                        set_icon_size: gtk::IconSize::Inherit,
                    },
                    gtk::Label {
                        #[watch]
                        set_label: &self.name,
                        add_css_class: constants::SIDEBAR_LABEL_CLASS,
                        set_hexpand: true,
                        set_halign: gtk::Align::Start,
                    },
                    #[name = "eject_button"]
                    gtk::Button {
                        #[wrap(Some)]
                        set_child = &gtk::Image {
                            set_icon_name: Some("media-eject"),
                            set_icon_size: gtk::IconSize::Inherit,
                        },
                        #[watch]
                        set_visible: self.is_mount,
                        add_css_class: "eject-button",
                        connect_clicked[path = self.path.clone()] => move |_| {
                            if let Some(s) = crate::model::SENDER.get() {
                                let _ = s.send(crate::model::AppMsg::UnmountDevice(path.clone()));
                            }
                        }
                    }
                },
            }
        }
    }

    fn init_model(init: Self::Init, _: &DynamicIndex, _: FactorySender<Self>) -> Self {
        init
    }
}
