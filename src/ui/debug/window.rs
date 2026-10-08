use super::ffi::collect_now;
use super::format::{fmt_bytes, fmt_kb};
use super::snapshot::DebugSnapshot;
use crate::model::FluxApp;
use adw::prelude::*;
use std::fs;

pub fn show_debug_window(app: &FluxApp) {
    let snapshot = DebugSnapshot::capture(app);
    let output = snapshot.render();

    let window = adw::Window::builder()
        .title("Flux Memory & Allocator Profiler")
        .default_width(1080)
        .default_height(820)
        .build();

    let toast_overlay = adw::ToastOverlay::new();

    let header_bar = adw::HeaderBar::new();

    let refresh_button = gtk::Button::builder()
        .icon_name("view-refresh-symbolic")
        .tooltip_text("Refresh (re-samples memory maps & allocators)")
        .build();

    let trim_button = gtk::Button::builder()
        .icon_name("edit-clear-all-symbolic")
        .tooltip_text("Purge Memory (forces mimalloc & glibc to return free pages to OS)")
        .build();

    let export_button = gtk::Button::builder()
        .icon_name("document-save-symbolic")
        .tooltip_text("Save memory profile report to disk…")
        .build();

    header_bar.pack_start(&refresh_button);
    header_bar.pack_start(&trim_button);
    header_bar.pack_end(&export_button);

    let view = gtk::TextView::builder()
        .editable(false)
        .monospace(true)
        .left_margin(16)
        .right_margin(16)
        .top_margin(12)
        .bottom_margin(12)
        .build();

    let doc_buffer = view.buffer();
    doc_buffer.set_text(&output);

    let scroll_container = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Automatic)
        .vscrollbar_policy(gtk::PolicyType::Automatic)
        .hexpand(true)
        .vexpand(true)
        .build();
    scroll_container.set_child(Some(&view));

    let status_bar = gtk::Label::builder()
        .label(format!(
            "RSS {}  │  Heap {}  │  glibc In-Use {} (Free: {})  │  {} threads  │  {} FDs",
            fmt_kb(snapshot.rss_kb),
            fmt_kb(snapshot.vm_data_kb),
            fmt_bytes(snapshot.mallinfo.uordblks_bytes as u64),
            fmt_bytes(snapshot.mallinfo.fordblks_bytes as u64),
            snapshot.thread_count,
            snapshot.fd_count,
        ))
        .halign(gtk::Align::Start)
        .margin_start(12)
        .margin_end(12)
        .margin_top(6)
        .margin_bottom(6)
        .build();
    status_bar.add_css_class("caption");
    status_bar.add_css_class("dim-label");

    let layout_root = gtk::Box::new(gtk::Orientation::Vertical, 0);
    layout_root.append(&header_bar);
    layout_root.append(&scroll_container);
    layout_root.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
    layout_root.append(&status_bar);

    toast_overlay.set_child(Some(&layout_root));
    window.set_content(Some(&toast_overlay));

    // Export button with FileChooserNative and visual Toast feedback
    {
        let report_payload = output.clone();
        let toast_overlay_clone = toast_overlay.clone();
        let parent_win = window.clone();

        export_button.connect_clicked(move |_| {
            let chooser = gtk::FileChooserNative::builder()
                .title("Save Memory Profile Report")
                .action(gtk::FileChooserAction::Save)
                .accept_label("Save")
                .cancel_label("Cancel")
                .modal(true)
                .transient_for(&parent_win)
                .build();

            chooser.set_current_name("flux_memory_profile.txt");

            let payload = report_payload.clone();
            let overlay = toast_overlay_clone.clone();

            chooser.connect_response(move |dialog, response| {
                if response == gtk::ResponseType::Accept {
                    if let Some(file) = dialog.file() {
                        if let Some(path) = file.path() {
                            match fs::write(&path, payload.as_bytes()) {
                                Ok(()) => {
                                    let msg = format!("Report saved to {}", path.display());
                                    overlay.add_toast(adw::Toast::new(&msg));
                                }
                                Err(err) => {
                                    let msg = format!("Failed to save file: {}", err);
                                    overlay.add_toast(adw::Toast::new(&msg));
                                }
                            }
                        }
                    }
                }
                dialog.destroy();
            });

            chooser.show();
        });
    }

    // Force purge memory (mimalloc + glibc) then trigger reload
    {
        let overlay_trim = toast_overlay.clone();
        trim_button.connect_clicked(move |_| {
            collect_now();
            unsafe {
                libc::malloc_trim(0);
            }
            overlay_trim.add_toast(adw::Toast::new("Memory purged and released to OS"));
            if let Some(chan) = crate::model::SENDER.get() {
                let _ = chan.send(crate::model::AppMsg::OpenDebugWindow);
            }
        });
    }

    // Refresh action
    refresh_button.connect_clicked(move |_| {
        if let Some(chan) = crate::model::SENDER.get() {
            let _ = chan.send(crate::model::AppMsg::OpenDebugWindow);
        }
    });

    window.present();
}
