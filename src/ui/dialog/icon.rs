use crate::app::state::FluxApp;
use crate::domain::model::{AppMsg, IconsMsg, NavigationMsg, WindowMsg};
use adw::gdk;
use adw::prelude::*;
use relm4::prelude::*;
use std::path::PathBuf;
use std::rc::Rc;

impl FluxApp {
    /// Displays a grid-based icon picker dialog to set custom folder icons.
    pub fn show_icon_picker(&self, target_path: PathBuf, sender: &AsyncComponentSender<Self>) {
        crate::hit!("show_icon_picker");
        let toplevels = gtk::Window::list_toplevels();
        let parent = toplevels
            .first()
            .and_then(|w| w.downcast_ref::<gtk::Window>());
        let dialog = adw::Dialog::builder().title("Select Folder Icon").build();
        let flow_box = gtk::FlowBox::builder()
            .valign(gtk::Align::Start)
            .max_children_per_line(6)
            .min_children_per_line(6)
            .selection_mode(gtk::SelectionMode::Single)
            .build();
        let scrolled = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .vscrollbar_policy(gtk::PolicyType::Automatic)
            .child(&flow_box)
            .height_request(350)
            .width_request(400)
            .build();
        let search_entry = gtk::SearchEntry::builder()
            .margin_top(6)
            .margin_bottom(6)
            .margin_start(6)
            .margin_end(6)
            .build();
        let content_area = gtk::Box::new(gtk::Orientation::Vertical, 0);
        content_area.append(&search_entry);
        content_area.append(&scrolled);

        let header = adw::HeaderBar::new();
        let cancel_btn = gtk::Button::with_label("Cancel");
        let select_btn = gtk::Button::with_label("Select");
        select_btn.add_css_class("suggested-action");
        header.pack_start(&cancel_btn);
        header.pack_end(&select_btn);

        let toolbar_view = adw::ToolbarView::new();
        toolbar_view.add_top_bar(&header);
        toolbar_view.set_content(Some(&content_area));
        dialog.set_child(Some(&toolbar_view));

        let confirm: Rc<dyn Fn()> = Rc::new({
            let flow_box_select = flow_box.clone();
            let sender_clone = sender.clone();
            let target_path_clone = target_path.clone();
            let dialog_weak = dialog.downgrade();
            move || {
                if let Some(row) = flow_box_select.selected_children().first() {
                    if let Some(button) = row.child().and_then(|c| c.downcast::<gtk::Button>().ok())
                    {
                        unsafe {
                            if let Some(name) =
                                button.data::<String>("icon-name").map(|p| p.as_ref())
                            {
                                sender_clone.input(AppMsg::Icons(IconsMsg::SetFolderIcon {
                                    path: target_path_clone.clone(),
                                    icon_name: name.to_string(),
                                }));
                            }
                        }
                    }
                }
                if let Some(dlg) = dialog_weak.upgrade() {
                    dlg.close();
                }
            }
        });

        let icon_theme = gtk::IconTheme::for_display(&gdk::Display::default().unwrap());
        let icon_names = icon_theme.icon_names();
        for icon_name in icon_names {
            let name_str = icon_name.as_str();
            if name_str.contains("folder") || name_str.contains("Folder") {
                let image = gtk::Image::from_icon_name(name_str);
                image.set_icon_size(gtk::IconSize::Large);
                let button = gtk::Button::builder()
                    .child(&image)
                    .tooltip_text(name_str)
                    .has_frame(false)
                    .build();
                unsafe {
                    button.set_data("icon-name", icon_name.to_string());
                }
                let confirm_btn_clone = confirm.clone();
                let flow_box_btn_clone = flow_box.clone();
                button.connect_clicked(move |btn| {
                    if let Some(row) = btn
                        .parent()
                        .and_then(|p| p.downcast::<gtk::FlowBoxChild>().ok())
                    {
                        flow_box_btn_clone.select_child(&row);
                        confirm_btn_clone();
                    }
                });
                flow_box.append(&button);
            }
        }
        let flow_box_clone = flow_box.clone();
        search_entry.connect_search_changed(move |entry| {
            let text = entry.text().to_string().to_lowercase();
            let mut child = flow_box_clone.first_child();
            while let Some(ref widget) = child {
                if let Some(child_row) = widget.downcast_ref::<gtk::FlowBoxChild>() {
                    if let Some(button) = child_row
                        .child()
                        .and_then(|c| c.downcast::<gtk::Button>().ok())
                    {
                        unsafe {
                            if let Some(name) =
                                button.data::<String>("icon-name").map(|p| p.as_ref())
                            {
                                child_row.set_visible(name.to_lowercase().contains(&text));
                            }
                        }
                    }
                }
                child = widget.next_sibling();
            }
        });
        let confirm_activate = confirm.clone();
        flow_box.connect_child_activated(move |_, _| confirm_activate());

        let confirm_select = confirm.clone();
        select_btn.connect_clicked(move |_| confirm_select());

        let dialog_cancel = dialog.downgrade();
        cancel_btn.connect_clicked(move |_| {
            if let Some(dlg) = dialog_cancel.upgrade() {
                dlg.close();
            }
        });

        dialog.present(parent);
    }

    pub fn show_custom_icon_file_chooser(
        target_path: PathBuf,
        toast: Option<String>,
        sender: AsyncComponentSender<Self>,
    ) {
        crate::hit!("show_custom_icon_file_chooser");
        let filter = gtk::FileFilter::new();
        filter.set_name(Some("Images"));
        filter.add_mime_type("image/png");
        filter.add_mime_type("image/jpeg");
        filter.add_mime_type("image/webp");
        filter.add_mime_type("image/svg+xml");

        let filters = gtk::gio::ListStore::new::<gtk::FileFilter>();
        filters.append(&filter);

        let toplevels = gtk::Window::list_toplevels();
        let parent = toplevels
            .first()
            .and_then(|w| w.downcast_ref::<gtk::Window>())
            .cloned();

        let chooser = gtk::FileDialog::builder()
            .title(crate::i18n::tr("Select Custom Icon Image"))
            .accept_label(crate::i18n::tr("Set Icon"))
            .filters(&filters)
            .build();

        chooser.open(
            parent.as_ref(),
            gtk::gio::Cancellable::NONE,
            move |result| {
                if let Ok(file) = result {
                    if let Some(image_path) = file.path() {
                        sender.input(AppMsg::Icons(IconsMsg::SetFileIcon {
                            path: target_path.clone(),
                            image_path,
                        }));
                        if let Some(ref msg) = toast {
                            sender.input(AppMsg::Window(WindowMsg::ShowToast(msg.clone())));
                        }
                    }
                }
            },
        );
    }

    pub fn show_extension_icon_file_chooser(
        ext: String,
        toast: Option<String>,
        sender: relm4::AsyncComponentSender<Self>,
    ) {
        crate::hit!("show_extension_icon_file_chooser");
        let filter = gtk::FileFilter::new();
        filter.set_name(Some("Image Files"));
        filter.add_mime_type("image/png");
        filter.add_mime_type("image/svg+xml");
        filter.add_mime_type("image/webp");
        filter.add_mime_type("image/jpeg");

        let filters = gtk::gio::ListStore::new::<gtk::FileFilter>();
        filters.append(&filter);

        let chooser = gtk::FileDialog::builder()
            .title("Select Extension Icon")
            .accept_label("Open")
            .filters(&filters)
            .build();

        chooser.open(
            gtk::Window::NONE,
            gtk::gio::Cancellable::NONE,
            move |result| {
                let Ok(file) = result else {
                    return;
                };
                let Some(src_path) = file.path() else {
                    return;
                };
                let Some(mut target_dir) = dirs::data_local_dir() else {
                    return;
                };
                target_dir.push("flux/icons/extensions/custom");
                let _ = std::fs::create_dir_all(&target_dir);

                let file_ext = src_path
                    .extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("png");

                let clean_ext = ext.trim_start_matches('.').to_ascii_lowercase();
                let dest_path = target_dir.join(format!("{}.{}", clean_ext, file_ext));

                for fmt in &["png", "svg", "webp", "jpg", "jpeg"] {
                    let old = target_dir.join(format!("{}.{}", clean_ext, fmt));
                    let _ = std::fs::remove_file(old);
                }

                if std::fs::copy(&src_path, &dest_path).is_ok() {
                    crate::infra::loader::invalidate_extension_icon_cache();
                    crate::utils::icon::invalidate_themed_icon_cache();
                    sender.input(AppMsg::Navigation(NavigationMsg::Refresh));

                    if let Some(ref msg) = toast {
                        sender.input(AppMsg::Window(WindowMsg::ShowToast(msg.clone())));
                    }
                }
            },
        );
    }
}
