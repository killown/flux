use crate::model::FluxApp;
use adw::prelude::*;
use relm4::prelude::*;

impl FluxApp {
    pub fn handle_show_toast(&mut self, msg: String) {
        if let Some(prev) = self.last_toast.take() {
            prev.dismiss();
        }

        let label = gtk::Label::builder()
            .label(&msg)
            .wrap(true)
            .wrap_mode(gtk::pango::WrapMode::WordChar)
            // Some languages require more space than others, so we add a character to the width to
            // avoid truncation.
            .width_chars(msg.chars().count() as i32 + 1)
            .xalign(0.5)
            .justify(gtk::Justification::Center)
            .build();
        label.add_css_class("body");
        label.add_css_class("flux-toast-label");

        let toast = adw::Toast::new("");
        toast.set_custom_title(Some(&label));
        self.toast_overlay.add_toast(toast.clone());
        self.last_toast = Some(toast);
    }
}
