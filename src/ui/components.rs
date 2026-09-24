use crate::i18n::tr;
use crate::model::FluxApp;
use crate::model::PathSegment;
use crate::ui::constants;
use crate::ui::constants::MOUSE_RIGHT_CLICK;
use crate::utils;
use adw::gdk;
use adw::prelude::*;
use gtk::gio;
use gtk::glib;
use gtk::glib::clone;
use relm4::prelude::*;
use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

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
    pub is_empty: bool,
    pub is_foreign_owner: bool,
    pub search_snippet: Option<String>,
    /// Whether the label should wrap to multiple lines instead of ellipsizing.
    pub expand_labels: bool,
    /// Whether this icon was set by the user via F3 (custom folder icon).
    /// Used to determine if we should fall back to default when icon missing from theme.
    pub is_custom_icon: bool,
    /// When true the item renders as a compact horizontal row instead of the
    /// default vertical card (icon top, label bottom).
    pub is_list_mode: bool,
    /// Shared cell holding the current path for this item.
    /// Updated in `bind()` and read by the right‑click gesture.
    pub active_path: Rc<RefCell<Option<PathBuf>>>,
    // In the FileItem struct, add after `is_custom_icon`:
    /// Unix timestamp of the last modification time, `0` when unavailable.
    pub mtime: i64,
    /// Position of this item in the grid model.
    ///
    /// Set at construction time in `loader.rs` and read by `bind()` to dispatch
    /// lazy thumbnail requests without relying on widget-data that may not yet
    /// be populated.
    pub grid_idx: u32,
    /// Cached from config at construction time - avoids a config.toml read per bind() call.
    pub max_width_chars: i32,
    /// Cached from config at construction time - avoids a config.toml read per bind() call.
    pub grid_spacing: i32,
    /// Whether the target is a symbolic link.
    pub is_symlink: bool,
    /// Whether the symbolic link target does not exist.
    pub is_broken_symlink: bool,
    /// User setting controlling whether the visual symbolic link badge is shown.
    pub show_symlink_emblem: bool,
    /// Line number from a content-search hit. 0 for all other items.
    pub line_number: usize,
    /// Whether the item is currently cut (moved via clipboard).
    pub is_cut: bool,
}

impl FileItem {
    pub fn builder(name: String, path: PathBuf, icon: adw::gio::Icon) -> FileItemBuilder {
        FileItemBuilder {
            name,
            path,
            icon,
            is_dir: false,
            size: 0,
            mtime: 0,
            icon_size: 32,
            is_list_mode: false,
            max_width_chars: 20,
            grid_spacing: 10,
            show_symlink_emblem: true,
            grid_idx: 0,
            line_number: 0,
            is_cut: false,
            is_empty: false,
            is_symlink: false,
            is_broken_symlink: false,
            expand_labels: false,
            is_foreign_owner: false,
            is_custom_icon: false,
            thumbnail: None,
            search_snippet: None,
        }
    }
}

#[allow(dead_code)]
pub struct FileItemBuilder {
    name: String,
    path: PathBuf,
    icon: adw::gio::Icon,
    is_dir: bool,
    size: u64,
    mtime: i64,
    icon_size: i32,
    is_list_mode: bool,
    max_width_chars: i32,
    grid_spacing: i32,
    show_symlink_emblem: bool,
    grid_idx: u32,
    line_number: usize,
    is_cut: bool,
    is_empty: bool,
    is_symlink: bool,
    is_broken_symlink: bool,
    expand_labels: bool,
    is_foreign_owner: bool,
    is_custom_icon: bool,
    thumbnail: Option<gdk::Texture>,
    search_snippet: Option<String>,
}

#[allow(dead_code)]
impl FileItemBuilder {
    pub fn is_dir(mut self, v: bool) -> Self {
        self.is_dir = v;
        self
    }

    pub fn size(mut self, v: u64) -> Self {
        self.size = v;
        self
    }

    pub fn mtime(mut self, v: i64) -> Self {
        self.mtime = v;
        self
    }

    pub fn icon_size(mut self, v: i32) -> Self {
        self.icon_size = v;
        self
    }

    pub fn is_list_mode(mut self, v: bool) -> Self {
        self.is_list_mode = v;
        self
    }

    pub fn max_width_chars(mut self, v: i32) -> Self {
        self.max_width_chars = v;
        self
    }

    pub fn grid_spacing(mut self, v: i32) -> Self {
        self.grid_spacing = v;
        self
    }

    pub fn show_symlink_emblem(mut self, v: bool) -> Self {
        self.show_symlink_emblem = v;
        self
    }

    pub fn grid_idx(mut self, v: u32) -> Self {
        self.grid_idx = v;
        self
    }

    pub fn line_number(mut self, v: usize) -> Self {
        self.line_number = v;
        self
    }

    pub fn is_cut(mut self, v: bool) -> Self {
        self.is_cut = v;
        self
    }

    pub fn is_empty(mut self, v: bool) -> Self {
        self.is_empty = v;
        self
    }

    pub fn is_symlink(mut self, v: bool) -> Self {
        self.is_symlink = v;
        self
    }

    pub fn is_broken_symlink(mut self, v: bool) -> Self {
        self.is_broken_symlink = v;
        self
    }

    pub fn expand_labels(mut self, v: bool) -> Self {
        self.expand_labels = v;
        self
    }

    pub fn is_foreign_owner(mut self, v: bool) -> Self {
        self.is_foreign_owner = v;
        self
    }

    pub fn is_custom_icon(mut self, v: bool) -> Self {
        self.is_custom_icon = v;
        self
    }

    pub fn thumbnail(mut self, v: Option<gdk::Texture>) -> Self {
        self.thumbnail = v;
        self
    }

    pub fn search_snippet(mut self, v: Option<String>) -> Self {
        self.search_snippet = v;
        self
    }

    pub fn build(self) -> FileItem {
        FileItem {
            name: self.name,
            icon: self.icon,
            size: self.size,
            thumbnail: self.thumbnail,
            is_dir: self.is_dir,
            path: self.path,
            icon_size: self.icon_size,
            is_editing: false,
            is_empty: self.is_empty,
            is_foreign_owner: self.is_foreign_owner,
            search_snippet: self.search_snippet,
            expand_labels: self.expand_labels,
            is_custom_icon: self.is_custom_icon,
            is_list_mode: self.is_list_mode,
            active_path: Rc::new(RefCell::new(None)),
            mtime: self.mtime,
            grid_idx: self.grid_idx,
            max_width_chars: self.max_width_chars,
            grid_spacing: self.grid_spacing,
            is_symlink: self.is_symlink,
            is_broken_symlink: self.is_broken_symlink,
            show_symlink_emblem: self.show_symlink_emblem,
            line_number: self.line_number,
            is_cut: self.is_cut,
        }
    }
}

/// Collection of GTK widgets utilized by a [FileItem] within the grid view.
pub struct FileWidgets {
    pub icon_widget: gtk::Image,
    pub video_widget: gtk::Video,
    pub preview_stack: gtk::Stack,
    pub lock_icon: gtk::Image,
    pub label: gtk::Label,
    pub stack: gtk::Stack,
    pub drag_source: gtk::DragSource,
    pub drop_target: gtk::DropTarget,
    pub info_label: gtk::Label,
    pub label_scroller: gtk::ScrolledWindow,
    pub scale_css_provider: Option<gtk::CssProvider>,
}

/// Formats a byte count into a human-readable string (B / KB / MB / GB).
fn format_size(bytes: u64) -> String {
    const KB: u64 = 1_024;
    const MB: u64 = 1_024 * KB;
    const GB: u64 = 1_024 * MB;
    match bytes {
        b if b >= GB => format!("{:.1} GB", b as f64 / GB as f64),
        b if b >= MB => format!("{:.1} MB", b as f64 / MB as f64),
        b if b >= KB => format!("{:.0} KB", b as f64 / KB as f64),
        b => format!("{b} B"),
    }
}

impl relm4::typed_view::grid::RelmGridItem for FileItem {
    type Root = gtk::Box;
    type Widgets = FileWidgets;

    /// Initializes the widget hierarchy and event controllers for a grid item.
    ///
    /// Sets up drag-and-drop functionality and mouse gesture listeners for
    /// context menu interaction.
    fn setup(_item: &gtk::ListItem) -> (Self::Root, Self::Widgets) {
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

        let grab_timer = std::rc::Rc::new(std::cell::Cell::new(None::<glib::SourceId>));

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
                            let idx_opt: Option<u32> = unsafe {
                                widget.data::<u32>("grid_item_index").map(|ptr| *ptr.as_ref())
                            };

                            if let Some(popover_parent) = widget.ancestor(gtk::GridView::static_type()) {
                                let (rel_x, rel_y) = widget.translate_coordinates(&popover_parent, x, y).unwrap_or((x, y));
                                gesture.set_state(gtk::EventSequenceState::Claimed);
                                s.send(crate::model::AppMsg::PrepareContextMenu(rel_x, rel_y, idx_opt)).ok();
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

                                let idx_opt: Option<u32> = unsafe {
                                    widget.data::<u32>("grid_item_index").map(|ptr| *ptr.as_ref())
                                };

                                if let Some(popover_parent) = widget.ancestor(gtk::GridView::static_type()) {
                                    let (rel_x, rel_y) = widget.translate_coordinates(&popover_parent, x, y).unwrap_or((x, y));
                                    s.send(crate::model::AppMsg::PrepareContextMenu(rel_x, rel_y, idx_opt)).ok();
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
            }
        }

        if !config.ui.disable_drag_and_drop {
            root.add_controller(drag_source.clone());
            root.add_controller(drop_target.clone());
        }

        // Append the info label after the scroller so it sits on the far right
        // of the horizontal list row. In grid mode it is hidden.
        root.append(&info_label);

        unsafe {
            root.set_data("preview_stack", preview_stack.clone());
            root.set_data("video_widget", video_widget.clone());
        }

        (
            root,
            FileWidgets {
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
            },
        )
    }

    /// Updates the item's widgets with current data from the [FileItem] model.
    ///
    /// Synchronizes labels, icons, thumbnails, and visibility states (e.g., rename entry).
    fn bind(&mut self, widgets: &mut Self::Widgets, root: &mut Self::Root) {
        let config = utils::load_config();
        let display_label = utils::helpers::format_display_label(
            &self.name,
            self.is_dir,
            &config.ui.hidden_extensions,
        );
        widgets.label.set_label(&display_label);

        if self.is_cut {
            root.add_css_class("flux-card--cut");
        } else {
            root.remove_css_class("flux-card--cut");
        }

        if self.is_list_mode {
            // Compact horizontal row: small icon on the left, filename fills the rest.
            root.set_orientation(gtk::Orientation::Horizontal);
            root.set_halign(gtk::Align::Fill);
            root.set_hexpand(true);
            root.set_spacing(10);

            widgets.icon_widget.set_pixel_size(self.icon_size);
            widgets
                .icon_widget
                .set_size_request(self.icon_size, self.icon_size);
            widgets.icon_widget.set_valign(gtk::Align::Center);
            widgets.icon_widget.set_halign(gtk::Align::Start);

            widgets
                .video_widget
                .set_size_request(self.icon_size, self.icon_size);

            widgets
                .preview_stack
                .set_size_request(self.icon_size, self.icon_size);
            widgets.preview_stack.set_valign(gtk::Align::Center);
            widgets.preview_stack.set_halign(gtk::Align::Start);
            widgets.preview_stack.set_visible_child_name("icon");
            widgets.label.set_halign(gtk::Align::Start);
            widgets.label.set_hexpand(true);
            widgets.label.set_justify(gtk::Justification::Left);
            widgets.label.set_width_chars(-1);
            widgets.label.set_max_width_chars(-1);
            // Never wrap or ellipsize in list mode: the ScrolledWindow handles overflow.
            widgets.label.set_wrap(false);
            widgets.label.set_ellipsize(gtk::pango::EllipsizeMode::None);

            if let Some(ref old_provider) = widgets.scale_css_provider {
                widgets.label.style_context().remove_provider(old_provider);
                widgets
                    .info_label
                    .style_context()
                    .remove_provider(old_provider);
                widgets.scale_css_provider = None;
            }

            if config.ui.scale_font_with_icons && self.icon_size > 0 {
                const DEFAULT_LIST_ICON_BASELINE: f64 = 64.0;

                let ratio = self.icon_size as f64 / DEFAULT_LIST_ICON_BASELINE;

                // Allow a broader clamp so size differences between 16px and 48px+ are visible
                let scale_factor = ratio.clamp(0.8, 2.0);

                let label_attrs = gtk::pango::AttrList::new();
                label_attrs.insert(gtk::pango::AttrFloat::new_scale(scale_factor));
                widgets.label.set_attributes(Some(&label_attrs));

                let info_attrs = gtk::pango::AttrList::new();
                info_attrs.insert(gtk::pango::AttrFloat::new_scale(scale_factor));
                widgets.info_label.set_attributes(Some(&info_attrs));
            } else {
                widgets.label.set_attributes(None);
                widgets.info_label.set_attributes(None);
            }

            // In list mode the scroller provides horizontal scrolling so the row
            // never grows wider than its allocated column width.
            widgets
                .label_scroller
                .set_hscrollbar_policy(gtk::PolicyType::Automatic);
            widgets.label_scroller.set_propagate_natural_width(false);
            widgets.label_scroller.set_propagate_natural_height(true);
            widgets.label_scroller.set_valign(gtk::Align::Center);
            widgets.label_scroller.set_hexpand(true);

            // Stack and its label-box fill the scroller's viewport.
            widgets.stack.set_halign(gtk::Align::Fill);
            widgets.stack.set_valign(gtk::Align::Center);
            widgets.stack.set_hexpand(true);

            // Get the label container (the Box inside the Stack) and make it start-aligned.
            if let Some(label_box) = widgets.stack.child_by_name(constants::VIEW_LABEL) {
                if let Some(box_widget) = label_box.downcast_ref::<gtk::Box>() {
                    box_widget.set_halign(gtk::Align::Start);
                    box_widget.set_valign(gtk::Align::Center);
                    box_widget.set_hexpand(true);
                    box_widget.set_size_request(-1, -1);
                }
            }

            // Invalidate cached measurements across the scroller boundary to force size renegotiation
            widgets.label.queue_resize();
            widgets.info_label.queue_resize();
            widgets.label_scroller.queue_resize();
            root.queue_resize();

            // Populate the info label with item count, size, or left-aligned content search snippet.
            {
                let mut info_parts: Vec<String> = Vec::new();

                if let Some(ref snippet) = self.search_snippet {
                    widgets.info_label.set_halign(gtk::Align::End);
                    widgets.info_label.set_xalign(1.0);
                    widgets.info_label.set_hexpand(false);
                    if self.line_number > 0 {
                        info_parts.push(format!("L:{}  {}", self.line_number, snippet));
                    } else {
                        info_parts.push(snippet.clone());
                    }
                } else {
                    widgets.info_label.set_halign(gtk::Align::End);
                    widgets.info_label.set_xalign(1.0);
                    widgets.info_label.set_hexpand(false);

                    if self.is_dir {
                        let count_str = if self.size == 1 {
                            "1 item".to_string()
                        } else {
                            format!("{} {}", self.size, tr("items"))
                        };
                        info_parts.push(count_str);
                    } else if self.size > 0 {
                        info_parts.push(format_size(self.size));
                    }

                    if self.mtime > 0 {
                        if let (Ok(file_dt), Ok(now_dt)) = (
                            glib::DateTime::from_unix_local(self.mtime),
                            glib::DateTime::now_local(),
                        ) {
                            let (f_y, f_m, f_d) =
                                (file_dt.year(), file_dt.month(), file_dt.day_of_month());
                            let (n_y, n_m, n_d) =
                                (now_dt.year(), now_dt.month(), now_dt.day_of_month());

                            let time_str = file_dt
                                .format("%H:%M")
                                .map(|g| g.to_string())
                                .unwrap_or_default();

                            if let Ok(exact) = file_dt.format("%x %X") {
                                widgets.info_label.set_tooltip_text(Some(exact.as_str()));
                            }

                            if f_y == n_y && f_m == n_m && f_d == n_d {
                                info_parts.push(format!("{}, {}", tr("Today"), time_str));
                            } else {
                                let is_yesterday = now_dt
                                    .add_days(-1)
                                    .map(|y_dt| {
                                        y_dt.year() == f_y
                                            && y_dt.month() == f_m
                                            && y_dt.day_of_month() == f_d
                                    })
                                    .unwrap_or(false);

                                if is_yesterday {
                                    info_parts.push(format!("{}, {}", tr("Yesterday"), time_str));
                                } else if let Ok(formatted) = file_dt.format("%x %H:%M") {
                                    info_parts.push(formatted.to_string());
                                }
                            }
                        }
                    } else {
                        widgets.info_label.set_tooltip_text(None::<&str>);
                    }
                }

                if !info_parts.is_empty() {
                    widgets.info_label.set_label(&info_parts.join(" · "));
                    widgets.info_label.set_visible(true);
                } else {
                    widgets.info_label.set_visible(false);
                }
            }
        } else {
            root.set_orientation(gtk::Orientation::Vertical);
            root.set_halign(gtk::Align::Center);
            root.set_spacing(0);
            widgets.icon_widget.set_pixel_size(self.icon_size);
            widgets.icon_widget.set_size_request(-1, -1);
            widgets.icon_widget.set_valign(gtk::Align::End);
            widgets.icon_widget.set_halign(gtk::Align::Center);
            widgets
                .video_widget
                .set_size_request(self.icon_size, self.icon_size);
            widgets.preview_stack.set_size_request(-1, -1);
            widgets.preview_stack.set_valign(gtk::Align::End);
            widgets.preview_stack.set_halign(gtk::Align::Center);
            widgets.preview_stack.set_visible_child_name("icon");
            widgets.label.set_halign(gtk::Align::Center);
            widgets.label.set_hexpand(false);
            widgets.label.set_justify(gtk::Justification::Center);
            widgets.label.set_max_width_chars(self.max_width_chars);
            widgets.label.set_width_chars(self.grid_spacing);

            if config.ui.scale_font_with_icons && self.icon_size > 0 {
                let base_size = if config.ui.default_icon_size > 0 {
                    config.ui.default_icon_size as f64
                } else {
                    96.0f64
                };
                let scale_factor = (self.icon_size as f64 / base_size).clamp(0.8, 1.3);
                let attrs = gtk::pango::AttrList::new();
                attrs.insert(gtk::pango::AttrFloat::new_scale(scale_factor));
                widgets.label.set_attributes(Some(&attrs));
            } else {
                widgets.label.set_attributes(None);
            }

            if self.expand_labels {
                widgets.label.set_wrap(true);
                widgets.label.set_wrap_mode(gtk::pango::WrapMode::WordChar);
                widgets.label.set_ellipsize(gtk::pango::EllipsizeMode::None);
            } else {
                widgets.label.set_wrap(false);
                widgets.label.set_ellipsize(gtk::pango::EllipsizeMode::End);
            }

            // Restore scroller to grid-mode defaults: no scrollbar, propagate natural size.
            widgets
                .label_scroller
                .set_hscrollbar_policy(gtk::PolicyType::Never);
            widgets.label_scroller.set_propagate_natural_width(true);
            widgets.label_scroller.set_valign(gtk::Align::Start);
            widgets.label_scroller.set_hexpand(false);

            // Reset Stack alignment for grid mode (centered horizontally, anchored to start vertically).
            widgets.stack.set_halign(gtk::Align::Center);
            widgets.stack.set_valign(gtk::Align::Start);
            widgets.stack.set_hexpand(false);
            if let Some(label_box) = widgets.stack.child_by_name(constants::VIEW_LABEL) {
                if let Some(box_widget) = label_box.downcast_ref::<gtk::Box>() {
                    box_widget.set_halign(gtk::Align::Center);
                    box_widget.set_valign(gtk::Align::Start);
                    box_widget.set_hexpand(false);
                    box_widget.set_size_request(-1, -1);
                }
            }

            // Always hide the info label in grid mode.
            widgets.info_label.set_visible(false);
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

        let show_indicator = self.show_symlink_emblem && self.is_symlink;

        if show_indicator {
            if self.is_broken_symlink {
                root.add_css_class("flux-card--broken-symlink");
                root.remove_css_class("flux-card--symlink");
            } else {
                root.add_css_class("flux-card--symlink");
                root.remove_css_class("flux-card--broken-symlink");
            }
        } else {
            root.remove_css_class("flux-card--symlink");
            root.remove_css_class("flux-card--broken-symlink");
        }

        if self.is_dir && config.ui.show_empty_dir_emblem {
            if self.is_empty {
                root.add_css_class("flux-card--empty");
            } else {
                root.remove_css_class("flux-card--empty");
            }
        } else {
            root.remove_css_class("flux-card--empty");
        }

        if self.is_editing {
            let entry = match widgets.stack.child_by_name(constants::VIEW_ENTRY) {
                Some(w) => w.downcast::<gtk::Entry>().unwrap(),
                None => {
                    let entry = gtk::Entry::builder()
                        .halign(gtk::Align::Center)
                        .css_classes([constants::RENAME_ENTRY_CLASS])
                        .build();

                    // Store the path directly as widget data on the entry itself
                    unsafe {
                        entry.set_data("target_rename_path", self.path.clone());
                    }

                    let submitted = std::rc::Rc::new(std::cell::Cell::new(false));

                    let focus_ctrl = gtk::EventControllerFocus::new();
                    let submitted_focus = submitted.clone();
                    focus_ctrl.connect_leave(move |_| {
                        if !submitted_focus.get() {
                            if let Some(s) = crate::model::SENDER.get() {
                                s.send(crate::model::AppMsg::Refresh).ok();
                            }
                        }
                    });
                    entry.add_controller(focus_ctrl);

                    let key_ctrl = gtk::EventControllerKey::new();
                    key_ctrl.set_propagation_phase(gtk::PropagationPhase::Capture);

                    let submitted_key = submitted.clone();
                    key_ctrl.connect_key_pressed(clone!(
                        #[weak]
                        entry,
                        #[upgrade_or]
                        glib::Propagation::Proceed,
                        move |_, keyval, _, _| {
                            if keyval == gdk::Key::Escape {
                                submitted_key.set(true);
                                if let Some(s) = crate::model::SENDER.get() {
                                    s.send(crate::model::AppMsg::Refresh).ok();
                                    return glib::Propagation::Stop;
                                }
                            } else if keyval == gdk::Key::Return || keyval == gdk::Key::KP_Enter {
                                submitted_key.set(true);
                                let target_path = unsafe {
                                    entry
                                        .data::<PathBuf>("target_rename_path")
                                        .map(|p| p.as_ref().clone())
                                };

                                if let Some(old_path) = target_path {
                                    let new_name = entry.text().to_string();
                                    if let Some(s) = crate::model::SENDER.get() {
                                        s.send(crate::model::AppMsg::PerformRename(
                                            old_path, new_name,
                                        ))
                                        .ok();
                                        return glib::Propagation::Stop;
                                    }
                                }
                            }
                            glib::Propagation::Proceed
                        }
                    ));
                    entry.add_controller(key_ctrl);

                    widgets.stack.add_named(&entry, Some(constants::VIEW_ENTRY));
                    entry
                }
            };

            // Update the data on existing entries if reused by the factory
            unsafe {
                entry.set_data("target_rename_path", self.path.clone());
            }

            widgets.stack.set_visible_child_name(constants::VIEW_ENTRY);
            entry.set_text(&self.name);
            let dot_pos = self.name.rfind('.').unwrap_or(self.name.len());
            entry.select_region(0, dot_pos as i32);

            gtk::glib::idle_add_local_once(clone!(
                #[weak]
                entry,
                move || {
                    entry.grab_focus();
                }
            ));
        } else {
            // Reclaim memory if a temporary Entry was previously instantiated
            if let Some(existing_entry) = widgets.stack.child_by_name(constants::VIEW_ENTRY) {
                widgets.stack.remove(&existing_entry);
            }
            // Ensure the label is visible when not editing.
            widgets.stack.set_visible_child_name(constants::VIEW_LABEL);
        }

        if let Some(ref texture) = self.thumbnail {
            widgets.icon_widget.set_paintable(Some(texture));
        } else {
            // Default: use the icon as-is
            widgets.icon_widget.set_pixel_size(self.icon_size);

            let display = root.display();
            let icon_theme = gtk::IconTheme::for_display(&display);
            let scale = root.scale_factor().max(1);
            let paintable = icon_theme.lookup_by_gicon(
                &self.icon,
                self.icon_size,
                scale,
                gtk::TextDirection::None,
                gtk::IconLookupFlags::PRELOAD,
            );
            widgets.icon_widget.set_paintable(Some(&paintable));
        }

        if config.ui.disable_drag_and_drop {
            widgets
                .drag_source
                .set_content(None::<&gdk::ContentProvider>);
            widgets.drop_target.set_actions(gdk::DragAction::empty());
        } else {
            let file = gtk::gio::File::for_path(&self.path);
            let uri = format!("{}\r\n", file.uri());
            let file_list_provider = gdk::ContentProvider::for_bytes(
                "text/uri-list",
                &glib::Bytes::from(uri.as_bytes()),
            );
            let file_provider = gdk::ContentProvider::for_value(&file.to_value());
            let content = gdk::ContentProvider::new_union(&[file_list_provider, file_provider]);
            widgets.drag_source.set_content(Some(&content));

            widgets.drop_target.set_actions(if self.is_dir {
                gdk::DragAction::COPY | gdk::DragAction::MOVE
            } else {
                gdk::DragAction::empty()
            });
        }

        // Update the shared cell with the current path so gestures can read it
        *self.active_path.borrow_mut() = Some(self.path.clone());

        // Store a clone of the Rc in the widget data so gestures can access the cell
        unsafe {
            root.set_data("active_path_cell", Rc::downgrade(&self.active_path));
            root.set_data("grid_item_index", self.grid_idx);
        }
    }
    /// Clears the per-cell lazy-thumbnail guard so the next item bound to this
    /// recycled widget cell can request its own thumbnail without being suppressed.
    fn unbind(&mut self, widgets: &mut Self::Widgets, root: &mut Self::Root) {
        if let Some(stream) = widgets.video_widget.media_stream() {
            stream.pause();
        }

        if let Some(ref provider) = widgets.scale_css_provider {
            widgets.label.style_context().remove_provider(provider);
            widgets.info_label.style_context().remove_provider(provider);
            widgets.scale_css_provider = None;
        }

        root.remove_css_class("flux-card--cut");

        widgets
            .video_widget
            .set_media_stream(None::<&gtk::MediaStream>);
        widgets.preview_stack.set_visible_child_name("icon");

        // Release the texture on both the widget and the item model
        widgets.icon_widget.set_paintable(None::<&gdk::Paintable>);
        widgets.icon_widget.clear();
        self.thumbnail = None;

        widgets
            .drag_source
            .set_content(None::<&gdk::ContentProvider>);
        widgets.drag_source.set_icon(None::<&gdk::Paintable>, 0, 0);
        widgets.label.set_text("");
        widgets.label.set_attributes(None);
        widgets.info_label.set_text("");

        root.remove_css_class("flux-card--symlink");
        root.remove_css_class("flux-card--broken-symlink");
        root.remove_css_class("flux-card--empty");

        if let Some(existing_entry) = widgets.stack.child_by_name(constants::VIEW_ENTRY) {
            widgets.stack.remove(&existing_entry);
        }
        *self.active_path.borrow_mut() = None;
        unsafe {
            let _ = root.steal_data::<std::rc::Weak<RefCell<Option<PathBuf>>>>("active_path_cell");
            let _ = root.steal_data::<u32>("lazy_thumb_requested");
            let _ = root.steal_data::<u32>("grid_item_index");
        }
    }
}

/// Output events emitted by a sidebar row.
#[derive(Debug)]
pub enum SidebarMsg {
    Navigate(PathBuf),
    Remove(PathBuf),
    ChangeIcon(PathBuf),
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
            add_css_class: constants::BREADCRUMB_BTN_CLASS,
            #[wrap(Some)]
            set_child = &gtk::Label {
                #[watch]
                set_label: &self.name,
                set_ellipsize: gtk::pango::EllipsizeMode::End,
                set_max_width_chars: constants::BREADCRUMB_MAX_WIDTH_CHARS as i32,
                set_wrap: false,
            },

            // Left-click: standard navigation
            connect_clicked[sender, path = self.path.clone()] => move |_| {
                let _ = sender.output(path.clone());
            },

            // Middle-click: add this specific breadcrumb folder to Quick List
            add_controller = gtk::GestureClick {
                set_button: 2, // constants::MOUSE_MIDDLE
                connect_pressed[path = self.path.clone()] => move |gesture, _, _, _| {
                    gesture.set_state(gtk::EventSequenceState::Claimed);
                    if let Some(s) = crate::model::SENDER.get() {
                        let _ = s.send(crate::model::AppMsg::AddExclusive(Some(path.clone())));
                    }
                }
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
                connect_pressed[path = self.path.clone(), is_label = self.is_section_label] => move |gesture, _, _, _| {
                    if is_label { return; }
                    gesture.set_state(gtk::EventSequenceState::Claimed);
                    if let Ok(exe) = std::env::current_exe() {
                        let _ = std::process::Command::new(exe).arg(&path).spawn();
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
                        menu_model.append(Some(&tr("Rename")), Some("sidebar.rename"));
                        menu_model.append(Some(&tr("Change icon")), Some("sidebar.change_icon"));
                        menu_model.append(Some(&tr("Remove from sidebar")), Some("sidebar.remove"));

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
                        set_icon_name: "media-eject",
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
