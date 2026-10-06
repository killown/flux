use super::file_item::FileItem;
use super::widgets::FileWidgets;
use crate::model::FluxApp;
use crate::ui::constants;
use crate::ui::constants::MOUSE_RIGHT_CLICK;
use crate::utils;
use adw::gdk;
use adw::prelude::*;
use gtk::glib;
use relm4::prelude::*;
use std::cell::RefCell;
use std::path::PathBuf;

impl FileItem {
    /// Initializes the widget hierarchy and event controllers for a grid item.
    ///
    /// Sets up drag-and-drop functionality and mouse gesture listeners for
    /// context menu interaction.
    pub(super) fn build_widgets(_item: &gtk::ListItem) -> (gtk::Overlay, FileWidgets) {
        let config = utils::load_config();
        let drag_source = gtk::DragSource::builder()
            .actions(gdk::DragAction::COPY | gdk::DragAction::MOVE)
            .build();

        let single_click = config.ui.single_click;

        drag_source.connect_drag_begin(|src, _| {
            if let Some(widget) = src.widget() {
                let paintable = gtk::WidgetPaintable::new(Some(&widget));
                src.set_icon(Some(&paintable), 0, 0);
                if let Some(native) = widget.native() {
                    if let Some(surface) = native.surface() {
                        surface.set_cursor(gdk::Cursor::from_name("grabbing", None).as_ref());
                    }
                }
                widget.set_cursor_from_name(Some("grabbing"));
            }
        });

        drag_source.connect_drag_end(move |src, _, _| {
            if let Some(widget) = src.widget() {
                if let Some(native) = widget.native() {
                    if let Some(surface) = native.surface() {
                        surface.set_cursor(None);
                    }
                }
                if single_click {
                    widget.set_cursor_from_name(Some("pointer"));
                } else {
                    widget.set_cursor(None);
                }
            }
        });

        drag_source.connect_drag_cancel(move |src, _, _| {
            if let Some(widget) = src.widget() {
                if let Some(native) = widget.native() {
                    if let Some(surface) = native.surface() {
                        surface.set_cursor(None);
                    }
                }
                if single_click {
                    widget.set_cursor_from_name(Some("pointer"));
                } else {
                    widget.set_cursor(None);
                }
            }
            false
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
                    .data::<std::rc::Weak<RefCell<Option<PathBuf>>>>("active_path_cell")
                    .and_then(|weak_ptr| weak_ptr.as_ref().upgrade())
                    .and_then(|rc| rc.borrow().clone())
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

        // Right-aligned info label for list mode (extension).
        // Created imperatively so it can be appended after the Stack.
        let info_label = gtk::Label::builder()
            .halign(gtk::Align::End)
            .valign(gtk::Align::Center)
            .hexpand(false)
            .opacity(0.5)
            .visible(false)
            .build();
        info_label.add_css_class("caption");
        info_label.add_css_class("flux-list-info");

        let icon_widget = gtk::Image::builder()
            .halign(gtk::Align::Center)
            .valign(gtk::Align::End)
            .vexpand(false)
            .css_classes([constants::THUMBNAIL_CLASS])
            .build();

        let video_widget = gtk::Video::builder()
            .autoplay(true)
            .loop_(true)
            .halign(gtk::Align::Center)
            .valign(gtk::Align::End)
            .vexpand(false)
            .css_classes([constants::THUMBNAIL_CLASS])
            .build();

        let preview_stack = gtk::Stack::builder()
            .transition_type(gtk::StackTransitionType::Crossfade)
            .halign(gtk::Align::Center)
            .valign(gtk::Align::End)
            .vexpand(false)
            .hhomogeneous(false)
            .vhomogeneous(false)
            .build();

        preview_stack.add_named(&icon_widget, Some("icon"));
        preview_stack.add_named(&video_widget, Some("video"));
        preview_stack.set_visible_child_name("icon");

        let git_badge = gtk::Label::builder()
            .halign(gtk::Align::End)
            .valign(gtk::Align::Start)
            .visible(false)
            .build();
        git_badge.add_css_class("flux-git-badge");

        let grab_timer = std::rc::Rc::new(std::cell::Cell::new(None::<glib::SourceId>));

        relm4::view! {
            #[root]
            root = gtk::Overlay {
                #[name = "card_box"]
                gtk::Box {
                    set_orientation: gtk::Orientation::Vertical,
                    set_halign: gtk::Align::Fill,
                    set_valign: gtk::Align::Fill,
                    set_spacing: 0,
                    add_css_class: constants::CARD_CSS_CLASS,

                    // only if single_click is enabled
                    connect_realize => move |w| {
                        FluxApp::set_cursor_pointer(w.as_ref(), config.ui.single_click);
                    },

                    // Sets hand/grabbing cursor on sustained mouse press, restores on release or cancel
                    add_controller = gtk::GestureClick {
                        set_button: 1,
                        set_propagation_phase: gtk::PropagationPhase::Capture,
                        connect_pressed[timer = grab_timer.clone()] => move |gesture, _, _, _| {
                            if let Some(event) = gesture.current_event() {
                                if let Some(device) = event.device() {
                                    if device.source() == gdk::InputSource::Touchscreen {
                                        return;
                                    }
                                }
                            }

                            if let Some(id) = timer.take() {
                                id.remove();
                            }

                            if let Some(widget) = gesture.widget() {
                                let widget_weak = widget.downgrade();
                                let timer_clone = timer.clone();
                                let gesture_weak = gesture.downgrade();

                                let id = glib::timeout_add_local_once(
                                    std::time::Duration::from_millis(300),
                                    move || {
                                        timer_clone.set(None);
                                        // Ensure the gesture is still active and the pointer button is still pressed
                                        let is_still_holding = gesture_weak
                                            .upgrade()
                                            .map(|g| g.is_recognized())
                                            .unwrap_or(false);

                                        if is_still_holding {
                                            if let Some(w) = widget_weak.upgrade() {
                                                if let Some(native) = w.native() {
                                                    if let Some(surface) = native.surface() {
                                                        surface.set_cursor(gdk::Cursor::from_name("grabbing", None).as_ref());
                                                    }
                                                }
                                                w.set_cursor_from_name(Some("grabbing"));
                                            }
                                        }
                                    },
                                );
                                timer.set(Some(id));
                            }
                        },
                        connect_released[single_click, timer = grab_timer.clone()] => move |gesture, _, _, _| {
                            if let Some(id) = timer.take() {
                                id.remove();
                            }
                            if let Some(widget) = gesture.widget() {
                                if let Some(native) = widget.native() {
                                    if let Some(surface) = native.surface() {
                                        surface.set_cursor(None);
                                    }
                                }
                                if single_click {
                                    widget.set_cursor_from_name(Some("pointer"));
                                } else {
                                    widget.set_cursor(None);
                                }
                            }
                        },
                        connect_cancel[single_click, timer = grab_timer] => move |gesture, _| {
                            if let Some(id) = timer.take() {
                                id.remove();
                            }
                            if let Some(widget) = gesture.widget() {
                                if let Some(native) = widget.native() {
                                    if let Some(surface) = native.surface() {
                                        surface.set_cursor(None);
                                    }
                                }
                                if single_click {
                                    widget.set_cursor_from_name(Some("pointer"));
                                } else {
                                    widget.set_cursor(None);
                                }
                            }
                        }
                    },

                    add_controller = gtk::GestureLongPress {
                        // Only claim or handle long press on touch input, preventing mouse drag conflicts
                        connect_pressed[sender = crate::model::SENDER.clone()] => move |gesture, x, y| {
                            if let Some(event) = gesture.current_event() {
                                // Reject any event originating from a mouse button press
                                if let Some(btn_event) = event.downcast_ref::<gdk::ButtonEvent>() {
                                    if btn_event.button() != 0 {
                                        gesture.set_state(gtk::EventSequenceState::Denied);
                                        return;
                                    }
                                }

                                // Strictly require touchscreen input source
                                if let Some(device) = event.device() {
                                    if device.source() != gdk::InputSource::Touchscreen {
                                        gesture.set_state(gtk::EventSequenceState::Denied);
                                        return;
                                    }
                                }
                            } else {
                                gesture.set_state(gtk::EventSequenceState::Denied);
                                return;
                            }

                            if let Some(s) = sender.get() {
                                let widget = gesture.widget().unwrap();
                                let path_opt: Option<PathBuf> = unsafe {
                                    widget
                                        .data::<PathBuf>("flux_path")
                                        .map(|p| p.as_ref().clone())
                                };

                                if let Some(popover_parent) = widget.ancestor(gtk::GridView::static_type()) {
                                    let (rel_x, rel_y) = widget.translate_coordinates(&popover_parent, x, y).unwrap_or((x, y));
                                    gesture.set_state(gtk::EventSequenceState::Claimed);
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
                                    let path_opt: Option<PathBuf> = unsafe {
                                        widget
                                            .data::<PathBuf>("flux_path")
                                            .map(|p| p.as_ref().clone())
                                    };

                                    if let Some(popover_parent) = widget.ancestor(gtk::GridView::static_type()) {
                                        let (rel_x, rel_y) = widget.translate_coordinates(&popover_parent, x, y).unwrap_or((x, y));
                                        s.send(crate::model::AppMsg::PrepareContextMenu(rel_x, rel_y, path_opt)).ok();
                                    }
                                }
                            }
                        }
                    },

                    append: &preview_stack,

                    #[name = "label_scroller"]
                    gtk::ScrolledWindow {
                        set_hscrollbar_policy: gtk::PolicyType::Never,
                        set_vscrollbar_policy: gtk::PolicyType::Never,
                        set_propagate_natural_width: true,
                        set_propagate_natural_height: true,
                        set_valign: gtk::Align::Start,
                        set_hexpand: false,
                        set_vexpand: false,
                        add_css_class: "flux-label-scroller",

                        #[name = "stack"]
                        #[wrap(Some)]
                        set_child = &gtk::Stack {
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
                                    set_ellipsize: gtk::pango::EllipsizeMode::End,
                                    set_hexpand: false,
                                    add_css_class: constants::FLUX_LABEL_CLASS,
                                },
                            } -> { set_name: constants::VIEW_LABEL }
                        }
                    }
                },

                add_overlay: &git_badge,
            }
        }

        if !config.ui.disable_drag_and_drop {
            root.add_controller(drag_source.clone());
            root.add_controller(drop_target.clone());
        }

        // Append the info label after the scroller so it sits on the far right
        // of the horizontal list row. In grid mode it is hidden.
        card_box.append(&info_label);

        unsafe {
            root.set_data("preview_stack", preview_stack.clone());
            root.set_data("video_widget", video_widget.clone());
        }

        (
            root,
            FileWidgets {
                card_box,
                icon_widget,
                video_widget,
                preview_stack,
                lock_icon,
                label,
                stack,
                drag_source,
                drop_target,
                info_label,
                label_scroller,
                scale_css_provider: None,
                git_badge,
            },
        )
    }
}
