use adw::prelude::*;
use relm4::prelude::*;

pub struct HelpWindow;

#[derive(Debug)]
pub enum HelpMsg {}

#[relm4::component(pub)]
impl SimpleComponent for HelpWindow {
    type Init = ();
    type Input = HelpMsg;
    type Output = ();

    view! {
        adw::Window {
            set_default_size: (550, 720),
            set_title: Some("flux - Shortcuts"),
            set_modal: true,
            set_resizable: false,

            gtk::Box {
                set_orientation: gtk::Orientation::Vertical,

                adw::HeaderBar {},

                gtk::ScrolledWindow {
                    set_vexpand: true,
                    set_propagate_natural_height: true,

                    adw::PreferencesPage {
                        // --- NAVIGATION ---
                        adw::PreferencesGroup {
                            set_title: "Navigation",
                            adw::ActionRow {
                                set_title: "Ctrl + ]",
                                set_subtitle: "Navigate to previous folder in parent directory"
                            },
                            adw::ActionRow {
                                set_title: "Ctrl + [",
                                set_subtitle: "Navigate to next folder in parent directory"
                            },
                            adw::ActionRow {
                                set_title: "Backspace",
                                set_subtitle: "Go to parent directory"
                            },
                        },

                        // --- QUICK LIST ---
                        adw::PreferencesGroup {
                            set_title: "Quick List (Exclusive List)",
                            adw::ActionRow {
                                set_title: "Insert",
                                set_subtitle: "Add selection or current folder to list"
                            },
                            adw::ActionRow {
                                set_title: "Tab",
                                set_subtitle: "Cycle to the next folder in the list"
                            },
                            adw::ActionRow {
                                set_title: "Ctrl + 1-9",
                                set_subtitle: "Jump to specific list index"
                            },
                            adw::ActionRow {
                                set_title: "Ctrl + End",
                                set_subtitle: "Clear the entire list"
                            },
                        },

                        // --- SYSTEM & VIEW ---
                        adw::PreferencesGroup {
                            set_title: "System & View",
                            adw::ActionRow {
                                set_title: "F1",
                                set_subtitle: "Show this help"
                            },
                            adw::ActionRow {
                                set_title: "F2",
                                set_subtitle: "Rename selected item"
                            },
                            adw::ActionRow {
                                set_title: "Ctrl + F",
                                set_subtitle: "Search files"
                            },
                            adw::ActionRow {
                                set_title: "Esc",
                                set_subtitle: "Back to path / Close search"
                            },
                            adw::ActionRow {
                                set_title: "Ctrl + H",
                                set_subtitle: "Toggle hidden files"
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
        _root: Self::Root,
        _sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let model = HelpWindow;
        let widgets = view_output!();
        ComponentParts { model, widgets }
    }

    fn update(&mut self, _msg: Self::Input, _sender: ComponentSender<Self>) {}
}
