use super::file_item::FileItem;
use super::widgets::FileWidgets;
use crate::i18n::tr;
use crate::ui::constants;
use adw::gdk;
use adw::prelude::*;
use gtk::glib;
use gtk::glib::clone;
use relm4::prelude::*;
use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

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

impl FileItem {
    /// Updates the item's widgets with current data from the [FileItem] model.
    ///
    /// Synchronizes labels, icons, thumbnails, and visibility states (e.g., rename entry).
    pub(super) fn bind_widgets(&mut self, widgets: &mut FileWidgets, root: &mut gtk::Overlay) {
        crate::hit!("bind");
        widgets.label.set_label(&self.display_label);

        if self.is_cut {
            root.add_css_class("flux-card--cut");
            widgets.card_box.add_css_class("flux-card--cut");
        } else {
            root.remove_css_class("flux-card--cut");
            widgets.card_box.remove_css_class("flux-card--cut");
        }

        if self.is_copy {
            root.add_css_class("flux-card--copy");
            widgets.card_box.add_css_class("flux-card--copy");
        } else {
            root.remove_css_class("flux-card--copy");
            widgets.card_box.remove_css_class("flux-card--copy");
        }

        if self.is_list_mode {
            crate::hit!("bind_layout_list");
            // Compact horizontal row: small icon on the left, filename fills the rest.
            widgets
                .card_box
                .set_orientation(gtk::Orientation::Horizontal);
            widgets.card_box.set_halign(gtk::Align::Fill);
            widgets.card_box.set_valign(gtk::Align::Center);
            widgets.card_box.set_hexpand(true);
            widgets.card_box.set_vexpand(false);
            widgets.card_box.set_spacing(10);

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

            if self.scale_font_with_icons && self.icon_size > 0 {
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
                crate::hit!("bind_info_label");
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
            crate::hit!("bind_layout_grid");
            widgets.card_box.set_orientation(gtk::Orientation::Vertical);
            widgets.card_box.set_halign(gtk::Align::Fill);
            widgets.card_box.set_valign(gtk::Align::Fill);
            widgets.card_box.set_hexpand(false);
            widgets.card_box.set_vexpand(false);
            widgets.card_box.set_spacing(0);
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

            if self.scale_font_with_icons && self.icon_size > 0 {
                let base_size = if self.default_icon_size > 0 {
                    self.default_icon_size as f64
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

        // WARNING: never use `set_widget_name(&self.path.to_string_lossy())` here.
        // `bind()` runs on every visible item on each scroll/zoom/layout pass.
        // Setting the widget name dirties the CSS node and forces GTK's style
        // engine to re-run a full CSS selector match for the entire grid on
        // every bind, which caused severe scrollbar lag in GridView mode with
        // thousands of items. Opaque widget data via `set_data` bypasses the
        // CSS engine entirely and is the correct, high-performance approach.
        crate::hit!("bind_flux_path_data");
        unsafe {
            root.set_data("flux_path", self.path.clone());
            widgets.card_box.set_data("flux_path", self.path.clone());
        }

        crate::hit!("bind_emblem_classes");
        if self.is_foreign_owner {
            root.add_css_class("flux-card--restricted");
            widgets.card_box.add_css_class("flux-card--restricted");
            widgets.lock_icon.set_visible(true);
        } else {
            root.remove_css_class("flux-card--restricted");
            widgets.card_box.remove_css_class("flux-card--restricted");
            widgets.lock_icon.set_visible(false);
        }

        let show_indicator = self.show_symlink_emblem && self.is_symlink;

        if show_indicator {
            if self.is_broken_symlink {
                root.add_css_class("flux-card--broken-symlink");
                root.remove_css_class("flux-card--symlink");
                widgets.card_box.add_css_class("flux-card--broken-symlink");
                widgets.card_box.remove_css_class("flux-card--symlink");
            } else {
                root.add_css_class("flux-card--symlink");
                root.remove_css_class("flux-card--broken-symlink");
                widgets.card_box.add_css_class("flux-card--symlink");
                widgets
                    .card_box
                    .remove_css_class("flux-card--broken-symlink");
            }
        } else {
            root.remove_css_class("flux-card--symlink");
            root.remove_css_class("flux-card--broken-symlink");
            widgets.card_box.remove_css_class("flux-card--symlink");
            widgets
                .card_box
                .remove_css_class("flux-card--broken-symlink");
        }

        if self.is_dir && self.show_empty_dir_emblem {
            if self.is_empty {
                root.add_css_class("flux-card--empty");
                widgets.card_box.add_css_class("flux-card--empty");
            } else {
                root.remove_css_class("flux-card--empty");
                widgets.card_box.remove_css_class("flux-card--empty");
            }
        } else {
            root.remove_css_class("flux-card--empty");
            widgets.card_box.remove_css_class("flux-card--empty");
        }

        crate::hit!("bind_git_badge");
        if let Some((emblem, class_name)) = self.git_status.badge_info() {
            widgets.git_badge.set_label(emblem);
            widgets
                .git_badge
                .set_css_classes(&["flux-git-badge", class_name]);
            widgets.git_badge.set_visible(true);
        } else {
            widgets.git_badge.set_visible(false);
        }

        if self.is_editing {
            crate::hit!("bind_rename_entry");
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
            crate::hit!("bind_rename_teardown");
            // Reclaim memory if a temporary Entry was previously instantiated
            if let Some(existing_entry) = widgets.stack.child_by_name(constants::VIEW_ENTRY) {
                widgets.stack.remove(&existing_entry);
            }
            // Ensure the label is visible when not editing.
            widgets.stack.set_visible_child_name(constants::VIEW_LABEL);
        }

        if let Some(ref texture) = self.thumbnail {
            crate::hit!("bind_thumbnail_paint");
            widgets.icon_widget.set_paintable(Some(texture));
        } else {
            crate::hit!("bind_icon_paint");

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
                gtk::IconLookupFlags::empty(),
            );
            widgets.icon_widget.set_paintable(Some(&paintable));
        }

        if self.disable_drag_and_drop {
            crate::hit!("bind_dnd_disabled");
            widgets
                .drag_source
                .set_content(None::<&gdk::ContentProvider>);
            widgets.drop_target.set_actions(gdk::DragAction::empty());
        } else {
            crate::hit!("bind_dnd_payload");
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

        crate::hit!("bind_path_cells");
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
    pub(super) fn unbind_widgets(&mut self, widgets: &mut FileWidgets, root: &mut gtk::Overlay) {
        crate::hit!("unbind");
        if let Some(stream) = widgets.video_widget.media_stream() {
            stream.pause();

            let video_weak = widgets.video_widget.downgrade();
            let stream_to_clear = stream.clone();
            glib::timeout_add_local_once(std::time::Duration::from_millis(100), move || {
                if let Some(video) = video_weak.upgrade() {
                    if video
                        .media_stream()
                        .map(|s| s == stream_to_clear)
                        .unwrap_or(false)
                    {
                        video.set_media_stream(None::<&gtk::MediaStream>);
                    }
                }
            });
        }

        if let Some(ref provider) = widgets.scale_css_provider {
            widgets.label.style_context().remove_provider(provider);
            widgets.info_label.style_context().remove_provider(provider);
            widgets.scale_css_provider = None;
        }

        root.remove_css_class("flux-card--cut");
        root.remove_css_class("flux-card--copy");
        root.remove_css_class("flux-card--restricted");
        widgets.card_box.remove_css_class("flux-card--cut");
        widgets.card_box.remove_css_class("flux-card--copy");
        widgets.card_box.remove_css_class("flux-card--restricted");

        widgets.preview_stack.set_visible_child_name("icon");

        widgets.git_badge.set_visible(false);
        widgets.git_badge.set_css_classes(&["flux-git-badge"]);

        // Release the texture on both the widget and the item model
        widgets.icon_widget.set_paintable(None::<&gdk::Paintable>);
        widgets.icon_widget.clear();

        widgets
            .drag_source
            .set_content(None::<&gdk::ContentProvider>);
        widgets.drag_source.set_icon(None::<&gdk::Paintable>, 0, 0);
        widgets.label.set_text("");
        widgets.label.set_attributes(None);
        widgets.info_label.set_text("");
        widgets.info_label.set_attributes(None);
        widgets.info_label.set_tooltip_text(None::<&str>);
        widgets.info_label.set_visible(false);

        root.remove_css_class("flux-card--symlink");
        root.remove_css_class("flux-card--broken-symlink");
        root.remove_css_class("flux-card--empty");
        widgets.card_box.remove_css_class("flux-card--symlink");
        widgets
            .card_box
            .remove_css_class("flux-card--broken-symlink");
        widgets.card_box.remove_css_class("flux-card--empty");

        if let Some(existing_entry) = widgets.stack.child_by_name(constants::VIEW_ENTRY) {
            widgets.stack.remove(&existing_entry);
        }
        *self.active_path.borrow_mut() = None;
        unsafe {
            let _ = root.steal_data::<std::rc::Weak<RefCell<Option<PathBuf>>>>("active_path_cell");
            let _ = root.steal_data::<u32>("lazy_thumb_requested");
            let _ = root.steal_data::<u32>("grid_item_index");
            let _ = root.steal_data::<PathBuf>("flux_path");
            let _ = widgets.card_box.steal_data::<PathBuf>("flux_path");
        }
    }
}
