//! Transfer progress dialog.
//!
//! Shows a tabbed Libadwaita window with one tab per active file operation.

use crate::model::AppMsg;
use crate::services::tasks::{format_bytes, format_duration, TaskQueue};
use adw::prelude::*;
use gtk::glib;
use gtk::pango;
use relm4::prelude::*;
use std::sync::Arc;

const REFRESH_INTERVAL_MS: u32 = 250;

// ─── Handle ──────────────────────────────────────────────────────────────────

pub struct TransferDialogHandle {
    window: adw::Window,
    notebook: gtk::Notebook,
    queue: Arc<TaskQueue>,
    ticker_id: Option<glib::SourceId>,
    sender: relm4::Sender<AppMsg>,
}

impl std::fmt::Debug for TransferDialogHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TransferDialogHandle").finish()
    }
}

impl TransferDialogHandle {
    #[allow(dead_code)]
    pub fn present(&self) {
        self.window.present();
    }

    pub fn refresh(&mut self) {
        let snapshot = self.queue.snapshot();
        if snapshot.is_empty() {
            self.close();
            return;
        }
        rebuild_tabs(&self.notebook, &snapshot, &self.queue, &self.sender);
    }

    pub fn close(&mut self) {
        if let Some(id) = self.ticker_id.take() {
            id.remove();
        }
        self.window.close();
    }
}

// ─── Factory ──────────────────────────────────────────────────────────────────

pub fn create_transfer_dialog(
    queue: Arc<TaskQueue>,
    sender: relm4::Sender<AppMsg>,
) -> TransferDialogHandle {
    let window = adw::Window::builder()
        .title(crate::i18n::tr("File Transfer"))
        .default_width(480)
        .default_height(260)
        .modal(false)
        .resizable(false)
        .build();

    if let Some(parent) = gtk::Application::default().active_window() {
        window.set_transient_for(Some(&parent));
    }

    let header = adw::HeaderBar::new();
    header.set_show_end_title_buttons(true);

    let notebook = gtk::Notebook::builder()
        .show_tabs(true)
        .scrollable(true)
        .vexpand(true)
        .build();

    let content = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .build();
    content.append(&header);
    content.append(&notebook);

    window.set_content(Some(&content));

    let s = sender.clone();
    window.connect_close_request(move |_win| {
        let _ = s.send(AppMsg::TransferDialogClosed);
        glib::Propagation::Proceed
    });

    let notebook_weak = notebook.downgrade();
    let queue_clone = queue.clone();
    let sender_clone = sender.clone();

    let ticker_id = glib::timeout_add_local(
        std::time::Duration::from_millis(REFRESH_INTERVAL_MS as u64),
        move || {
            let Some(nb) = notebook_weak.upgrade() else {
                return glib::ControlFlow::Break;
            };
            let snapshot = queue_clone.snapshot();
            if snapshot.is_empty() {
                let _ = sender_clone.send(AppMsg::TransferDialogClosed);
                return glib::ControlFlow::Break;
            }
            rebuild_tabs(&nb, &snapshot, &queue_clone, &sender_clone);
            glib::ControlFlow::Continue
        },
    );

    let snapshot = queue.snapshot();
    rebuild_tabs(&notebook, &snapshot, &queue, &sender);

    window.present();

    TransferDialogHandle {
        window,
        notebook,
        queue,
        ticker_id: Some(ticker_id),
        sender,
    }
}

// ─── Tab builder ─────────────────────────────────────────────────────────────

fn rebuild_tabs(
    notebook: &gtk::Notebook,
    snapshot: &[(u64, crate::services::tasks::Task)],
    queue: &Arc<TaskQueue>,
    sender: &relm4::Sender<AppMsg>,
) {
    while notebook.n_pages() > 0 {
        notebook.remove_page(Some(0));
    }

    for (_id, task) in snapshot {
        let page = build_task_tab(task, queue, sender);
        let label = gtk::Label::new(Some(short_label(&task.label)));
        notebook.append_page(&page, Some(&label));
    }

    if notebook.n_pages() > 0 {
        notebook.set_current_page(Some(0));
    }
}

fn build_task_tab(
    task: &crate::services::tasks::Task,
    _queue: &Arc<TaskQueue>,
    _sender: &relm4::Sender<AppMsg>,
) -> gtk::Widget {
    let container = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(12)
        .margin_top(16)
        .margin_bottom(16)
        .margin_start(20)
        .margin_end(20)
        .build();

    // ─── Operation label ──────────────────────────────────────────────────
    let op_label = gtk::Label::builder()
        .label(&task.label)
        .xalign(0.0)
        .ellipsize(pango::EllipsizeMode::Middle)
        .css_classes(["heading"])
        .build();
    container.append(&op_label);

    // ─── Progress bar ─────────────────────────────────────────────────────
    let fraction = if task.total > 0 {
        (task.current as f64 / task.total as f64).clamp(0.0, 1.0)
    } else {
        -1.0
    };
    let progress = gtk::ProgressBar::builder().show_text(false).build();
    if fraction < 0.0 {
        progress.pulse();
    } else {
        progress.set_fraction(fraction);
    }
    container.append(&progress);

    // ─── Stats row: bytes + speed + ETA ─────────────────────────────────
    let stats_box = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(8)
        .build();

    let bytes_label = {
        let text = if task.total > 0 {
            format!(
                "{} / {} ({:.0}%)",
                format_bytes(task.current),
                format_bytes(task.total),
                fraction * 100.0
            )
        } else {
            format!("{} transferred", format_bytes(task.current))
        };
        gtk::Label::builder()
            .label(&text)
            .xalign(0.0)
            .hexpand(true)
            .build()
    };
    stats_box.append(&bytes_label);

    let bps = task.speed.bytes_per_sec();
    if bps > 1.0 {
        let speed_str = format!("{}/s", format_bytes(bps as u64));
        let speed_label = gtk::Label::builder()
            .label(&speed_str)
            .css_classes(["dim-label"])
            .build();
        stats_box.append(&speed_label);

        if task.total > task.current {
            let remaining = task.total - task.current;
            let eta_secs = (remaining as f64 / bps) as u64;
            if eta_secs < 86_400 {
                let template = crate::i18n::tr("- {} remaining");
                let label_text = template.replace("{}", &format_duration(eta_secs));
                let eta_label = gtk::Label::builder()
                    .label(&label_text)
                    .css_classes(["dim-label"])
                    .build();
                stats_box.append(&eta_label);
            }
        }
    }

    container.append(&stats_box);

    // ─── Additional info: items processed ──────────────────────────────
    if task.total_items > 1 {
        let items_label = gtk::Label::builder()
            .label(format!(
                "{} of {} items",
                task.total_items, task.total_items
            ))
            .xalign(0.0)
            .css_classes(["dim-label"])
            .build();
        container.append(&items_label);
    }

    container.upcast()
}

fn short_label(s: &str) -> &str {
    if s.len() > 30 {
        &s[..30]
    } else {
        s
    }
}
