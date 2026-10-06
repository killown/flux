use crate::model::PathSegment;
use crate::ui::constants;
use adw::prelude::*;
use relm4::prelude::*;
use std::path::PathBuf;

#[relm4::factory(pub)]
impl FactoryComponent for PathSegment {
    type Init = PathSegment;
    type Input = ();
    type Output = PathBuf;
    type ParentWidget = gtk::Box;
    type CommandOutput = ();

    fn init_model(init: Self::Init, _index: &DynamicIndex, _sender: FactorySender<Self>) -> Self {
        init
    }

    view! {
        #[root]
        gtk::Button {
            add_css_class: constants::BREADCRUMB_BTN_CLASS,
            #[wrap(Some)]
            set_child = &gtk::Label {
                #[watch]
                set_label: &self.name,
                set_ellipsize: gtk::pango::EllipsizeMode::End,
                set_max_width_chars: constants::BREADCRUMB_MAX_WIDTH_CHARS as i32,
                set_wrap: false,
            },

            // Left-click: standard navigation
            connect_clicked[sender, path = self.path.clone()] => move |_| {
                let _ = sender.output(path.clone());
            },

            // Middle-click: add this specific breadcrumb folder to Quick List
            add_controller = gtk::GestureClick {
                set_button: 2, // constants::MOUSE_MIDDLE
                connect_pressed[path = self.path.clone()] => move |gesture, _, _, _| {
                    gesture.set_state(gtk::EventSequenceState::Claimed);
                    if let Some(s) = crate::model::SENDER.get() {
                        let _ = s.send(crate::model::AppMsg::AddExclusive(Some(path.clone())));
                    }
                }
            }
        }
    }
}
