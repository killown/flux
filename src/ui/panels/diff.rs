use super::header::panel_header;
use super::resize::resizable_panel;
use super::spec::PanelSpec;
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

fn parse_hunk_target_line(line: &str) -> Option<usize> {
    let minus_idx = line.find('-')?;
    let rest = &line[minus_idx + 1..];
    let num_str: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
    num_str.parse().ok()
}

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
    let spec = PanelSpec::diff().with_initial(initial_width);

    let panel = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(0)
        .width_request(spec.effective())
        .hexpand(false)
        .build();
    panel.add_css_class("sidebar");

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

    {
        let s = sender.clone();
        let header = panel_header(
            Some("text-x-generic-symbolic"),
            &tr("Git Diff"),
            &[copy_btn.upcast::<gtk::Widget>()],
            move || s.input(AppMsg::ToggleDiffPanel),
        );
        panel.append(&header);
    }

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

    let sender_for_resize = sender.clone();
    resizable_panel(&spec, &panel, move |w| {
        sender_for_resize.input(AppMsg::SetDiffPanelWidth(w));
    })
}
