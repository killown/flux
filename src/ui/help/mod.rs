mod format;
mod page;

use crate::i18n::tr;
use crate::model::Config;
use adw::prelude::*;
use relm4::prelude::*;

pub struct HelpWindow {
    config: Config,
}

#[derive(Debug)]
pub enum HelpMsg {}

#[relm4::component(pub)]
impl SimpleComponent for HelpWindow {
    type Init = ();
    type Input = HelpMsg;
    type Output = ();

    view! {
        adw::PreferencesWindow {
            set_title: Some(&tr("flux - Keyboard Shortcuts")),
            set_default_size: (600, 700),
            set_modal: true,
            set_search_enabled: true,
            set_resizable: true,

            add = &page::navigation::build(&model.config),
            add = &page::quick_list::build(&model.config),
            add = &page::search::build(&model.config),
            add = &page::system_view::build(&model.config),
            add = &page::application::build(&model.config),
        }
    }

    fn init(
        _init: Self::Init,
        _root: Self::Root,
        _sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let config = crate::utils::load_config();
        let model = HelpWindow { config };
        let widgets = view_output!();
        ComponentParts { model, widgets }
    }

    fn update(&mut self, _msg: Self::Input, _sender: ComponentSender<Self>) {}
}
