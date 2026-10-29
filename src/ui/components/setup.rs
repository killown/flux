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

/// Returns the [gtk::GridView] that owns `widget`, if any.
fn owning_grid_view(widget: &gtk::Widget) -> Option<gtk::GridView> {
    widget
        .ancestor(gtk::GridView::static_type())
        .and_then(|ancestor| ancestor.downcast::<gtk::GridView>().ok())
}

/// Switches off rubber-band selection on the grid view that owns `widget`.
fn suspend_rubberband(widget: &gtk::Widget) -> bool {
    match owning_grid_view(widget) {
        Some(view) if view.enables_rubberband() => {
            view.set_enable_rubberband(false);
            true
        }
        _ => false,
    }
}

/// Restores rubber-band selection previously disabled by [`suspend_rubberband`].
fn resume_rubberband(widget: &gtk::Widget, suspended: &std::cell::Cell<bool>) {
    if suspended.replace(false) {
        if let Some(view) = owning_grid_view(widget) {
            view.set_enable_rubberband(true);
        }
    }
}

/// Deselects every item of the grid view that owns `widget`.
///
/// Used when a Ctrl+Shift rubber-band starts, so previously selected items
/// outside the ring do not stay selected once the gesture ends.
fn clear_grid_selection(widget: &gtk::Widget) {
    if let Some(model) = owning_grid_view(widget).and_then(|view| view.model()) {
        model.unselect_all();
    }
}

/// Paths of the whole selection when `widget` is one of several selected cells.
///
/// Returns `None` (single-item drag, exactly as before) when the selection has
/// fewer than two items, when the dragged cell is not part of it, when any
/// selected item lives inside an archive, or when an item cannot be resolved.
/// Runs at drag time on the GTK thread, never from `bind`.
fn multi_drag_paths(widget: &gtk::Widget) -> Option<Vec<PathBuf>> {
    crate::hit!("multi_drag_paths");
    let dragged: PathBuf = unsafe {
        widget
            .data::<PathBuf>("flux_path")
            .map(|p| p.as_ref().clone())
    }?;
    let model = owning_grid_view(widget)?.model()?;
    let selection = model.selection();
    if selection.size() < 2 {
        return None;
    }

    let (mut iter, first) = gtk::BitsetIter::init_first(&selection)?;
    let mut paths = Vec::with_capacity(selection.size() as usize);
    let mut current = Some(first);
    while let Some(idx) = current {
        let item = model.item(idx).and_downcast::<glib::BoxedAnyObject>()?;
        let path = item.borrow::<FileItem>().path.clone();
        if path
            .to_string_lossy()
            .starts_with(crate::infra::archive::ARCHIVE_URI)
        {
            return None;
        }
        paths.push(path);
        current = iter.next();
    }

    // Dragging an unselected cell must not drag the selection along.
    paths.contains(&dragged).then_some(paths)
}

/// Drag payload for several files: a `FileList` for in-app drop targets and a
/// CRLF-separated `text/uri-list` (same body as `handle_clipboard_action`) for
/// other windows and applications.
fn multi_drag_provider(paths: &[PathBuf]) -> gdk::ContentProvider {
    crate::hit!("multi_drag_provider");
    let files: Vec<gtk::gio::File> = paths.iter().map(gtk::gio::File::for_path).collect();

    let mut uri_list = String::new();
    for file in &files {
        uri_list.push_str(&file.uri());
        uri_list.push_str("\r\n");
    }

    let uri_provider =
        gdk::ContentProvider::for_bytes("text/uri-list", &glib::Bytes::from(uri_list.as_bytes()));
    let file_list = gdk::FileList::from_array(&files);
    let file_list_provider = gdk::ContentProvider::for_value(&file_list.to_value());
    gdk::ContentProvider::new_union(&[file_list_provider, uri_provider])
}

/// Drag icon for several items: the pressed cell plus a `+N` badge.
fn set_multi_drag_icon(drag: &gdk::Drag, cell: &gtk::WidgetPaintable, count: usize) {
    crate::hit!("set_multi_drag_icon");
    let picture = gtk::Picture::for_paintable(cell);
    picture.set_can_shrink(false);

    let badge = gtk::Label::new(Some(&format!("+{}", count - 1)));
    badge.add_css_class("osd");
    badge.add_css_class("caption-heading");
    badge.set_halign(gtk::Align::End);
    badge.set_valign(gtk::Align::Start);

    let overlay = gtk::Overlay::new();
    overlay.set_child(Some(&picture));
    overlay.add_overlay(&badge);

    if let Ok(icon) = gtk::DragIcon::for_drag(drag).downcast::<gtk::DragIcon>() {
        icon.set_child(Some(&overlay));
    }
}

/// Disables `source` for the current press sequence, remembering its phase.
///
/// Used for the Ctrl+Shift select gesture: with the controller detached from
/// event propagation the press falls through to the grid view, so rubber-band
/// selection can start from any item. `saved` holds the phase to restore and
/// doubles as the "was disabled here" flag.
fn suspend_drag_source(
    source: &gtk::DragSource,
    saved: &std::cell::Cell<Option<gtk::PropagationPhase>>,
) {
    if saved.get().is_none() {
        saved.set(Some(source.propagation_phase()));
        source.set_propagation_phase(gtk::PropagationPhase::None);
    }
}

/// Re-attaches `source` after [`suspend_drag_source`].
///
/// A no-op unless `saved` holds a phase, so release and cancel paths may both
/// call it.
fn resume_drag_source(
    source: &gtk::DragSource,
    saved: &std::cell::Cell<Option<gtk::PropagationPhase>>,
) {
    if let Some(phase) = saved.take() {
        source.set_propagation_phase(phase);
    }
}

impl FileItem {
    /// Initializes the widget hierarchy and event controllers for a grid item.
    ///
    /// Sets up drag-and-drop functionality and mouse gesture listeners for
    /// context menu interaction.
    pub(super) fn build_widgets(_item: &gtk::ListItem) -> (gtk::Overlay, FileWidgets) {
        crate::hit!("build_widgets");
        let config = utils::config_snapshot();
        let dnd_enabled = !config.ui.disable_drag_and_drop;
        let rubberband_suspended = std::rc::Rc::new(std::cell::Cell::new(false));
        let drag_phase_saved =
            std::rc::Rc::new(std::cell::Cell::new(None::<gtk::PropagationPhase>));
        let drag_source = gtk::DragSource::builder()
            .actions(gdk::DragAction::COPY | gdk::DragAction::MOVE)
            .build();

        let single_click = config.ui.single_click;

        // Number of items in the drag being started, 0 means a single-item drag.
        let multi_count = std::rc::Rc::new(std::cell::Cell::new(0usize));

        // Falls back to the single-item provider that `bind` installed on the
        // source. Returning `None` here would not do that: the signal stops at
        // the first handler, so GTK's default handler, which returns the
        // `content` property, never runs and the drag would not start.
        let prepare_count = multi_count.clone();
        drag_source.connect_prepare(move |src, _, _| {
            prepare_count.set(0);
            if !dnd_enabled {
                return None;
            }
            let multi = src.widget().as_ref().and_then(multi_drag_paths);
            match multi {
                Some(paths) => {
                    prepare_count.set(paths.len());
                    Some(multi_drag_provider(&paths))
                }
                None => src.content(),
            }
        });

        let begin_count = multi_count.clone();
        drag_source.connect_drag_begin(move |src, drag| {
            if let Some(widget) = src.widget() {
                let paintable = gtk::WidgetPaintable::new(Some(&widget));
                let count = begin_count.get();
                if count > 1 {
                    set_multi_drag_icon(drag, &paintable, count);
                } else {
                    src.set_icon(Some(&paintable), 0, 0);
                }
                if let Some(native) = widget.native() {
                    if let Some(surface) = native.surface() {
                        surface.set_cursor(gdk::Cursor::from_name("grabbing", None).as_ref());
                    }
                }
                widget.set_cursor_from_name(Some("grabbing"));
            }
        });

        let rubberband_on_end = rubberband_suspended.clone();
        drag_source.connect_drag_end(move |src, _, _| {
            if let Some(widget) = src.widget() {
                resume_rubberband(&widget, &rubberband_on_end);
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

        let rubberband_on_cancel = rubberband_suspended.clone();
        drag_source.connect_drag_cancel(move |src, _, _| {
            if let Some(widget) = src.widget() {
                resume_rubberband(&widget, &rubberband_on_cancel);
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
            .css_classes([constants::THUMBNAIL_CLASS, "flux-vd-folder"])
            .build();

        // The archive's real icon for a virtual dir, stacked over `icon_widget`.
        // It is transparent until the grid child is `:selected` (see `style.css`),
        // so the swap needs no signal handler and no repaint.
        let original_widget = gtk::Image::builder()
            .halign(gtk::Align::Fill)
            .valign(gtk::Align::Fill)
            .can_target(false)
            .css_classes([constants::THUMBNAIL_CLASS, "flux-vd-original"])
            .build();

        let icon_overlay = gtk::Overlay::new();
        icon_overlay.set_child(Some(&icon_widget));
        icon_overlay.add_overlay(&original_widget);

        // Padlock hints for encrypted archives. Two fixed widgets instead of one with a
        // swapped icon, so `bind` flips visibility only. They sit on the icon's corner,
        // clear of the archive and git badges, and can never take a click.
        let make_lock_badge = |icon_name: &str, state_class: &str| {
            gtk::Image::builder()
                .icon_name(icon_name)
                .pixel_size(12)
                .halign(gtk::Align::End)
                .valign(gtk::Align::End)
                .can_target(false)
                .can_focus(false)
                .visible(false)
                .css_classes(["flux-lock-hint", state_class])
                .build()
        };
        let lock_badge_closed =
            make_lock_badge("changes-prevent-symbolic", "flux-lock-hint--locked");
        let lock_badge_open = make_lock_badge("changes-allow-symbolic", "flux-lock-hint--unlocked");
        icon_overlay.add_overlay(&lock_badge_closed);
        icon_overlay.add_overlay(&lock_badge_open);

        let video_widget = gtk::Video::builder()
            .autoplay(true)
            .loop_(true)
            .halign(gtk::Align::Fill)
            .valign(gtk::Align::Fill)
            .css_classes([constants::THUMBNAIL_CLASS])
            .build();

        // gtk::Video reports the video's intrinsic size as its natural size,
        // which would stretch the grid cell. A ScrolledWindow that does not
        // propagate natural size reports only the size we request on it.
        let video_frame = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .vscrollbar_policy(gtk::PolicyType::Never)
            .propagate_natural_width(false)
            .propagate_natural_height(false)
            .halign(gtk::Align::Fill)
            .valign(gtk::Align::Fill)
            .child(&video_widget)
            .build();

        let preview_stack = gtk::Stack::builder()
            .transition_type(gtk::StackTransitionType::Crossfade)
            .halign(gtk::Align::Center)
            .valign(gtk::Align::End)
            .vexpand(false)
            .hhomogeneous(false)
            .vhomogeneous(false)
            .build();

        preview_stack.add_named(&icon_overlay, Some("icon"));
        preview_stack.add_named(&video_frame, Some("video"));
        preview_stack.set_visible_child_name("icon");

        let git_badge = gtk::Label::builder()
            .halign(gtk::Align::End)
            .valign(gtk::Align::Start)
            .visible(false)
            .build();
        git_badge.add_css_class("flux-git-badge");

        // Static marker so an archive drawn as a folder still reads as an archive
        // without hovering. Hidden until `bind_widgets` sees a virtual dir.
        let archive_badge = gtk::Image::builder()
            .icon_name("package-x-generic-symbolic")
            .pixel_size(14)
            .halign(gtk::Align::Start)
            .valign(gtk::Align::Start)
            .can_target(false)
            .visible(false)
            .css_classes(["flux-archive-badge"])
            .build();

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
                        connect_pressed[timer = grab_timer.clone(), suspended = rubberband_suspended.clone(), phase_saved = drag_phase_saved.clone(), drag_source = drag_source.clone(), dnd_enabled] => move |gesture, _, _, _| {
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

                            if dnd_enabled
                                && gesture.current_event_state().contains(
                                    gdk::ModifierType::CONTROL_MASK | gdk::ModifierType::SHIFT_MASK,
                                )
                            {
                                suspend_drag_source(&drag_source, &phase_saved);
                                if let Some(widget) = gesture.widget() {
                                    clear_grid_selection(&widget);
                                }
                                return;
                            }

                            if dnd_enabled {
                                if let Some(widget) = gesture.widget() {
                                    if suspend_rubberband(&widget) {
                                        suspended.set(true);
                                    }
                                }
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
                        connect_released[single_click, timer = grab_timer.clone(), suspended = rubberband_suspended.clone(), phase_saved = drag_phase_saved.clone(), drag_source = drag_source.clone()] => move |gesture, _, _, _| {
                            if let Some(id) = timer.take() {
                                id.remove();
                            }
                            resume_drag_source(&drag_source, &phase_saved);
                            if let Some(widget) = gesture.widget() {
                                resume_rubberband(&widget, &suspended);
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
                        connect_cancel[single_click, timer = grab_timer, suspended = rubberband_suspended, phase_saved = drag_phase_saved, drag_source = drag_source.clone()] => move |gesture, _| {
                            if let Some(id) = timer.take() {
                                id.remove();
                            }
                            resume_drag_source(&drag_source, &phase_saved);
                            if let Some(widget) = gesture.widget() {
                                resume_rubberband(&widget, &suspended);
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
                                    let (rel_x, rel_y) = crate::utils::helpers::translate_point(&widget, &popover_parent, x, y).unwrap_or((x, y));
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
                                        let (rel_x, rel_y) = crate::utils::helpers::translate_point(&widget, &popover_parent, x, y).unwrap_or((x, y));
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
                add_overlay: &archive_badge,
            }
        }

        if dnd_enabled {
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
                archive_badge,
                lock_badge_closed,
                lock_badge_open,
                original_widget,
                icon_overlay,
            },
        )
    }
}
