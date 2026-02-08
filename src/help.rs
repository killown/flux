use adw::prelude::*;
use relm4::prelude::*;

pub struct HelpWindow;

#[derive(Debug)]
pub enum HelpMsg {
    Close,
}

#[relm4::component(pub)]
impl SimpleComponent for HelpWindow {
    type Init = ();
    type Input = HelpMsg;
    type Output = ();

    view! {
        adw::Window {
            set_default_size: (500, 620),
            set_title: Some("flux - Shortcuts"),
            set_modal: true,

            gtk::Box {
                set_orientation: gtk::Orientation::Vertical,
                set_spacing: 0,

                adw::HeaderBar {},

                gtk::ScrolledWindow {
                    set_vexpand: true,
                    adw::PreferencesPage {
                        adw::PreferencesGroup {
                            set_title: "Navigation",
                            adw::ActionRow {
                                set_title: "F1",
                                set_subtitle: "Show this help"
                            },
                            adw::ActionRow {
                                set_title: "Ctrl + F",
                                set_subtitle: "Search files"
                            },
                            adw::ActionRow {
                                set_title: "Esc",
                                set_subtitle: "Return to path view / Close entries"
                            },
                        },
                        adw::PreferencesGroup {
                            set_title: "File Operations",
                            adw::ActionRow {
                                set_title: "F2",
                                set_subtitle: "Rename selected item"
                            },
                            adw::ActionRow {
                                set_title: "Ctrl + H",
                                set_subtitle: "Toggle hidden files"
                            },
                        },
                        adw::PreferencesGroup {
                            set_title: "View",
                            adw::ActionRow {
                                set_title: "Ctrl + Scroll",
                                set_subtitle: "Zoom icons"
                            },
                            adw::ActionRow {
                                set_title: "Ctrl + S",
                                set_subtitle: "Cycle sort mode"
                            },
                        }
                    }
                }
            }
        }
    }

    fn init(
        _: Self::Init,
        root: Self::Root,
        _sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let model = HelpWindow;
        let widgets = view_output!();
        ComponentParts { model, widgets }
    }

    fn update(&mut self, _msg: Self::Input, _sender: ComponentSender<Self>) {}
}
