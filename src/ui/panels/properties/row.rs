use adw::prelude::*;

use relm4::factory::FactoryComponent;
use relm4::prelude::*;

pub struct PropertyRow {
    key: String,
    value: String,
}

#[relm4::factory(pub)]
impl FactoryComponent for PropertyRow {
    type Init = (String, String);
    type Input = ();
    type Output = String;
    type ParentWidget = adw::PreferencesGroup;
    type CommandOutput = ();

    view! {
        adw::ActionRow {
            set_title: &self.key,
            set_subtitle: &self.value,
            set_activatable: true,
            add_suffix = &gtk::Image::from_icon_name("edit-copy-symbolic"),
            connect_activated[sender, val = self.value.clone()] => move |_| {
                let _ = sender.output(val.clone());
            }
        }
    }

    fn init_model(init: Self::Init, _: &DynamicIndex, _: FactorySender<Self>) -> Self {
        Self {
            key: init.0,
            value: init.1,
        }
    }
}
