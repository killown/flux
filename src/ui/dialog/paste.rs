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
        crate::hit!("show_confirm_replace_paste");
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
        let dialog = adw::AlertDialog::new(Some("Replace Existing Folder?"), Some(&body));
        dialog.add_response("cancel", "Cancel");
        dialog.add_response("replace", "Replace");
        dialog.set_response_appearance("replace", adw::ResponseAppearance::Destructive);
        dialog.set_close_response("cancel");

        let s = sender.clone();
        dialog.connect_response(None, move |_, response| {
            if response == "replace" {
                s.input(AppMsg::PerformPasteForced {
                    files: files.clone(),
                    is_cut,
                });
            }
        });
        dialog.present(window.as_ref());
    }
}
