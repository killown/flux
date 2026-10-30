//! Leaf widgets of the main header bar.

use crate::domain::model::{
    AppMsg, ArchiveMsg, FileOpsMsg, FluxApp, NavigationMsg, PreviewMsg, SearchMsg, ViewMsg,
};
use crate::i18n::tr;
use crate::ui::constants;
use adw::prelude::*;
use relm4::prelude::*;

fn pointer_cursor(widget: &impl IsA<gtk::Widget>) {
    widget.connect_realize(|w| FluxApp::set_cursor_pointer(w.upcast_ref(), true));
}

pub(in crate::ui::view) fn icon_row(icon_name: &str) -> gtk::Box {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, constants::HEADER_BTN_SPACING);
    row.append(&gtk::Image::from_icon_name(icon_name));
    row
}

fn icon_label_row(icon_name: &str, label: &str) -> gtk::Box {
    let row = icon_row(icon_name);
    row.append(&gtk::Label::new(Some(label)));
    row
}

pub(in crate::ui::view) fn build_sort_indicator(
    sender: &AsyncComponentSender<FluxApp>,
) -> gtk::Box {
    let container = gtk::Box::new(gtk::Orientation::Horizontal, constants::STATUS_ICON_SPACING);
    container.set_margin_end(constants::HEADER_MARGIN_END);
    container.set_valign(gtk::Align::Center);
    container.add_css_class(constants::SORT_CONTAINER_CLASS);

    let click = gtk::GestureClick::new();
    click.set_propagation_phase(gtk::PropagationPhase::Capture);
    let s = sender.clone();
    click.connect_pressed(move |gesture, _, _, _| {
        gesture.set_state(gtk::EventSequenceState::Claimed);
        s.input(AppMsg::View(ViewMsg::CycleSort));
    });
    container.add_controller(click);

    let icon = gtk::Image::from_icon_name(constants::ICON_SORT_INDICATOR);
    icon.set_pixel_size(constants::STATUS_ICON_SIZE);
    icon.add_css_class("dimmed");
    container.append(&icon);
    container
}

pub(in crate::ui::view) fn build_main_menu_button() -> gtk::MenuButton {
    let button = gtk::MenuButton::new();
    button.set_icon_name("open-menu-symbolic");
    button.set_tooltip_text(Some(&tr("Main Menu")));
    button.add_css_class("flat");
    pointer_cursor(&button);
    button
}

pub(in crate::ui::view) fn build_git_menu_button() -> gtk::MenuButton {
    let button = gtk::MenuButton::new();
    button.set_icon_name("git-symbolic");
    button.set_tooltip_text(Some(&tr("Git Actions")));
    button.add_css_class("flat");
    pointer_cursor(&button);
    button
}

pub(in crate::ui::view) fn build_empty_trash_button(
    sender: &AsyncComponentSender<FluxApp>,
) -> gtk::Button {
    let button = gtk::Button::new();
    button.set_tooltip_text(Some(&tr("Empty Trash")));
    button.add_css_class(constants::DESTRUCTIVE_ACTION_CLASS);
    pointer_cursor(&button);
    let s = sender.clone();
    button.connect_clicked(move |_| s.input(AppMsg::FileOps(FileOpsMsg::EmptyTrash)));
    button.set_child(Some(&icon_label_row(
        constants::ICON_TRASH,
        &tr("Empty Trash"),
    )));
    button
}

pub(in crate::ui::view) fn build_extract_archive_button(
    sender: &AsyncComponentSender<FluxApp>,
) -> gtk::Button {
    let button = gtk::Button::new();
    button.set_tooltip_text(Some(&tr("Extract Archive")));
    button.add_css_class("suggested-action");
    pointer_cursor(&button);
    let s = sender.clone();
    button.connect_clicked(move |_| s.input(AppMsg::Archive(ArchiveMsg::ExtractArchive)));
    button.set_child(Some(&icon_label_row(
        "archive-extract-symbolic",
        &tr("Extract"),
    )));
    button
}

pub(in crate::ui::view) fn build_recents_button(
    sender: &AsyncComponentSender<FluxApp>,
) -> gtk::Button {
    let button = gtk::Button::new();
    button.add_css_class(constants::DESTRUCTIVE_ACTION_CLASS);
    pointer_cursor(&button);
    let s = sender.clone();
    button.connect_clicked(move |_| s.input(AppMsg::Navigation(NavigationMsg::ClearRecents)));
    button
}

pub(in crate::ui::view) fn build_list_toggle(
    sender: &AsyncComponentSender<FluxApp>,
) -> gtk::ToggleButton {
    let button = gtk::ToggleButton::new();
    button.set_icon_name("view-list-symbolic");
    button.set_tooltip_text(Some(&tr("Toggle List Mode")));
    button.add_css_class("flat");
    pointer_cursor(&button);
    let s = sender.clone();
    button.connect_clicked(move |_| s.input(AppMsg::View(ViewMsg::ToggleListMode)));
    button
}

pub(in crate::ui::view) fn build_filter_toggle(
    sender: &AsyncComponentSender<FluxApp>,
) -> gtk::ToggleButton {
    let button = gtk::ToggleButton::new();
    button.set_tooltip_text(Some(&tr("Filter by Pattern")));
    button.add_css_class("flat");
    pointer_cursor(&button);
    button.set_child(Some(&gtk::Image::from_gicon(
        &gtk::gio::ThemedIcon::from_names(&["view-filter-symbolic", "filter-symbolic"]),
    )));
    let s = sender.clone();
    button.connect_clicked(move |_| s.input(AppMsg::Search(SearchMsg::ToggleFilterBar)));
    button
}

pub(in crate::ui::view) fn build_preview_toggle(
    sender: &AsyncComponentSender<FluxApp>,
) -> gtk::ToggleButton {
    let button = gtk::ToggleButton::new();
    button.set_icon_name("view-reveal-symbolic");
    button.set_tooltip_text(Some(&tr("Toggle Preview Panel")));
    button.add_css_class("flat");
    pointer_cursor(&button);
    let s = sender.clone();
    button.connect_clicked(move |_| s.input(AppMsg::Preview(PreviewMsg::TogglePreview)));
    button
}
