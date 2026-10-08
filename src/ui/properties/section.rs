use adw::prelude::*;

use super::row::PropertyRow;
use relm4::factory::{FactoryComponent, FactoryVecDeque};
use relm4::prelude::*;

pub struct MetadataSection {
    title: String,
    rows: FactoryVecDeque<PropertyRow>,
}

#[relm4::factory(pub)]
impl FactoryComponent for MetadataSection {
    type Init = (String, Vec<(String, String)>);
    type Input = String;
    type Output = String;
    type ParentWidget = gtk::Box;
    type CommandOutput = ();

    view! {
        adw::PreferencesGroup {
            set_title: &self.title,
            #[local_ref]
            add = rows_widget -> adw::PreferencesGroup,
        }
    }

    fn init_model(init: Self::Init, _: &DynamicIndex, sender: FactorySender<Self>) -> Self {
        let mut rows = FactoryVecDeque::builder()
            .launch(adw::PreferencesGroup::new())
            .forward(sender.input_sender(), |msg| msg);
        {
            let mut guard = rows.guard();
            for (k, v) in init.1 {
                guard.push_back((k, v));
            }
        }
        Self {
            title: init.0,
            rows,
        }
    }

    fn init_widgets(
        &mut self,
        _: &DynamicIndex,
        _root: Self::Root,
        _: &gtk::Widget,
        _: FactorySender<Self>,
    ) -> Self::Widgets {
        let rows_widget = self.rows.widget();
        let widgets = view_output!();
        widgets
    }

    fn update(&mut self, msg: Self::Input, sender: FactorySender<Self>) {
        let _ = sender.output(msg);
    }
}
