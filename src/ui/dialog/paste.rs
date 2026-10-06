use crate::model::{AppMsg, FluxApp};
use adw::prelude::*;
use relm4::prelude::*;

impl FluxApp {
    /// Presents a warning dialog when pasting into a location where target folders already exist.
    pub fn show_confirm_replace_paste(
        &self,
        files: Vec<gio::File>,
        conflicts: Vec<String>,
        is_cut: bool,
        sender: &AsyncComponentSender<Self>,
    ) {
        let window = gtk::Application::default().active_window();
        let body = if conflicts.len() == 1 {
            format!(
                "\"{}\" already exists in this location. Replace it and merge its contents?",
                conflicts[0]
            )
        } else {
            format!(
                "{} folders already exist in this location. Replace them and merge their contents?",
                conflicts.len()
            )
        };
        let dialog = gtk::MessageDialog::new(
            window.as_ref(),
            gtk::DialogFlags::MODAL | gtk::DialogFlags::DESTROY_WITH_PARENT,
            gtk::MessageType::Warning,
            gtk::ButtonsType::None,
            "Replace Existing Folder?",
        );
        dialog.set_secondary_text(Some(&body));
        dialog.add_button("Cancel", gtk::ResponseType::Cancel);

        let replace_btn = dialog.add_button("Replace", gtk::ResponseType::Accept);
        replace_btn.style_context().add_class("destructive-action");

        let s = sender.clone();
        dialog.connect_response(move |dlg, response| {
            dlg.close();
            if response == gtk::ResponseType::Accept {
                s.input(AppMsg::PerformPasteForced {
                    files: files.clone(),
                    is_cut,
                });
            }
        });
        dialog.present();
    }
}
