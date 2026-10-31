//! "Save search as tag" prompt: names a tag and labels every file in the current results.

use crate::app::state::FluxApp;
use crate::domain::model::{AppMsg, TagsMsg};
use adw::prelude::*;
use relm4::prelude::*;
use std::path::PathBuf;

/// Lowercases a typed tag name and drops its leading `#`, as the tag picker does.
/// Returns `None` when nothing is left.
fn normalize_tag_name(raw: &str) -> Option<String> {
    let clean = raw.trim().trim_start_matches('#').trim().to_lowercase();
    if clean.is_empty() {
        None
    } else {
        Some(clean)
    }
}

/// Dialog body: how many files get the tag, then the warning that later matches do not.
fn body_text(count: usize, tag: &str) -> String {
    let head = if tag.is_empty() {
        crate::i18n::tr("{count} files will be tagged.").replace("{count}", &count.to_string())
    } else {
        crate::i18n::tr("{count} files will be tagged #{tag}.")
            .replace("{count}", &count.to_string())
            .replace("{tag}", tag)
    };
    format!(
        "{}\n{}",
        head,
        crate::i18n::tr("New files matching this search will not be tagged automatically.")
    )
}

impl FluxApp {
    /// Displays a modal prompt that saves the current search results as a tag.
    ///
    /// `paths` are the results to label. Nothing is tagged until the user confirms, and
    /// files that match the same search later are not tagged automatically.
    pub fn show_save_search_as_tag(
        &self,
        paths: Vec<PathBuf>,
        sender: &AsyncComponentSender<Self>,
    ) {
        let parent = gtk::Application::default().active_window();
        let s = sender.clone();
        let count = paths.len();

        let dialog = adw::AlertDialog::new(
            Some(&crate::i18n::tr("Save Search as Tag")),
            Some(&body_text(count, "")),
        );

        dialog.add_response("cancel", &crate::i18n::tr("Cancel"));
        dialog.add_response("save", &crate::i18n::tr("Save"));
        dialog.set_response_appearance("save", adw::ResponseAppearance::Suggested);
        dialog.set_response_enabled("save", false);
        dialog.set_default_response(Some("save"));
        dialog.set_close_response("cancel");

        let entry = gtk::Entry::builder()
            .placeholder_text(crate::i18n::tr("Tag name"))
            .activates_default(true)
            .build();
        entry.connect_map(|e| {
            e.grab_focus();
        });

        let pin_check = gtk::CheckButton::builder()
            .label(crate::i18n::tr("Pin to sidebar"))
            .active(true)
            .build();

        let content = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(12)
            .build();
        content.append(&entry);
        content.append(&pin_check);
        dialog.set_extra_child(Some(&content));

        {
            // Weak, so the entry's handler does not keep the dialog that owns it alive.
            let dialog_weak = dialog.downgrade();
            entry.connect_changed(move |e| {
                let Some(dialog) = dialog_weak.upgrade() else {
                    return;
                };
                let name = normalize_tag_name(&e.text());
                dialog.set_response_enabled("save", name.is_some());
                dialog.set_body(&body_text(count, name.as_deref().unwrap_or("")));
            });
        }

        dialog.present(parent.as_ref());

        let entry_clone = entry.clone();
        dialog.connect_response(None, move |_, resp| {
            if resp != "save" {
                return;
            }
            let Some(tag) = normalize_tag_name(&entry_clone.text()) else {
                return;
            };
            s.input(AppMsg::Tags(TagsMsg::SaveSearchAsTag {
                tag,
                paths: paths.clone(),
                pin_to_sidebar: pin_check.is_active(),
            }));
        });
    }
}
