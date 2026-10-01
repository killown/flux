use crate::i18n::tr;
use crate::model::{AppMsg, FluxApp};
use adw::prelude::*;
use relm4::AsyncComponentSender;

/// Highlights git diff lines with color tags inside the buffer.
pub fn apply_diff_markup(buffer: &gtk::TextBuffer, raw_diff: &str) {
    buffer.set_text("");
    let tag_table = buffer.tag_table();

    if tag_table.lookup("diff_add").is_none() {
        let tag_add = gtk::TextTag::builder()
            .name("diff_add")
            .foreground("#57ab5a")
            .build();
        let tag_del = gtk::TextTag::builder()
            .name("diff_del")
            .foreground("#e5534b")
            .build();
        let tag_hdr = gtk::TextTag::builder()
            .name("diff_hdr")
            .foreground("#76e3ea")
            .weight(700)
            .build();
        let tag_hunk = gtk::TextTag::builder()
            .name("diff_hunk_link")
            .foreground("#76e3ea")
            .underline(gtk::pango::Underline::Single)
            .weight(700)
            .build();

        tag_table.add(&tag_add);
        tag_table.add(&tag_del);
        tag_table.add(&tag_hdr);
        tag_table.add(&tag_hunk);
    }

    for line in raw_diff.lines() {
        let mut end = buffer.end_iter();
        let tag_name = if line.starts_with('+') && !line.starts_with("+++") {
            Some("diff_add")
        } else if line.starts_with('-') && !line.starts_with("---") {
            Some("diff_del")
        } else if line.starts_with("@@") {
            Some("diff_hunk_link")
        } else if line.starts_with("diff --git") {
            Some("diff_hdr")
        } else {
            None
        };

        if let Some(tag) = tag_name {
            buffer.insert_with_tags_by_name(&mut end, &format!("{}\n", line), &[tag]);
        } else {
            buffer.insert(&mut end, &format!("{}\n", line));
        }
    }
}

/// Parses the start line number from a hunk header `@@ -a,b +c,d @@`.
fn parse_hunk_target_line(line: &str) -> Option<usize> {
    let minus_idx = line.find('-')?;
    let rest = &line[minus_idx + 1..];
    let num_str: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
    num_str.parse().ok()
}

/// Cleans a diff added line into a safe, distinctive search token for Vim.
fn sanitize_diff_query(raw_text: &str) -> Option<String> {
    let without_diff_marker = raw_text.strip_prefix('+').unwrap_or(raw_text).trim();

    let stripped = without_diff_marker
        .trim_start_matches("///")
        .trim_start_matches("//!")
        .trim_start_matches("//")
        .trim_start_matches("/*")
        .trim_start_matches("*/")
        .trim_start_matches('*')
        .trim_start_matches('#')
        .trim();

    if stripped.is_empty() {
        return None;
    }

    let mut escaped = String::with_capacity(stripped.len() + 8);
    for ch in stripped.chars() {
        match ch {
            '/' | '\\' | '[' | ']' | '^' | '$' | '.' | '*' | '~' => {
                escaped.push('\\');
                escaped.push(ch);
            }
            _ => escaped.push(ch),
        }
    }

    if escaped.trim().is_empty() {
        None
    } else {
        Some(escaped)
    }
}

/// Finds the hunk header line and extracts its start line and the sanitized search token.
fn find_hunk_info_from_iter(iter: &gtk::TextIter) -> Option<(usize, Option<String>)> {
    let mut current = *iter;

    loop {
        let mut line_start = current;
        line_start.set_line_offset(0);
        let mut line_end = line_start;
        line_end.forward_to_line_end();

        let line_text = current.buffer().text(&line_start, &line_end, false);
        if line_text.starts_with("@@") {
            let start_line = parse_hunk_target_line(&line_text)?;

            let mut scan = line_start;
            let mut query = None;

            while scan.forward_line() {
                let mut s_start = scan;
                s_start.set_line_offset(0);
                let mut s_end = s_start;
                s_end.forward_to_line_end();

                let text = scan.buffer().text(&s_start, &s_end, false);

                if text.starts_with("@@") || text.starts_with("diff --git") {
                    break;
                }

                if text.starts_with('+') && !text.starts_with("+++") {
                    if let Some(cleaned) = sanitize_diff_query(&text) {
                        query = Some(cleaned);
                        break;
                    }
                }
            }

            return Some((start_line, query));
        }

        if line_start.line() == 0 || !current.backward_line() {
            break;
        }
    }
    None
}

/// Constructs the sliding git diff sidebar panel with a resizable handle.
pub fn build_diff_panel(
    initial_width: i32,
    text_buffer: gtk::TextBuffer,
    sender: AsyncComponentSender<FluxApp>,
) -> gtk::Box {
    let effective_width = if initial_width <= 0 {
        420
    } else {
        initial_width.clamp(280, 900)
    };

    let panel = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(0)
        .width_request(effective_width)
        .hexpand(false)
        .build();

    panel.add_css_class("sidebar");

    let resize_handle = gtk::Separator::builder()
        .orientation(gtk::Orientation::Vertical)
        .css_classes(["sidebar-resize-handle"])
        .build();

    let drag_gesture = gtk::GestureDrag::new();
    let start_width = std::rc::Rc::new(std::cell::Cell::new(effective_width));
    let start_root_x = std::rc::Rc::new(std::cell::Cell::new(0.0));
    let hover_timer = std::rc::Rc::new(std::cell::Cell::new(None::<gtk::glib::SourceId>));
    let is_ready = std::rc::Rc::new(std::cell::Cell::new(false));
    let motion_ctrl = gtk::EventControllerMotion::new();

    {
        let timer_c = hover_timer.clone();
        let ready_c = is_ready.clone();
        motion_ctrl.connect_enter(move |ctrl, _, _| {
            if let Some(id) = timer_c.take() {
                id.remove();
            }
            ready_c.set(false);
            let ctrl_weak = ctrl.downgrade();
            let timer_inner = timer_c.clone();
            let ready_inner = ready_c.clone();

            let id = gtk::glib::timeout_add_local_once(
                std::time::Duration::from_millis(100),
                move || {
                    timer_inner.set(None);
                    ready_inner.set(true);
                    if let Some(c) = ctrl_weak.upgrade() {
                        if let Some(widget) = c.widget() {
                            widget.set_cursor_from_name(Some("col-resize"));
                        }
                    }
                },
            );
            timer_c.set(Some(id));
        });
    }

    {
        let timer_c = hover_timer;
        let ready_c = is_ready.clone();
        motion_ctrl.connect_leave(move |ctrl| {
            if let Some(id) = timer_c.take() {
                id.remove();
            }
            ready_c.set(false);
            if let Some(widget) = ctrl.widget() {
                widget.set_cursor(None);
            }
        });
    }

    resize_handle.add_controller(motion_ctrl);

    {
        let panel_weak = panel.downgrade();
        let start_width_c = start_width.clone();
        let start_root_x_c = start_root_x.clone();
        let ready_c = is_ready.clone();

        drag_gesture.connect_drag_begin(move |gesture, x, _| {
            if !ready_c.get() {
                gesture.set_state(gtk::EventSequenceState::Denied);
                return;
            }
            if let Some(p) = panel_weak.upgrade() {
                start_width_c.set(p.width());
                if let Some(root) = p.root() {
                    if let Some(handle) = gesture.widget() {
                        let (rx, _) = handle
                            .translate_coordinates(&root, x, 0.0)
                            .unwrap_or((x, 0.0));
                        start_root_x_c.set(rx);
                    }
                }
            }
        });
    }

    {
        let panel_weak = panel.downgrade();
        let start_width_c = start_width.clone();
        let start_root_x_c = start_root_x.clone();
        let ready_c = is_ready.clone();

        drag_gesture.connect_drag_update(move |gesture, _, _| {
            if !ready_c.get() {
                return;
            }
            if let Some(p) = panel_weak.upgrade() {
                if let Some(root) = p.root() {
                    if let Some(handle) = gesture.widget() {
                        if let Some((curr_x, _)) = gesture.point(None) {
                            if let Some((curr_root_x, _)) =
                                handle.translate_coordinates(&root, curr_x, 0.0)
                            {
                                let delta_x = curr_root_x - start_root_x_c.get();
                                let new_w = (start_width_c.get() - delta_x as i32).clamp(280, 900);
                                if p.width_request() != new_w {
                                    p.set_width_request(new_w);
                                }
                            }
                        }
                    }
                }
            }
        });
    }

    {
        let panel_weak = panel.downgrade();
        let s = sender.clone();
        let ready_c = is_ready;

        drag_gesture.connect_drag_end(move |gesture, _, _| {
            if !ready_c.get() {
                return;
            }
            if let Some(p) = panel_weak.upgrade() {
                let final_width = p.width().clamp(280, 900);
                p.set_width_request(final_width);
                s.input(AppMsg::SetDiffPanelWidth(final_width));
            }
            if let Some(widget) = gesture.widget() {
                widget.set_cursor(None);
            }
        });
    }

    resize_handle.add_controller(drag_gesture);

    let header_box = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(8)
        .margin_start(12)
        .margin_end(12)
        .margin_top(8)
        .margin_bottom(6)
        .build();

    let diff_icon = gtk::Image::from_icon_name("text-x-generic-symbolic");

    let title_label = gtk::Label::builder()
        .label(tr("Git Diff"))
        .css_classes(["heading"])
        .hexpand(true)
        .xalign(0.0)
        .build();

    let copy_btn = gtk::Button::builder()
        .icon_name("edit-copy-symbolic")
        .css_classes(["flat", "circular"])
        .valign(gtk::Align::Center)
        .tooltip_text(tr("Copy Diff to Clipboard"))
        .build();

    {
        let buf = text_buffer.clone();
        let s = sender.clone();
        copy_btn.connect_clicked(move |btn| {
            let start = buf.start_iter();
            let end = buf.end_iter();
            let diff_text = buf.text(&start, &end, false);
            if !diff_text.is_empty() {
                let display = btn.display();
                display.clipboard().set_text(&diff_text);
                s.input(AppMsg::ShowToast(tr("Diff copied to clipboard")));
            }
        });
    }

    let close_btn = gtk::Button::builder()
        .icon_name("window-close-symbolic")
        .css_classes(["flat", "circular"])
        .valign(gtk::Align::Center)
        .tooltip_text(tr("Close"))
        .build();

    {
        let s = sender.clone();
        close_btn.connect_clicked(move |_| {
            s.input(AppMsg::ToggleDiffPanel);
        });
    }

    header_box.append(&diff_icon);
    header_box.append(&title_label);
    header_box.append(&copy_btn);
    header_box.append(&close_btn);
    panel.append(&header_box);

    let text_view = gtk::TextView::builder()
        .buffer(&text_buffer)
        .editable(false)
        .monospace(true)
        .wrap_mode(gtk::WrapMode::WordChar)
        .top_margin(8)
        .bottom_margin(8)
        .left_margin(12)
        .right_margin(12)
        .hexpand(true)
        .vexpand(true)
        .css_classes(["diff-view"])
        .build();

    let click_gesture = gtk::GestureClick::new();
    {
        let s = sender.clone();
        let tv_weak = text_view.downgrade();
        click_gesture.connect_released(move |gesture, _, x, y| {
            if gesture.current_button() != 1 {
                return;
            }
            let Some(tv) = tv_weak.upgrade() else { return };

            let (bx, by) =
                tv.window_to_buffer_coords(gtk::TextWindowType::Text, x as i32, y as i32);

            let Some(iter) = tv.iter_at_location(bx, by) else {
                return;
            };

            if let Some((start_line, query)) = find_hunk_info_from_iter(&iter) {
                s.input(AppMsg::OpenActiveDiffLine {
                    line: start_line,
                    query,
                });
            }
        });
    }
    text_view.add_controller(click_gesture);

    let motion_controller = gtk::EventControllerMotion::new();
    {
        let tv_weak = text_view.downgrade();
        motion_controller.connect_motion(move |_, x, y| {
            let Some(tv) = tv_weak.upgrade() else { return };
            let (bx, by) =
                tv.window_to_buffer_coords(gtk::TextWindowType::Text, x as i32, y as i32);
            let is_hunk = tv
                .iter_at_location(bx, by)
                .map(|iter| {
                    let mut ls = iter;
                    ls.set_line_offset(0);
                    let mut le = ls;
                    le.forward_to_line_end();
                    tv.buffer().text(&ls, &le, false).starts_with("@@")
                })
                .unwrap_or(false);

            if is_hunk {
                tv.set_cursor_from_name(Some("pointer"));
            } else {
                tv.set_cursor_from_name(Some("text"));
            }
        });
    }
    text_view.add_controller(motion_controller);

    let scrolled = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .vscrollbar_policy(gtk::PolicyType::Automatic)
        .propagate_natural_width(false)
        .vexpand(true)
        .hexpand(true)
        .child(&text_view)
        .build();

    panel.append(&scrolled);

    let root_container = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(0)
        .hexpand(false)
        .halign(gtk::Align::End)
        .build();

    root_container.append(&resize_handle);
    root_container.append(&panel);
    root_container
}
