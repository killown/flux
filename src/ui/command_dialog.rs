use crate::model::AppMsg;
use crate::services::tasks::TaskQueue;
use adw::prelude::*;
use gtk::glib;
use gtk::pango;
use relm4::prelude::*;
use std::fs;
use std::sync::Arc;
use std::time::Instant;

// ─── libc helpers ────────────────────────────────────────────────────────────

fn page_size() -> usize {
    unsafe { libc::sysconf(libc::_SC_PAGESIZE) as usize }
}

fn clock_tick() -> f64 {
    unsafe { libc::sysconf(libc::_SC_CLK_TCK) as f64 }
}

// ─── /proc parsing helpers ──────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct ProcStat {
    pub pid: u32,
    pub ppid: u32,
    pub state: char,
    pub utime: f64,
    pub stime: f64,
    #[allow(dead_code)]
    pub nice: i32,
    pub num_threads: u64,
    pub rss: u64,
    #[allow(dead_code)]
    pub vms: u64,
}

fn parse_stat(pid: u32) -> Option<ProcStat> {
    let path = format!("/proc/{}/stat", pid);
    let content = fs::read_to_string(&path).ok()?;
    let _open_paren = content.find('(')?;
    let close_paren = content.rfind(')')?;
    let after_comm = &content[close_paren + 2..];
    let parts: Vec<&str> = after_comm.split_whitespace().collect();
    if parts.len() < 22 {
        return None;
    }
    let state = parts[0].chars().next()?;
    let ppid: u32 = parts[1].parse().ok()?;
    let utime: u64 = parts[11].parse().ok()?;
    let stime: u64 = parts[12].parse().ok()?;
    let nice: i32 = parts[16].parse().ok()?;
    let num_threads: u64 = parts[17].parse().ok()?;
    let rss_pages: u64 = parts[21].parse().ok()?;

    let tick = clock_tick();
    let page_sz = page_size() as u64;
    Some(ProcStat {
        pid,
        ppid,
        state,
        utime: utime as f64 / tick,
        stime: stime as f64 / tick,
        nice,
        num_threads,
        rss: rss_pages * page_sz,
        vms: 0,
    })
}

fn parse_statm(pid: u32) -> Option<u64> {
    let path = format!("/proc/{}/statm", pid);
    let content = fs::read_to_string(&path).ok()?;
    let mut parts = content.split_whitespace();
    let vms_pages: u64 = parts.next()?.parse().ok()?;
    let page_sz = page_size() as u64;
    Some(vms_pages * page_sz)
}

fn get_cmdline(pid: u32) -> Option<String> {
    let path = format!("/proc/{}/cmdline", pid);
    fs::read_to_string(&path).ok().and_then(|s| {
        let parts: Vec<&str> = s.split('\0').filter(|p| !p.is_empty()).collect();
        if parts.is_empty() {
            None
        } else {
            Some(parts.join(" "))
        }
    })
}

fn count_fds(pid: u32) -> Option<usize> {
    let path = format!("/proc/{}/fd", pid);
    fs::read_dir(&path).ok().map(|entries| entries.count())
}

fn get_cwd(pid: u32) -> Option<String> {
    let path = format!("/proc/{}/cwd", pid);
    fs::read_link(&path)
        .ok()
        .and_then(|p| p.to_str().map(|s| s.to_owned()))
}

fn state_name(ch: char) -> String {
    match ch {
        'R' => crate::i18n::tr("Running"),
        'S' => crate::i18n::tr("Sleeping"),
        'D' => crate::i18n::tr("Uninterruptible Sleep"),
        'Z' => crate::i18n::tr("Zombie"),
        'T' => crate::i18n::tr("Stopped"),
        't' => crate::i18n::tr("Tracing Stop"),
        'X' | 'x' => crate::i18n::tr("Dead"),
        _ => crate::i18n::tr("Unknown"),
    }
}

// ─── Formatting helpers ─────────────────────────────────────────────────────

fn format_duration(secs: f64) -> String {
    let total = secs as u64;
    let h = total / 3600;
    let m = (total % 3600) / 60;
    let s = total % 60;
    if h > 0 {
        format!("{:02}:{:02}:{:02}", h, m, s)
    } else {
        format!("{:02}:{:02}", m, s)
    }
}

fn format_bytes(bytes: u64) -> String {
    const KB: u64 = 1_024;
    const MB: u64 = 1_024 * KB;
    const GB: u64 = 1_024 * MB;
    match bytes {
        b if b >= GB => format!("{:.2} GB", b as f64 / GB as f64),
        b if b >= MB => format!("{:.2} MB", b as f64 / MB as f64),
        b if b >= KB => format!("{:.2} KB", b as f64 / KB as f64),
        b => format!("{} B", b),
    }
}

fn get_io_stats(pid: u32) -> Option<String> {
    let path = format!("/proc/{}/io", pid);
    let content = fs::read_to_string(&path).ok()?;
    let mut stats = Vec::new();
    for line in content.lines() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 2 {
            continue;
        }
        let key = parts[0];
        let value: u64 = parts[1].parse().ok()?;
        match key {
            "rchar:" => stats.push(("rchar", value, "chars read")),
            "wchar:" => stats.push(("wchar", value, "chars written")),
            "syscr:" => stats.push(("syscr", value, "read syscalls")),
            "syscw:" => stats.push(("syscw", value, "write syscalls")),
            "read_bytes:" => stats.push(("read_bytes", value, "bytes read")),
            "write_bytes:" => stats.push(("write_bytes", value, "bytes written")),
            "cancelled_write_bytes:" => {
                stats.push(("cancelled_write_bytes", value, "cancelled writes"))
            }
            _ => {}
        }
    }
    if stats.is_empty() {
        return None;
    }
    let lines: Vec<String> = stats
        .into_iter()
        .map(|(key, val, _)| {
            let disp = if key == "rchar"
                || key == "wchar"
                || key == "read_bytes"
                || key == "write_bytes"
                || key == "cancelled_write_bytes"
            {
                format_bytes(val)
            } else {
                val.to_string()
            };
            format!("{}: {}", key, disp)
        })
        .collect();
    Some(lines.join("  │  "))
}

fn copy_to_clipboard(text: &str) {
    if let Some(display) = adw::gdk::Display::default() {
        display.clipboard().set_text(text);
    }
}

// ─── Handle ──────────────────────────────────────────────────────────────────

#[derive(Debug)]
pub struct CommandDialogHandle {
    #[allow(dead_code)]
    window: adw::Window,
    text_view: gtk::TextView,
    pub task_id: u64,
    #[allow(dead_code)]
    action_name: Option<String>,
    no_dialog_switch: gtk::Switch,
    #[allow(dead_code)]
    sender: relm4::Sender<AppMsg>,
}

impl CommandDialogHandle {
    pub fn append_output(&self, line: &str) {
        let buffer = self.text_view.buffer();
        let mut end_iter = buffer.end_iter();
        buffer.insert(&mut end_iter, &format!("{}\n", line));
        let mark = buffer.create_mark(None, &buffer.end_iter(), false);
        self.text_view.scroll_to_mark(&mark, 0.0, false, 0.0, 1.0);
    }

    #[allow(dead_code)]
    pub fn close(&mut self) {
        self.window.close();
    }

    pub fn update_switch_state(&self, active: bool) {
        self.no_dialog_switch.set_active(active);
    }
}

// ─── Factory ──────────────────────────────────────────────────────────────────

pub fn create_command_dialog(
    task_id: u64,
    queue: Arc<TaskQueue>,
    sender: relm4::Sender<AppMsg>,
) -> CommandDialogHandle {
    let window = adw::Window::builder()
        .title(crate::i18n::tr("Command Output"))
        .default_width(720)
        .default_height(650)
        .modal(true)
        .resizable(true)
        .build();

    if let Some(parent) = gtk::Application::default().active_window() {
        window.set_transient_for(Some(&parent));
    }

    // ─── Header bar ──────────────────────────────────────────────────────────

    let header = adw::HeaderBar::new();
    header.set_show_end_title_buttons(true);

    // Get action_name from task for the switch
    let action_name = queue
        .snapshot()
        .iter()
        .find(|(id, _)| *id == task_id)
        .and_then(|(_, task)| task.action_name.clone());

    let no_dialog_switch = gtk::Switch::new();
    no_dialog_switch.set_active(false);

    // ─── Pack switch on the LEFT side with tooltip ────────────────────────

    let switch_box = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(6)
        .build();

    let switch_label = gtk::Label::builder()
        .label(crate::i18n::tr("Suppress dialog"))
        .css_classes(["caption"])
        .build();

    switch_label.set_tooltip_text(Some(
        &crate::i18n::tr("When enabled, this command will not show the output dialog again. This setting is saved in your menu configuration."),
    ));
    no_dialog_switch.set_tooltip_text(Some(&crate::i18n::tr(
        "Toggle to suppress the dialog for this specific command.",
    )));

    switch_box.append(&switch_label);
    switch_box.append(&no_dialog_switch);
    header.pack_start(&switch_box);

    // ─── Title widget (centered) ───────────────────────────────────────────

    let title_label = gtk::Label::builder()
        .label(crate::i18n::tr("Command Output"))
        .halign(gtk::Align::Center)
        .css_classes(["title-4"])
        .build();
    header.set_title_widget(Some(&title_label));

    // ─── Cancel button on the RIGHT side ──────────────────────────────────

    let cancel_btn = gtk::Button::builder()
        .label(crate::i18n::tr("Cancel"))
        .css_classes(["destructive-action"])
        .build();
    header.pack_end(&cancel_btn);

    // ─── Rest of the content ───────────────────────────────────────────────

    let content_box = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(0)
        .build();
    content_box.append(&header);

    // Info panel
    let info_box = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(4)
        .margin_top(12)
        .margin_bottom(8)
        .margin_start(16)
        .margin_end(16)
        .build();

    let pid_label = gtk::Label::builder()
        .xalign(0.0)
        .selectable(true)
        .css_classes(["caption"])
        .build();

    let state_label = gtk::Label::builder()
        .xalign(0.0)
        .selectable(true)
        .css_classes(["caption"])
        .build();

    let cpu_label = gtk::Label::builder()
        .xalign(0.0)
        .selectable(true)
        .css_classes(["caption"])
        .build();

    let mem_label = gtk::Label::builder()
        .xalign(0.0)
        .selectable(true)
        .css_classes(["caption"])
        .build();

    let threads_label = gtk::Label::builder()
        .xalign(0.0)
        .selectable(true)
        .css_classes(["caption"])
        .build();

    let fds_label = gtk::Label::builder()
        .xalign(0.0)
        .selectable(true)
        .css_classes(["caption"])
        .build();

    let cwd_label = gtk::Label::builder()
        .xalign(0.0)
        .selectable(true)
        .wrap(true)
        .css_classes(["caption", "dim-label"])
        .build();

    let io_label = gtk::Label::builder()
        .xalign(0.0)
        .selectable(true)
        .wrap(true)
        .css_classes(["caption", "dim-label"])
        .build();

    let cmd_label = gtk::Label::builder()
        .xalign(0.0)
        .selectable(true)
        .wrap(false)
        .ellipsize(pango::EllipsizeMode::Middle)
        .css_classes(["caption", "dim-label"])
        .build();

    // ─── Copy button ─────────────────────────────────────────────────────────

    let copy_button = gtk::Button::builder()
        .icon_name("edit-copy-symbolic")
        .tooltip_text(crate::i18n::tr("Copy all info to clipboard"))
        .css_classes(["flat", "circular"])
        .halign(gtk::Align::End)
        .build();

    let cmd_row = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(6)
        .build();
    cmd_row.append(&cmd_label);
    cmd_row.append(&copy_button);

    info_box.append(&pid_label);
    info_box.append(&state_label);
    info_box.append(&cpu_label);
    info_box.append(&mem_label);
    info_box.append(&threads_label);
    info_box.append(&fds_label);
    info_box.append(&cwd_label);
    info_box.append(&io_label);
    info_box.append(&cmd_row);

    content_box.append(&info_box);

    // Output text view
    let text_view = gtk::TextView::builder()
        .editable(false)
        .monospace(true)
        .wrap_mode(gtk::WrapMode::WordChar)
        .margin_top(6)
        .margin_bottom(6)
        .margin_start(6)
        .margin_end(6)
        .build();

    let mut prev_cpu_time: f64 = 0.0;
    let mut last_update = Instant::now();

    if let Some((_, task)) = queue.snapshot().iter().find(|(id, _)| *id == task_id) {
        let buffer = text_view.buffer();
        buffer.set_text(&task.output.join("\n"));
        let pid = task.pid.unwrap_or(0);
        pid_label.set_text(&format!("{} {}", crate::i18n::tr("PID:"), pid));
    }

    let scrolled = gtk::ScrolledWindow::builder()
        .child(&text_view)
        .vexpand(true)
        .hexpand(true)
        .margin_start(12)
        .margin_end(12)
        .margin_bottom(12)
        .build();
    content_box.append(&scrolled);

    window.set_content(Some(&content_box));

    // ─── Signals ────────────────────────────────────────────────────────────

    // Cancel button
    let s_cancel = sender.clone();
    let window_cancel = window.clone();
    cancel_btn.connect_clicked(move |_| {
        let _ = s_cancel.send(AppMsg::CancelTask(task_id));
        window_cancel.close();
    });

    // Switch: toggle flag and close dialog if suppressing
    let sender_switch = sender.clone();
    let action_name_switch = action_name.clone();
    let window_switch = window.clone();
    no_dialog_switch.connect_state_set(move |_switch, state| {
        if let Some(ref name) = action_name_switch {
            let _ = sender_switch.send(AppMsg::ToggleNoCommandDialog(name.clone()));
            if state {
                // Hide the window instead of closing it, so the task keeps running
                let _ = sender_switch.send(AppMsg::CommandDialogClosed);
                window_switch.hide();
            }
        }
        glib::Propagation::Proceed
    });

    // Copy button
    let queue_for_copy = queue.clone();
    let task_id_copy = task_id;
    copy_button.connect_clicked(move |_| {
        if let Some((_, task)) = queue_for_copy
            .snapshot()
            .iter()
            .find(|(id, _)| *id == task_id_copy)
        {
            let pid = task.pid.unwrap_or(0);
            let cmd = get_cmdline(pid)
                .or_else(|| task.full_command.clone())
                .unwrap_or_else(|| task.label.clone());
            let io = get_io_stats(pid)
                .unwrap_or_else(|| crate::i18n::tr("I/O: (unavailable)").to_string());
            let cwd = get_cwd(pid).unwrap_or_else(|| crate::i18n::tr("N/A").to_string());

            let (state_str, cpu, mem, threads) = if let Some(s) = parse_stat(pid) {
                (
                    state_name(s.state),
                    format_duration(s.utime + s.stime),
                    format_bytes(s.rss),
                    s.num_threads.to_string(),
                )
            } else {
                (
                    crate::i18n::tr("Unknown"),
                    crate::i18n::tr("N/A"),
                    crate::i18n::tr("N/A"),
                    crate::i18n::tr("N/A"),
                )
            };
            let fds = count_fds(pid)
                .map_or_else(|| crate::i18n::tr("N/A").to_string(), |n| n.to_string());

            let full = format!(
                "{} {}\n{} {}\n{} {}\n{} {}\n{} {}\n{} {}\n{} {}\n{} {}\n{} {}",
                crate::i18n::tr("PID:"),
                pid,
                crate::i18n::tr("State:"),
                state_str,
                crate::i18n::tr("CPU time:"),
                cpu,
                crate::i18n::tr("Memory (RSS):"),
                mem,
                crate::i18n::tr("Threads:"),
                threads,
                crate::i18n::tr("Open FDs:"),
                fds,
                crate::i18n::tr("CWD:"),
                cwd,
                crate::i18n::tr("I/O:"),
                io,
                crate::i18n::tr("Command:"),
                cmd
            );
            copy_to_clipboard(&full);
        }
    });

    // Timer
    let queue_clone = queue.clone();
    let task_id_clone = task_id;
    let window_weak = window.downgrade();

    let pid_label_timer = pid_label.clone();
    let state_label_timer = state_label.clone();
    let cpu_label_timer = cpu_label.clone();
    let mem_label_timer = mem_label.clone();
    let threads_label_timer = threads_label.clone();
    let fds_label_timer = fds_label.clone();
    let cwd_label_timer = cwd_label.clone();
    let io_label_timer = io_label.clone();
    let cmd_label_timer = cmd_label.clone();

    let sender_timer = sender.clone();

    glib::timeout_add_local(std::time::Duration::from_secs(1), move || {
        let Some(win) = window_weak.upgrade() else {
            return glib::ControlFlow::Break;
        };

        let still_active = queue_clone
            .snapshot()
            .iter()
            .any(|(id, _)| *id == task_id_clone);

        if !still_active {
            state_label_timer.set_text(&crate::i18n::tr("State: Completed / Terminated"));
            let _ = sender_timer.send(AppMsg::CommandDialogClosed);
            win.close();
            return glib::ControlFlow::Break;
        }

        let task = match queue_clone
            .snapshot()
            .iter()
            .find(|(id, _)| *id == task_id_clone)
        {
            Some((_, t)) => t.clone(),
            None => return glib::ControlFlow::Continue,
        };

        let pid_num = task.pid.unwrap_or(0);
        if pid_num == 0 {
            return glib::ControlFlow::Continue;
        }

        let stat = parse_stat(pid_num);
        let vms = parse_statm(pid_num).unwrap_or(0);
        let fd_count = count_fds(pid_num).unwrap_or(0);
        let cwd = get_cwd(pid_num).unwrap_or_else(|| crate::i18n::tr("N/A").to_string());
        let io_stats = get_io_stats(pid_num)
            .unwrap_or_else(|| crate::i18n::tr("I/O: (unavailable)").to_string());
        let cmd = get_cmdline(pid_num).unwrap_or_else(|| task.label.clone());

        cmd_label_timer.set_text(&cmd);
        cmd_label_timer.set_tooltip_text(Some(&cmd));

        if let Some(s) = stat {
            let state_str = state_name(s.state);
            let cpu_total = s.utime + s.stime;
            let cpu_delta = cpu_total - prev_cpu_time;
            let time_delta = last_update.elapsed().as_secs_f64();
            let cpu_pct = if time_delta > 0.0 && cpu_delta > 0.0 {
                (cpu_delta / time_delta) * 100.0
            } else {
                0.0
            };
            prev_cpu_time = cpu_total;
            last_update = Instant::now();

            pid_label_timer.set_text(&format!("{} {}", crate::i18n::tr("PID:"), s.pid));
            state_label_timer.set_text(&format!(
                "{} {} ({} {})",
                crate::i18n::tr("State:"),
                state_str,
                crate::i18n::tr("PPID:"),
                s.ppid
            ));
            cpu_label_timer.set_text(&format!(
                "{} {}  ({}% {})",
                crate::i18n::tr("CPU time:"),
                format_duration(cpu_total),
                (cpu_pct * 10.0).round() / 10.0,
                crate::i18n::tr("user+sys"),
            ));
            mem_label_timer.set_text(&format!(
                "{} {} / {}",
                crate::i18n::tr("Memory (RSS/VMS):"),
                format_bytes(s.rss),
                format_bytes(vms)
            ));
            threads_label_timer.set_text(&format!(
                "{} {}",
                crate::i18n::tr("Threads:"),
                s.num_threads
            ));
        } else {
            pid_label_timer.set_text(&format!("{} {}", crate::i18n::tr("PID:"), pid_num));
            state_label_timer.set_text(&crate::i18n::tr("State: (unavailable)"));
        }

        fds_label_timer.set_text(&format!("{} {}", crate::i18n::tr("Open FDs:"), fd_count));
        cwd_label_timer.set_text(&format!("{} {}", crate::i18n::tr("CWD:"), cwd));
        io_label_timer.set_text(&format!("{} {}", crate::i18n::tr("I/O:"), io_stats));

        glib::ControlFlow::Continue
    });

    // Close request
    let s_close = sender.clone();
    window.connect_close_request(move |_| {
        let _ = s_close.send(AppMsg::CommandDialogClosed);
        let _ = s_close.send(AppMsg::CancelTask(task_id));
        glib::Propagation::Proceed
    });

    window.present();

    CommandDialogHandle {
        window,
        text_view,
        task_id,
        action_name,
        no_dialog_switch,
        sender,
    }
}
