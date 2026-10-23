//! "Commit changes" dialog for the git header menu.

use crate::i18n::tr;
use crate::model::AppMsg;
use adw::prelude::*;
use relm4::AsyncComponentSender;

pub fn show_commit_dialog(
    parent: Option<&gtk::Window>,
    sender: AsyncComponentSender<crate::model::FluxApp>,
) {
    present_commit_dialog(parent, sender, None);
}

/// Same dialog prefilled with `last_message`, committing with `git commit --amend`.
pub fn show_amend_dialog(
    parent: Option<&gtk::Window>,
    sender: AsyncComponentSender<crate::model::FluxApp>,
    last_message: &str,
) {
    present_commit_dialog(parent, sender, Some(last_message));
}

/// Builds the commit dialog, amending the last commit when `amend_message` is set.
fn present_commit_dialog(
    parent: Option<&gtk::Window>,
    sender: AsyncComponentSender<crate::model::FluxApp>,
    amend_message: Option<&str>,
) {
    crate::hit!("show_commit_dialog");
    let amend = amend_message.is_some();
    let dialog = adw::Window::builder()
        .title(if amend {
            tr("Amend Last Commit")
        } else {
            tr("Commit Changes")
        })
        .modal(true)
        .default_width(480)
        .default_height(360)
        .resizable(false)
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
            .label(tr("Commit message"))
            .halign(gtk::Align::Start)
            .css_classes(["heading"])
            .build(),
    );

    let entry = gtk::TextView::builder()
        .wrap_mode(gtk::WrapMode::WordChar)
        .top_margin(6)
        .bottom_margin(6)
        .left_margin(6)
        .right_margin(6)
        .height_request(96)
        .build();

    let scroller = gtk::ScrolledWindow::builder()
        .child(&entry)
        .hscrollbar_policy(gtk::PolicyType::Never)
        .min_content_height(140)
        .vexpand(true)
        .build();
    scroller.add_css_class("card");
    body.append(&scroller);

    let btn_row = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(8)
        .halign(gtk::Align::End)
        .build();
    let cancel_btn = gtk::Button::with_label(&tr("Cancel"));
    let commit_btn = gtk::Button::with_label(&if amend { tr("Amend") } else { tr("Commit") });
    commit_btn.add_css_class("suggested-action");
    btn_row.append(&cancel_btn);
    btn_row.append(&commit_btn);
    body.append(&btn_row);

    outer.append(&body);
    dialog.set_content(Some(&outer));

    if let Some(message) = amend_message {
        entry.buffer().set_text(message);
    }

    {
        let d = dialog.clone();
        cancel_btn.connect_clicked(move |_| d.close());
    }
    {
        let d = dialog.clone();
        let s = sender.clone();
        let buf = entry.buffer();
        let commit_btn_c = commit_btn.clone();
        commit_btn.connect_clicked(move |_| {
            let (start, end) = buf.bounds();
            let msg = buf.text(&start, &end, false).trim().to_string();
            if msg.is_empty() {
                commit_btn_c.add_css_class("error");
                return;
            }
            let _ = s.input_sender().send(if amend {
                AppMsg::GitAmend(msg)
            } else {
                AppMsg::GitCommit(msg)
            });
            d.close();
        });
    }
    // Esc closes.
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
