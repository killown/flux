//! Plain-text editor for a `menu-*.rs` template file.
//!
//! Opened from the contextual-actions panel's Edit button, which always
//! knows the exact template `resolve_secondary_menu_template` returned
//! for the current selection's MIME type.

use crate::i18n::tr;
use adw::prelude::*;
use relm4::AsyncComponentSender;
use std::path::PathBuf;

/// Opens a modal editor for `template_path`.
///
/// The dialog takes the parent window's size so editing feels like an
/// in-place edit of the panel's backing file rather than a separate window.
pub fn show_template_editor(
    parent: Option<&gtk::Window>,
    template_path: PathBuf,
    sender: AsyncComponentSender<crate::model::FluxApp>,
) {
    crate::hit!("show_template_editor");
    let dialog = adw::Window::builder()
        .title(tr("Edit Actions Template").as_str())
        .modal(true)
        .build();

    if let Some(win) = parent {
        dialog.set_transient_for(Some(win));
        dialog.set_default_size(win.default_width(), win.default_height());
    }

    let outer = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .build();

    let header = adw::HeaderBar::builder()
        .show_start_title_buttons(false)
        .show_end_title_buttons(false)
        .build();

    let title_widget = adw::WindowTitle::builder()
        .title(tr("Edit Actions Template").as_str())
        .subtitle(
            template_path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| template_path.to_string_lossy().into_owned())
                .as_str(),
        )
        .build();
    header.set_title_widget(Some(&title_widget));

    let cancel_btn = gtk::Button::builder().label(tr("Cancel").as_str()).build();
    let save_btn = gtk::Button::builder()
        .label(tr("Save").as_str())
        .css_classes(["suggested-action"])
        .build();
    header.pack_start(&cancel_btn);
    header.pack_end(&save_btn);
    outer.append(&header);

    let initial_text = std::fs::read_to_string(&template_path)
        .unwrap_or_else(|e| format!("// could not read {}: {e}\n", template_path.display()));

    let text_view = gtk::TextView::builder()
        .editable(true)
        .monospace(true)
        .wrap_mode(gtk::WrapMode::None)
        .top_margin(8)
        .bottom_margin(8)
        .left_margin(8)
        .right_margin(8)
        .build();
    text_view.buffer().set_text(&initial_text);

    let scroller = gtk::ScrolledWindow::builder()
        .child(&text_view)
        .vexpand(true)
        .hexpand(true)
        .build();
    outer.append(&scroller);

    let status = gtk::Label::builder()
        .xalign(0.0)
        .margin_start(12)
        .margin_end(12)
        .margin_bottom(8)
        .css_classes(["caption", "dim-label"])
        .label(template_path.to_string_lossy().as_ref())
        .build();
    outer.append(&status);

    dialog.set_content(Some(&outer));

    {
        let d = dialog.clone();
        cancel_btn.connect_clicked(move |_| d.close());
    }

    {
        let d = dialog.clone();
        let esc = gtk::EventControllerKey::new();
        esc.set_propagation_phase(gtk::PropagationPhase::Capture);
        esc.connect_key_pressed(move |_, key, _, _| {
            if key == gtk::gdk::Key::Escape {
                d.close();
                return gtk::glib::Propagation::Stop;
            }
            gtk::glib::Propagation::Proceed
        });
        dialog.add_controller(esc);
    }

    {
        let d = dialog.clone();
        let s = sender.clone();
        let path = template_path.clone();
        let view = text_view.clone();
        let status = status.clone();
        save_btn.connect_clicked(move |_| {
            let buf = view.buffer();
            let (start, end) = buf.bounds();
            let text = buf.text(&start, &end, false).to_string();
            match std::fs::write(&path, text.as_bytes()) {
                Ok(()) => {
                    s.input(crate::model::AppMsg::ReloadActionsTemplate { path: path.clone() });
                    d.close();
                }
                Err(e) => {
                    status.set_label(&format!("{}: {e}", tr("Could not save")));
                    status.remove_css_class("dim-label");
                    status.add_css_class("error");
                }
            }
        });
    }

    // Match the parent window's current size, not its default size, so the
    // editor lands at the same dimensions the user is looking at right now.
    if let Some(win) = parent {
        let (w, h) = (win.width(), win.height());
        if w > 0 && h > 0 {
            dialog.set_default_size(w, h);
        }
    }

    dialog.present();
}
