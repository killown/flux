//! "Switch Branch" dialog for the git header menu: pick a local branch or create a new one.

use crate::i18n::tr;
use crate::model::AppMsg;
use adw::prelude::*;
use relm4::AsyncComponentSender;
use std::rc::Rc;

pub fn show_switch_branch_dialog(
    parent: Option<&gtk::Window>,
    sender: AsyncComponentSender<crate::model::FluxApp>,
    current: Option<String>,
    branches: Vec<String>,
) {
    crate::hit!("show_switch_branch_dialog");
    let dialog = adw::Dialog::builder()
        .title(tr("Switch Branch"))
        .content_width(420)
        .content_height(480)
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

    body.append(
        &gtk::Label::builder()
            .label(tr("Branch"))
            .halign(gtk::Align::Start)
            .css_classes(["heading"])
            .build(),
    );

    let entry = gtk::Entry::builder()
        .placeholder_text(tr("Filter branches or type a new name"))
        .activates_default(false)
        .build();
    body.append(&entry);

    let list = gtk::ListBox::builder()
        .selection_mode(gtk::SelectionMode::Single)
        .activate_on_single_click(false)
        .build();
    list.add_css_class("boxed-list");

    let branches = Rc::new(branches);
    for name in branches.iter() {
        let is_current = current.as_deref() == Some(name.as_str());
        let label = gtk::Label::builder()
            .label(if is_current {
                format!("{name}  ({})", tr("current"))
            } else {
                name.clone()
            })
            .halign(gtk::Align::Start)
            .margin_start(10)
            .margin_end(10)
            .margin_top(8)
            .margin_bottom(8)
            .build();
        let row = gtk::ListBoxRow::new();
        row.set_child(Some(&label));
        row.set_activatable(!is_current);
        list.append(&row);
    }

    {
        let entry = entry.clone();
        let branches = branches.clone();
        list.set_filter_func(move |row| {
            let needle = entry.text().to_lowercase();
            if needle.is_empty() {
                return true;
            }
            usize::try_from(row.index())
                .ok()
                .and_then(|i| branches.get(i))
                .map(|name| name.to_lowercase().contains(&needle))
                .unwrap_or(true)
        });
    }

    let scroller = gtk::ScrolledWindow::builder()
        .child(&list)
        .hscrollbar_policy(gtk::PolicyType::Never)
        .min_content_height(240)
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
    let create_btn = gtk::Button::with_label(&tr("Create & Switch"));
    let switch_btn = gtk::Button::with_label(&tr("Switch"));
    switch_btn.add_css_class("suggested-action");
    switch_btn.set_sensitive(false);
    create_btn.set_sensitive(false);
    btn_row.append(&cancel_btn);
    btn_row.append(&create_btn);
    btn_row.append(&switch_btn);
    body.append(&btn_row);

    outer.append(&body);
    dialog.set_child(Some(&outer));

    // Typing refilters the list and enables "Create" only for a name that does not exist yet.
    {
        let list = list.clone();
        let create_btn = create_btn.clone();
        let branches = branches.clone();
        entry.connect_changed(move |e| {
            list.invalidate_filter();
            let typed = e.text();
            let typed = typed.trim();
            create_btn.set_sensitive(!typed.is_empty() && !branches.iter().any(|b| b == typed));
        });
    }

    {
        let switch_btn = switch_btn.clone();
        list.connect_row_selected(move |_, row| {
            switch_btn.set_sensitive(row.map(|r| r.is_activatable()).unwrap_or(false));
        });
    }

    let switch_to = {
        let d = dialog.clone();
        let s = sender.clone();
        let branches = branches.clone();
        Rc::new(move |row: &gtk::ListBoxRow| {
            let Some(name) = usize::try_from(row.index())
                .ok()
                .and_then(|i| branches.get(i))
            else {
                return;
            };
            let _ = s.input_sender().send(AppMsg::GitSwitchBranch(name.clone()));
            d.close();
        })
    };

    {
        let switch_to = switch_to.clone();
        list.connect_row_activated(move |_, row| switch_to(row));
    }
    {
        let list = list.clone();
        let switch_to = switch_to.clone();
        switch_btn.connect_clicked(move |_| {
            if let Some(row) = list.selected_row() {
                switch_to(&row);
            }
        });
    }
    {
        let d = dialog.clone();
        let s = sender.clone();
        let entry = entry.clone();
        create_btn.connect_clicked(move |_| {
            let name = entry.text().trim().to_string();
            if name.is_empty() {
                return;
            }
            let _ = s.input_sender().send(AppMsg::GitCreateBranch(name));
            d.close();
        });
    }
    {
        let d = dialog.clone();
        cancel_btn.connect_clicked(move |_| {
            d.close();
        });
    }

    dialog.present(parent);
    entry.grab_focus();
}
