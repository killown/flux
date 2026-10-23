//! Read-only text dialog for git output that is too long for a toast: logs and error messages.

use crate::i18n::tr;
use adw::prelude::*;

pub fn show_git_output_dialog(parent: Option<&gtk::Window>, title: &str, text: &str) {
    crate::hit!("show_git_output_dialog");
    let dialog = adw::Dialog::builder()
        .title(title)
        .content_width(820)
        .content_height(560)
        .build();

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

    let text_view = gtk::TextView::builder()
        .monospace(true)
        .editable(false)
        .cursor_visible(false)
        .wrap_mode(gtk::WrapMode::WordChar)
        .top_margin(6)
        .bottom_margin(6)
        .left_margin(6)
        .right_margin(6)
        .build();
    text_view.buffer().set_text(text);

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

    let btn_row = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(8)
        .halign(gtk::Align::End)
        .build();
    let copy_btn = gtk::Button::with_label(&tr("Copy"));
    let close_btn = gtk::Button::with_label(&tr("Close"));
    close_btn.add_css_class("suggested-action");
    btn_row.append(&copy_btn);
    btn_row.append(&close_btn);
    body.append(&btn_row);

    outer.append(&body);
    dialog.set_child(Some(&outer));

    {
        let text = text.to_string();
        copy_btn.connect_clicked(move |_| {
            if let Some(display) = gtk::gdk::Display::default() {
                display.clipboard().set_text(&text);
            }
        });
    }
    {
        let d = dialog.clone();
        close_btn.connect_clicked(move |_| {
            d.close();
        });
    }

    dialog.present(parent);
}
