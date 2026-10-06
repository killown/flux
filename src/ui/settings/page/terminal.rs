use crate::i18n::tr;
use crate::model::{AppMsg, Config};
use adw::prelude::*;
use relm4::prelude::*;

pub fn build(config: &Config) -> adw::PreferencesPage {
    relm4::view! {
        page = adw::PreferencesPage {
            set_title: &tr("Terminal"),
            set_icon_name: Some("utilities-terminal-symbolic"),

            add = &adw::PreferencesGroup {
                set_title: &tr("Appearance & Execution"),
                set_description: Some(&tr("Customize the embedded terminal look, feel, and executable")),
                add = &adw::ActionRow {
                    set_title: &tr("Shell Executable"),
                    set_subtitle: &tr("Custom shell path (e.g., '/bin/zsh'). Leave blank for default"),
                    add_suffix = &gtk::Entry {
                        set_text: config.ui.terminal.shell.as_deref().unwrap_or(""),
                        set_valign: gtk::Align::Center,
                        set_placeholder_text: Some("/bin/bash"),
                        connect_changed => move |entry: &gtk::Entry| {
                            let val = entry.text().to_string();
                            let shell = if val.trim().is_empty() { None } else { Some(val.trim().to_string()) };
                            if let Some(s) = crate::model::SENDER.get() {
                                let _ = s.send(AppMsg::SetTerminalShell(shell));
                            }
                        }
                    }
                },
                add = &adw::ActionRow {
                    set_title: &tr("Height (lines)"),
                    set_subtitle: &tr("Number of character lines in the terminal"),
                    add_suffix = &gtk::SpinButton {
                        set_adjustment: &gtk::Adjustment::new(
                            config.ui.terminal.height as f64,
                            10.0, 200.0, 5.0, 0.0, 0.0
                        ),
                        set_numeric: true,
                        set_valign: gtk::Align::Center,
                        connect_value_changed => move |spin| {
                            if let Some(s) = crate::model::SENDER.get() {
                                let _ = s.send(AppMsg::SetTerminalHeight(spin.value() as i32));
                            }
                        }
                    }
                },
                add = &adw::ActionRow {
                    set_title: &tr("Font"),
                    set_subtitle: &tr("Pango font description"),
                    add_suffix = &gtk::Entry {
                        set_text: &config.ui.terminal.font,
                        set_valign: gtk::Align::Center,
                        connect_changed => move |entry| {
                            if let Some(s) = crate::model::SENDER.get() {
                                let _ = s.send(AppMsg::SetTerminalFont(entry.text().to_string()));
                            }
                        }
                    }
                },
                add = &adw::ActionRow {
                    set_title: &tr("Foreground Color"),
                    set_subtitle: &tr("Text color (hex, e.g., '#E5E5E5')"),
                    add_suffix = &gtk::Entry {
                        set_text: &config.ui.terminal.fg_color,
                        set_valign: gtk::Align::Center,
                        connect_changed => move |entry| {
                            if let Some(s) = crate::model::SENDER.get() {
                                let _ = s.send(AppMsg::SetTerminalFgColor(entry.text().to_string()));
                            }
                        }
                    }
                },
                add = &adw::ActionRow {
                    set_title: &tr("Background Color"),
                    set_subtitle: &tr("Background color (hex, e.g., '#1A1A1A')"),
                    add_suffix = &gtk::Entry {
                        set_text: &config.ui.terminal.bg_color,
                        set_valign: gtk::Align::Center,
                        connect_changed => move |entry| {
                            if let Some(s) = crate::model::SENDER.get() {
                                let _ = s.send(AppMsg::SetTerminalBgColor(entry.text().to_string()));
                            }
                        }
                    }
                },
            },
        }
    }

    page
}
