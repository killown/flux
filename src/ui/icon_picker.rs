// src/ui/icon_picker.rs
use crate::model::AppMsg;
use adw::prelude::*;
use gtk::glib;
use relm4::prelude::*;

pub struct IconPicker {
    target_path: std::path::PathBuf,
    icon_names: Vec<String>,
}

#[derive(Debug)]
pub enum IconPickerMsg {
    IconSelected(String),
    ResetIcon,
    Close,
}

#[relm4::component(pub)]
impl SimpleComponent for IconPicker {
    type Init = std::path::PathBuf;
    type Input = IconPickerMsg;
    type Output = ();

    view! {
        adw::Window {
            set_title: Some("Select Folder Icon"),
            set_default_size: (500, 600),
            set_modal: true,

            gtk::Box {
                set_orientation: gtk::Orientation::Vertical,

                adw::HeaderBar {
                    pack_end = &gtk::Button {
                        set_label: "Close",
                        connect_clicked => IconPickerMsg::Close,
                    }
                },

                gtk::ScrolledWindow {
                    set_vexpand: true,
                    gtk::FlowBox {
                        set_column_spacing: 12,
                        set_row_spacing: 12,
                        set_selection_mode: gtk::SelectionMode::Single,
                        // Populate with icon buttons dynamically
                    }
                },
            }
        }
    }

    fn init(
        target_path: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        // Common GTK icon names for selection
        let icon_names = vec![
            "folder-symbolic",
            "folder-documents-symbolic",
            "folder-download-symbolic",
            "folder-music-symbolic",
            "folder-pictures-symbolic",
            "folder-videos-symbolic",
            "folder-development-symbolic",
            "folder-remote-symbolic",
            "user-home-symbolic",
            "drive-harddisk-symbolic",
        ];

        let model = IconPicker {
            target_path,
            icon_names,
        };

        let widgets = view_output!();
        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: Self::Input, sender: ComponentSender<Self>) {
        match msg {
            IconPickerMsg::IconSelected(icon_name) => {
                // Send message to main app to persist the icon
                if let Some(main_sender) = crate::model::SENDER.get() {
                    let _ = main_sender.send(AppMsg::SetFolderIcon {
                        path: self.target_path.clone(),
                        icon_name,
                    });
                }
                sender.output(()).ok();
            }
            IconPickerMsg::ResetIcon => {
                if let Some(main_sender) = crate::model::SENDER.get() {
                    let _ = main_sender.send(AppMsg::ResetFolderIcon(self.target_path.clone()));
                }
                sender.output(()).ok();
            }
            IconPickerMsg::Close => {
                sender.output(()).ok();
            }
        }
    }
}
