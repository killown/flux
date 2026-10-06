use crate::model::FluxApp;
use adw::prelude::*;
use relm4::prelude::*;

impl FluxApp {
    pub fn show_about_window() {
        let about = gtk::AboutDialog::builder()
            .program_name("Flux")
            .version(env!("CARGO_PKG_VERSION"))
            .logo_icon_name("system-file-manager")
            .authors(vec!["killown".to_string()])
            .website("https://github.com/killown/flux")
            .website_label(crate::i18n::tr("Source Code"))
            .comments(crate::i18n::tr(
                "A fast, keyboard-driven file manager built with GTK4 and Libadwaita.",
            ))
            .license_type(gtk::License::Gpl30Only)
            .modal(true)
            .resizable(false)
            .build();

        if let Some(window) = gtk::Application::default().active_window() {
            about.set_transient_for(Some(&window));
        }

        about.present();
    }
}
