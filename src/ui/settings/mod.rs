use crate::i18n::tr;
use crate::model::Config;
use adw::prelude::*;
use relm4::prelude::*;

mod page;

pub struct SettingsWindow {
    config: Config,
}

#[relm4::component(pub)]
impl SimpleComponent for SettingsWindow {
    type Init = ();
    type Input = ();
    type Output = ();

    view! {
        adw::PreferencesWindow {
            set_title: Some(&tr("Preferences")),
            set_default_size: (800, 700),
            set_modal: true,
            set_search_enabled: true,
            add_css_class: "settings-window",

            add: &page::appearance::build(&model.config),
            add: &page::thumbnails::build(&model.config),
            add: &page::behavior::build(&model.config),
            add: &page::terminal::build(&model.config),
            add: &page::system::build(&model.config),
            add: &page::shortcuts::build(&model.config),
        }
    }

    fn init(
        _init: Self::Init,
        root: Self::Root,
        _sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let config = crate::utils::load_config();
        let model = SettingsWindow { config };
        let widgets = view_output!();
        ComponentParts { model, widgets }
    }
}
