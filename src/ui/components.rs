use crate::model::FluxApp;
use crate::model::PathSegment;
use crate::ui::constants;
use crate::ui::constants::MOUSE_RIGHT_CLICK;
use crate::utils;
use crate::utils::PathExt;
use adw::gdk;
use adw::prelude::*;
use gtk::gio;
use gtk::glib;
use relm4::prelude::*;
use std::path::PathBuf;

/// Data model representing a file or directory entry in the application grid.
#[derive(Debug, Clone)]
pub struct FileItem {
    pub name: String,
    pub icon: adw::gio::Icon,
    pub size: u64,
    pub thumbnail: Option<gdk::Texture>,
    #[allow(dead_code)]
    pub is_dir: bool,
    pub path: PathBuf,
    pub icon_size: i32,
    pub is_editing: bool,
    pub is_foreign_owner: bool,
    /// Whether the label should wrap to multiple lines instead of ellipsizing.
    pub expand_labels: bool,
}

/// Collection of GTK widgets utilized by a [FileItem] within the grid view.
pub struct FileWidgets {
    pub icon_widget: gtk::Image,
    pub lock_icon: gtk::Image,
    pub label: gtk::Label,
    pub entry: gtk::Entry,
    pub stack: gtk::Stack,
    pub drag_source: gtk::DragSource,
    pub drop_target: gtk::DropTarget,
}

impl relm4::typed_view::grid::RelmGridItem for FileItem {
    type Root = gtk::Box;
    type Widgets = FileWidgets;

    /// Initializes the widget hierarchy and event controllers for a grid item.
    ///
    /// Sets up drag-and-drop functionality and mouse gesture listeners for
    /// context menu interaction.
    fn setup(_item: &gtk::ListItem) -> (Self::Root, Self::Widgets) {
        let drag_source = gtk::DragSource::builder()
            .actions(gdk::DragAction::COPY | gdk::DragAction::MOVE)
            .build();
        let config = utils::load_config();

        drag_source.connect_drag_begin(|src, _| {
            if let Some(widget) = src.widget() {
                let paintable = gtk::WidgetPaintable::new(Some(&widget));
                src.set_icon(Some(&paintable), 0, 0);
            }
        });

        let formats = gdk::ContentFormats::builder()
            .add_type(gdk::FileList::static_type())
            .add_type(gtk::gio::File::static_type())
            .build();

        let drop_target = gtk::DropTarget::builder()
            .formats(&formats)
            .actions(gdk::DragAction::COPY | gdk::DragAction::MOVE)
            .build();

        drop_target.connect_drop(|target, value, _, _| {
            let widget = target.widget().unwrap();
            let sender = crate::model::SENDER.get();

            let dest_path_opt: Option<PathBuf> = unsafe {
                widget
                    .data::<PathBuf>("file_path")
                    .map(|p| p.as_ref().clone())
            };

            let mut source_paths = Vec::new();

            if let Ok(file_list) = value.get::<gdk::FileList>() {
                source_paths = file_list
                    .files()
                    .into_iter()
                    .filter_map(|f| f.path())
                    .collect();
            } else if let Ok(file) = value.get::<gtk::gio::File>() {
                if let Some(path) = file.path() {
                    source_paths.push(path);
                }
            }

            if let (Some(dest), Some(s)) = (dest_path_opt, sender) {
                if !source_paths.is_empty() {
                    s.send(crate::model::AppMsg::HandleDrop {
                        source_paths,
                        dest_path: dest,
                    })
                    .ok();
                    return true;
                }
            }
            false
        });

        relm4::view! {
            #[root]
            root = gtk::Box {
                set_orientation: gtk::Orientation::Vertical,
                set_halign: gtk::Align::Center,
                set_spacing: 0,
                set_valign: gtk::Align::Center,
                add_css_class: constants::CARD_CSS_CLASS,

                // only if single_click is enabled
                connect_realize => move |w| {
                    FluxApp::set_cursor_pointer(w.as_ref(), config.ui.single_click);
                },

                add_controller: drag_source.clone(),
                add_controller: drop_target.clone(),

                add_controller = gtk::GestureLongPress {
                    connect_pressed[sender = crate::model::SENDER.clone()] => move |gesture, x, y| {
                        if let Some(s) = sender.get() {
                            let widget = gesture.widget().unwrap();
                            let path_opt: Option<PathBuf> = unsafe {
                                widget.data::<PathBuf>("file_path").map(|p| p.as_ref().expand_tilde())
                            };

                            if let Some(popover_parent) = widget.ancestor(gtk::GridView::static_type()) {
                                let (rel_x, rel_y) = widget.translate_coordinates(&popover_parent, x, y).unwrap_or((x, y));
                                s.send(crate::model::AppMsg::PrepareContextMenu(rel_x, rel_y, path_opt)).ok();
                            }
                        }
                    }
                },

                add_controller = gtk::GestureClick {
                    set_button: 0,
                    connect_pressed => |gesture, _, _, _| {
                        let button = gesture.current_button();
                        if button == constants::MOUSE_RIGHT_CLICK {
                            gesture.set_state(gtk::EventSequenceState::Claimed);
                        }
                    },
                    connect_released[sender = crate::model::SENDER.clone()] => move |gesture, _, x, y| {
                        if gesture.current_button() == MOUSE_RIGHT_CLICK {
                            if let Some(s) = sender.get() {
                                let widget = gesture.widget().unwrap();

                                // Extract and resolve the path immediately to prevent double-expansion bugs
                                let path_opt: Option<PathBuf> = unsafe {
                                    widget.data::<PathBuf>("file_path").map(|p| {
                                        p.as_ref().expand_tilde()
                                    })
                                };

                                if let Some(popover_parent) = widget.ancestor(gtk::GridView::static_type()) {
                                    let (rel_x, rel_y) = widget.translate_coordinates(&popover_parent, x, y).unwrap_or((x, y));
                                    s.send(crate::model::AppMsg::PrepareContextMenu(rel_x, rel_y, path_opt)).ok();
                                }
                            }
                        }
                    }
                },

                #[name = "icon_widget"]
                gtk::Image {
                    set_halign: gtk::Align::Center,
                    set_valign: gtk::Align::End,
                    set_vexpand: false,
                    add_css_class: constants::THUMBNAIL_CLASS,
                },

                #[name = "stack"]
                gtk::Stack {
                    set_transition_type: gtk::StackTransitionType::Crossfade,
                    set_halign: gtk::Align::Center,
                    set_vexpand: false,

                    add_child = &gtk::Box {
                        set_orientation: gtk::Orientation::Horizontal,
                        set_halign: gtk::Align::Center,
                        set_spacing: 4,

                        #[name = "lock_icon"]
                        gtk::Image {
                            set_icon_name: Some("changes-prevent-symbolic"),
                            set_pixel_size: 12,
                            set_visible: false,
                            add_css_class: "flux-lock-badge",
                        },

                        #[name = "label"]
                        gtk::Label {
                            set_justify: gtk::Justification::Center,
                            set_max_width_chars: config.ui.max_width_chars,
                            set_width_chars: config.ui.grid_spacing,
                            set_ellipsize: gtk::pango::EllipsizeMode::End,
                            set_hexpand: false,
                            add_css_class: constants::FLUX_LABEL_CLASS,
                        },
                    } -> { set_name: constants::VIEW_LABEL },

                    #[name = "entry"]
                    add_child = &gtk::Entry {
                        set_halign: gtk::Align::Center,
                        add_css_class: constants::RENAME_ENTRY_CLASS,

                        // 1. Reliable focus loss detection
                        add_controller = gtk::EventControllerFocus {
                            connect_leave[sender = crate::model::SENDER.clone()] => move |_| {
                                if let Some(s) = sender.get() {
                                    s.send(crate::model::AppMsg::Refresh).ok();
                                }
                            }
                        },

                        // 2. Escape key handling
                        add_controller = gtk::EventControllerKey {
                            connect_key_pressed[sender = crate::model::SENDER.clone()] => move |_, keyval, _, _| {
                                if keyval == gdk::Key::Escape {
                                    if let Some(s) = sender.get() {
                                        s.send(crate::model::AppMsg::Refresh).ok();
                                        return glib::Propagation::Stop;
                                    }
                                }
                                glib::Propagation::Proceed
                            }
                        },

                        // 3. Enter key handling
                        connect_activate[sender = crate::model::SENDER.clone(), root] => move |entry| {
                            if let Some(s) = sender.get() {
                                let old_path_opt: Option<PathBuf> = unsafe {
                                    root.data::<PathBuf>("file_path").map(|p| p.as_ref().clone())
                                };
                                if let Some(old_path) = old_path_opt {
                                    let new_name = entry.text().to_string();
                                    s.send(crate::model::AppMsg::PerformRename(old_path, new_name)).ok();
                                }
                            }
                        },
                    } -> { set_name: constants::VIEW_ENTRY }
                }
            }
        }

        (
            root,
            FileWidgets {
                icon_widget,
                lock_icon,
                label,
                entry,
                stack,
                drag_source,
                drop_target,
            },
        )
    }

    /// Updates the item's widgets with current data from the [FileItem] model.
    ///
    /// Synchronizes labels, icons, thumbnails, and visibility states (e.g., rename entry).
    fn bind(&mut self, widgets: &mut Self::Widgets, root: &mut Self::Root) {
        widgets.label.set_label(&self.name);
        widgets.icon_widget.set_pixel_size(self.icon_size);

        if self.expand_labels {
            widgets.label.set_wrap(true);
            widgets.label.set_wrap_mode(gtk::pango::WrapMode::WordChar);
            widgets.label.set_ellipsize(gtk::pango::EllipsizeMode::None);
        } else {
            widgets.label.set_wrap(false);
            widgets.label.set_ellipsize(gtk::pango::EllipsizeMode::End);
        }

        // Set the widget name to the absolute path so the app.rs controller can find it
        root.set_widget_name(&self.path.to_string_lossy());

        if self.is_foreign_owner {
            root.add_css_class("flux-card--restricted");
            widgets.lock_icon.set_visible(true);
        } else {
            root.remove_css_class("flux-card--restricted");
            widgets.lock_icon.set_visible(false);
        }

        if self.is_editing {
            widgets.stack.set_visible_child_name(constants::VIEW_ENTRY);
            widgets.entry.set_text(&self.name);
            let dot_pos = self.name.rfind('.').unwrap_or(self.name.len());
            widgets.entry.select_region(0, dot_pos as i32);

            // Defer focus grab to next main loop iteration so the widget
            // is fully realized after the stack transition completes.
            let entry = widgets.entry.clone();
            gtk::glib::idle_add_local_once(move || {
                entry.grab_focus();
            });
        }

        if let Some(ref texture) = self.thumbnail {
            widgets.icon_widget.set_paintable(Some(texture));
        } else {
            // Check if the icon exists in the current theme before using it.
            // If the custom icon name doesn't exist (e.g., after theme change),
            // fall back to the default folder icon.
            let display = match adw::gdk::Display::default() {
                Some(d) => d,
                None => {
                    widgets.icon_widget.set_from_gicon(&self.icon);
                    return;
                }
            };
            let icon_theme = gtk::IconTheme::for_display(&display);

            // Try to get the icon name from the GIcon
            if let Some(icon_str) = self.icon.to_string() {
                let icon_str = icon_str.as_str();
                if !icon_str.is_empty() {
                    // Check if the icon exists in the current theme
                    let has_icon = icon_theme.has_icon(icon_str);
                    if has_icon {
                        widgets.icon_widget.set_from_gicon(&self.icon);
                    } else {
                        // Fallback: use the default folder icon
                        let fallback_icon =
                            gio::Icon::for_string("folder").unwrap_or_else(|_| self.icon.clone());
                        widgets.icon_widget.set_from_gicon(&fallback_icon);
                    }
                    return;
                }
            }

            // Fallback: use the icon as-is if we couldn't determine its name
            widgets.icon_widget.set_from_gicon(&self.icon);
        }

        let file = gtk::gio::File::for_path(&self.path);
        let content = gdk::ContentProvider::for_value(&file.to_value());
        widgets.drag_source.set_content(Some(&content));

        widgets.drop_target.set_actions(if self.is_dir {
            gdk::DragAction::COPY | gdk::DragAction::MOVE
        } else {
            gdk::DragAction::empty()
        });

        unsafe {
            root.set_data("file_path", self.path.clone());
        }
    }
}

/// Output events emitted by a sidebar row.
#[derive(Debug)]
pub enum SidebarMsg {
    Navigate(PathBuf),
    Remove(PathBuf),
    Reorder { from: PathBuf, to: PathBuf },
}

/// Simple model for a pinned sidebar location.
#[derive(Debug)]
pub struct SidebarPlace {
    pub name: String,
    pub icon: String,
    pub path: PathBuf,
    pub is_mount: bool,
}

#[relm4::factory(pub)]
impl FactoryComponent for PathSegment {
    type Init = PathSegment;
    type Input = ();
    type Output = PathBuf;
    type ParentWidget = gtk::Box;
    type CommandOutput = ();

    fn init_model(init: Self::Init, _index: &DynamicIndex, _sender: FactorySender<Self>) -> Self {
        init
    }

    view! {
        #[root]
        gtk::Button {
            set_label: &self.name,
            add_css_class: constants::BREADCRUMB_BTN_CLASS,
            #[wrap(Some)]
            set_child = &gtk::Label {
                #[watch]
                set_label: &self.name,
                set_ellipsize: gtk::pango::EllipsizeMode::End,
                set_max_width_chars: constants::BREADCRUMB_MAX_WIDTH_CHARS as i32,
            },
            connect_clicked[sender, path = self.path.clone()] => move |_| {
                let _ = sender.output(path.clone());
            }
        }
    }
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
            add_css_class: constants::SIDEBAR_ROW_CLASS,
            set_selectable: false,
            connect_realize => |w| FluxApp::set_cursor_pointer(w.as_ref(), true),

            add_controller = gtk::GestureClick {
                connect_released[sender, path = self.path.clone()] => move |gesture, _, _, _| {
                    if gesture.current_button() == 1 {
                        let _ = sender.output(SidebarMsg::Navigate(path.clone()));
                    }
                }
            },

            add_controller = gtk::GestureClick {
                set_button: 2,
                connect_pressed[path = self.path.clone()] => move |gesture, _, _, _| {
                    gesture.set_state(gtk::EventSequenceState::Claimed);
                    if let Ok(exe) = std::env::current_exe() {
                        let _ = std::process::Command::new(exe).arg(&path).spawn();
                    }
                }
            },

            add_controller = gtk::GestureClick {
                set_button: MOUSE_RIGHT_CLICK,
                connect_pressed[sender, path = self.path.clone()] => move |gesture, _, x, y| {
                    gesture.set_state(gtk::EventSequenceState::Claimed);

                    let menu = gtk::PopoverMenu::builder()
                        .has_arrow(false)
                        .build();

                    let menu_model = gio::Menu::new();
                    menu_model.append(Some("Remove from sidebar"), Some("sidebar.remove"));
                    menu.set_menu_model(Some(&menu_model));

                    if let Some(widget) = gesture.widget() {
                        menu.set_parent(&widget);

                        let rect = gdk::Rectangle::new(x as i32, y as i32, 1, 1);
                        menu.set_pointing_to(Some(&rect));

                        let action_group = gio::SimpleActionGroup::new();
                        let remove_action = gio::SimpleAction::new("remove", None);
                        let sender_c = sender.clone();
                        let path_c = path.clone();
                        remove_action.connect_activate(move |_, _| {
                            let _ = sender_c.output(SidebarMsg::Remove(path_c.clone()));
                        });
                        action_group.add_action(&remove_action);
                        widget.insert_action_group("sidebar", Some(&action_group));

                        menu.popup();
                    }
                }
            },

            // Drag source: carry this row's path as a plain string
            add_controller = gtk::DragSource {
                set_actions: gdk::DragAction::MOVE,
                connect_prepare[path = self.path.clone()] => move |src, _, _| {
                    if let Some(w) = src.widget() {
                        w.add_css_class("sidebar-dragging");
                    }
                    Some(gdk::ContentProvider::for_value(
                        &path.to_string_lossy().to_string().to_value()
                    ))
                },
                connect_drag_end => |src, _, _| {
                    if let Some(w) = src.widget() {
                        w.remove_css_class("sidebar-dragging");
                    }
                },
            },

            // Drop target: accept a path string dropped from another sidebar row
            add_controller = gtk::DropTarget {
                set_actions: gdk::DragAction::MOVE,
                set_types: &[glib::types::Type::STRING],
                connect_drop[sender, path = self.path.clone()] => move |_, value, _, _| {
                    if let Ok(from_str) = value.get::<String>() {
                        let from = PathBuf::from(&from_str);
                        if from != path {
                            let _ = sender.output(SidebarMsg::Reorder {
                                from,
                                to: path.clone(),
                            });
                        }
                    }
                    true
                },
            },

            gtk::Box {
                set_orientation: gtk::Orientation::Horizontal,
                set_spacing: constants::SIDEBAR_SPACING,
                gtk::Image { set_icon_name: Some(&self.icon) },
                gtk::Label {
                    set_label: &self.name,
                    add_css_class: constants::SIDEBAR_LABEL_CLASS,
                    set_hexpand: true,
                    set_halign: gtk::Align::Start,
                },
               #[name = "eject_button"]
                gtk::Button {
                    set_icon_name: "media-eject-symbolic",
                    set_visible: self.is_mount,
                    add_css_class: "flat",
                    connect_clicked[path = self.path.clone()] => move |_| {
                        if let Some(s) = crate::model::SENDER.get() {
                            let _ = s.send(crate::model::AppMsg::UnmountDevice(path.clone()));
                        }
                    }
                }
            }
        }
    }

    fn init_model(init: Self::Init, _: &DynamicIndex, _: FactorySender<Self>) -> Self {
        init
    }
}
