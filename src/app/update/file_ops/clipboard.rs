use crate::domain::model::{AppMsg, FileOpsMsg, WindowMsg, FluxApp};
use adw::prelude::*;
use relm4::prelude::*;

impl FluxApp {
    /// Handles clipboard Copy and Cut actions by populating standard GTK Clipboard providers.
    pub fn handle_copy_or_cut(&mut self, is_cut: bool, sender: &AsyncComponentSender<Self>) {
        crate::hit!("handle_copy_or_cut");
        self.handle_clipboard_action(is_cut);

        let selection = self.get_selection();
        for tab in &mut self.tabs {
            for i in (0..tab.files.len()).rev() {
                if let Some(wrapper) = tab.files.get(i) {
                    let mut item = wrapper.borrow().clone();
                    let is_selected = selection.contains(&item.path);
                    let should_be_cut = is_cut && is_selected;
                    let should_be_copy = !is_cut && is_selected;

                    if item.is_cut != should_be_cut || item.is_copy != should_be_copy {
                        item.is_cut = should_be_cut;
                        item.is_copy = should_be_copy;
                        tab.files.remove(i);
                        tab.files.insert(i, item);
                    }
                }
            }
        }

        let cmd = if is_cut {
            "builtin::cut"
        } else {
            "builtin::copy"
        };
        if let Some(toast) = self
            .menu_actions
            .iter()
            .find(|a| a.command == cmd)
            .and_then(|a| a.toast.clone())
        {
            sender.input(AppMsg::Window(WindowMsg::ShowToast(toast)));
        }
    }

    /// Copies the absolute paths of selected items to the clipboard.
    /// Resolves symlinks to their canonical target path when available.
    pub fn handle_copy_path(&self, sender: &AsyncComponentSender<Self>) {
        crate::hit!("handle_copy_path");
        let selection = self.get_selection();
        if selection.is_empty() {
            sender.input(AppMsg::Window(WindowMsg::ShowToast(crate::i18n::tr("No items selected."))));
            return;
        }

        let s = sender.clone();
        relm4::spawn(async move {
            let worker = relm4::spawn_blocking(move || {
                let paths: Vec<String> = selection
                    .iter()
                    .map(|p| {
                        let resolved = p.canonicalize().unwrap_or_else(|_| p.clone());
                        let path = resolved.to_string_lossy();
                        if path.contains(' ') {
                            format!("'{}'", path.replace('\'', "'\\'\\'"))
                        } else {
                            path.into_owned()
                        }
                    })
                    .collect();
                paths.join(" ")
            });

            match worker.await {
                Ok(text) => s.input(AppMsg::FileOps(FileOpsMsg::CopyPathReady(text))),
                Err(_) => s.input(AppMsg::Window(WindowMsg::ShowToast(crate::i18n::tr(
                    "Failed to copy paths to clipboard.",
                )))),
            }
        });
    }

    /// Places the prepared path text on the clipboard once the background
    /// canonicalization in `handle_copy_path` has finished.
    pub fn handle_copy_path_ready(&self, text: String, sender: &AsyncComponentSender<Self>) {
        crate::hit!("handle_copy_path_ready");
        let Some(display) = gtk::gdk::Display::default() else {
            sender.input(AppMsg::Window(WindowMsg::ShowToast(crate::i18n::tr(
                "No display available for clipboard operation",
            ))));
            return;
        };
        display.clipboard().set_text(&text);
        sender.input(AppMsg::Window(WindowMsg::ShowToast(crate::i18n::tr(
            "Paths copied to clipboard.",
        ))));
    }
}
