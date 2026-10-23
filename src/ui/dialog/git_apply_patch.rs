//! "Apply Patch" dialog for the git header menu, to review and edit a unified diff first.

use crate::i18n::tr;
use crate::model::AppMsg;
use adw::prelude::*;
use relm4::AsyncComponentSender;
use std::cell::RefCell;
use std::rc::Rc;

/// Returns the whole buffer text, hidden chars included, exactly as inserted.
fn buffer_text(buf: &gtk::TextBuffer) -> String {
    let (start, end) = buf.bounds();
    buf.text(&start, &end, true).to_string()
}

pub fn show_apply_patch_dialog(
    parent: Option<&gtk::Window>,
    sender: AsyncComponentSender<crate::model::FluxApp>,
) {
    crate::hit!("show_apply_patch_dialog");
    let dialog = adw::Window::builder()
        .title(tr("Apply Patch"))
        .modal(true)
        .default_width(720)
        .default_height(560)
        .resizable(true)
        .build();
    if let Some(p) = parent {
        dialog.set_transient_for(Some(p));
    }

    let outer = gtk::Box::new(gtk::Orientation::Vertical, 0);
    outer.append(&adw::HeaderBar::new());

    let body = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(12)
        .margin_start(16)
        .margin_end(16)
        .margin_top(12)
        .margin_bottom(16)
        .build();

    body.append(
        &gtk::Label::builder()
            .label(tr("Patch (unified diff)"))
            .halign(gtk::Align::Start)
            .css_classes(["heading"])
            .build(),
    );

    let text_view = gtk::TextView::builder()
        .monospace(true)
        .wrap_mode(gtk::WrapMode::None)
        .top_margin(6)
        .bottom_margin(6)
        .left_margin(6)
        .right_margin(6)
        .build();

    let scroller = gtk::ScrolledWindow::builder()
        .child(&text_view)
        .hscrollbar_policy(gtk::PolicyType::Automatic)
        .vscrollbar_policy(gtk::PolicyType::Automatic)
        .min_content_height(320)
        .vexpand(true)
        .hexpand(true)
        .build();
    scroller.add_css_class("card");
    body.append(&scroller);

    let status_label = gtk::Label::builder()
        .label("")
        .halign(gtk::Align::Start)
        .xalign(0.0)
        .css_classes(["caption", "dim-label"])
        .build();
    body.append(&status_label);

    let btn_row = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(8)
        .halign(gtk::Align::End)
        .build();
    let from_clipboard_btn = gtk::Button::with_label(&tr("From Clipboard"));
    let cancel_btn = gtk::Button::with_label(&tr("Cancel"));
    let apply_btn = gtk::Button::with_label(&tr("Apply"));
    apply_btn.add_css_class("suggested-action");
    btn_row.append(&from_clipboard_btn);
    btn_row.append(&cancel_btn);
    btn_row.append(&apply_btn);
    body.append(&btn_row);

    outer.append(&body);
    dialog.set_content(Some(&outer));

    // Clipboard text as received, sent as-is while the buffer is unedited.
    let original: Rc<RefCell<Option<String>>> = Rc::new(RefCell::new(None));

    let load_into_buffer = {
        let buf = text_view.buffer();
        let original = original.clone();
        let status = status_label.clone();
        move |text: String, source: &str| {
            if text.trim().is_empty() {
                status.set_label(&tr("Clipboard is empty"));
                return;
            }
            buf.set_text(&text);
            *original.borrow_mut() = Some(text);
            status.set_label(&match source {
                "open" => tr("Loaded from clipboard"),
                _ => tr("Reloaded from clipboard"),
            });
        }
    };

    {
        let load = load_into_buffer.clone();
        if let Some(clipboard) = gtk::gdk::Display::default().map(|d| d.clipboard()) {
            clipboard.read_text_async(gtk::gio::Cancellable::NONE, move |res| {
                if let Ok(Some(text)) = res {
                    load(text.to_string(), "open");
                }
            });
        }
    }

    {
        let load = load_into_buffer;
        from_clipboard_btn.connect_clicked(move |_| {
            if let Some(clipboard) = gtk::gdk::Display::default().map(|d| d.clipboard()) {
                let load = load.clone();
                clipboard.read_text_async(gtk::gio::Cancellable::NONE, move |res| match res {
                    Ok(Some(text)) => load(text.to_string(), "refresh"),
                    Ok(None) => {}
                    Err(e) => eprintln!("[flux][git] clipboard error: {e}"),
                });
            }
        });
    }

    {
        let d = dialog.clone();
        cancel_btn.connect_clicked(move |_| d.close());
    }

    {
        let d = dialog.clone();
        let s = sender.clone();
        let buf = text_view.buffer();
        let original = original.clone();
        let status = status_label.clone();
        let apply_btn_c = apply_btn.clone();
        apply_btn.connect_clicked(move |_| {
            let now = buffer_text(&buf);
            let bytes = match original.borrow().clone() {
                Some(orig) if now.trim_end_matches('\n') == orig.trim_end_matches('\n') => orig,
                _ => now,
            };

            if bytes.trim().is_empty() {
                apply_btn_c.add_css_class("error");
                status.set_label(&tr("Patch is empty"));
                return;
            }
            let _ = s.input_sender().send(AppMsg::GitApplyPatch(bytes));
            d.close();
        });
    }

    {
        let d = dialog.clone();
        let esc = gtk::ShortcutController::new();
        esc.add_shortcut(gtk::Shortcut::new(
            gtk::ShortcutTrigger::parse_string("Escape"),
            Some(gtk::CallbackAction::new(move |_, _| {
                d.close();
                gtk::glib::Propagation::Stop
            })),
        ));
        dialog.add_controller(esc);
    }

    dialog.present();
}
