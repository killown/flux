use crate::i18n::tr;
use super::shortcut_picker::ShortcutPicker;
use crate::model::{AppMsg, Config};
use adw::prelude::*;
use relm4::prelude::*;

pub fn build(config: &Config) -> adw::PreferencesPage {
    crate::hit!("build");
    relm4::view! {
        page = adw::PreferencesPage {
            set_title: &tr("Shortcuts"),
            set_icon_name: Some("org.gnome.Settings-keyboard-symbolic"),

            add = &adw::PreferencesGroup {
                set_title: &tr("Navigation"),
                set_description: Some(&tr("Shortcuts for navigating between directories")),
                add = &adw::ActionRow {
                    set_title: &tr("Back"),
                    set_subtitle: &tr("Go to previous directory in history"),
                    add_suffix = &ShortcutPicker::new(
                        config.shortcuts.back.clone(),
                        move |val| {
                            if let Some(s) = crate::model::SENDER.get() {
                                let _ = s.send(AppMsg::SetShortcut("back".to_string(), val));
                            }
                        },
                    ),
                },
                add = &adw::ActionRow {
                    set_title: &tr("Forward"),
                    set_subtitle: &tr("Go to next directory in history"),
                    add_suffix = &ShortcutPicker::new(
                        config.shortcuts.forward.clone(),
                        move |val| {
                            if let Some(s) = crate::model::SENDER.get() {
                                let _ = s.send(AppMsg::SetShortcut("forward".to_string(), val));
                            }
                        },
                    ),
                },
                add = &adw::ActionRow {
                    set_title: &tr("Home"),
                    set_subtitle: &tr("Navigate to user's home directory"),
                    add_suffix = &ShortcutPicker::new(
                        config.shortcuts.home.clone(),
                        move |val| {
                            if let Some(s) = crate::model::SENDER.get() {
                                let _ = s.send(AppMsg::SetShortcut("home".to_string(), val));
                            }
                        },
                    ),
                },
                add = &adw::ActionRow {
                    set_title: &tr("Root"),
                    set_subtitle: &tr("Navigate to filesystem root directory"),
                    add_suffix = &ShortcutPicker::new(
                        config.shortcuts.root.clone(),
                        move |val| {
                            if let Some(s) = crate::model::SENDER.get() {
                                let _ = s.send(AppMsg::SetShortcut("root".to_string(), val));
                            }
                        },
                    ),
                },
                add = &adw::ActionRow {
                    set_title: &tr("Open"),
                    set_subtitle: &tr("Open selected file or directory"),
                    add_suffix = &ShortcutPicker::new(
                        config.shortcuts.open.clone(),
                        move |val| {
                            if let Some(s) = crate::model::SENDER.get() {
                                let _ = s.send(AppMsg::SetShortcut("open".to_string(), val));
                            }
                        },
                    ),
                },
                add = &adw::ActionRow {
                    set_title: &tr("Cycle By Letter Re-anchor"),
                    set_subtitle: &tr("Re-anchor the Ctrl+Alt+letter folder jump"),
                    add_suffix = &ShortcutPicker::new(
                        config.shortcuts.cycle_by_letter_reanchor.clone(),
                        move |val| {
                            if let Some(s) = crate::model::SENDER.get() {
                                let _ = s.send(AppMsg::SetShortcut("cycle_by_letter_reanchor".to_string(), val));
                            }
                        },
                    ),
                },
                add = &adw::ActionRow {
                    set_title: &tr("Cycle By Letter Cancel"),
                    set_subtitle: &tr("Cancel the Ctrl+Alt+letter folder jump"),
                    add_suffix = &ShortcutPicker::new(
                        config.shortcuts.cycle_by_letter_cancel.clone(),
                        move |val| {
                            if let Some(s) = crate::model::SENDER.get() {
                                let _ = s.send(AppMsg::SetShortcut("cycle_by_letter_cancel".to_string(), val));
                            }
                        },
                    ),
                },
                add = &adw::ActionRow {
                    set_title: &tr("Focus Pane Left"),
                    set_subtitle: &tr("Move keyboard focus to the pane on the left"),
                    add_suffix = &ShortcutPicker::new(
                        config.shortcuts.focus_pane_left.clone(),
                        move |val| {
                            if let Some(s) = crate::model::SENDER.get() {
                                let _ = s.send(AppMsg::SetShortcut("focus_pane_left".to_string(), val));
                            }
                        },
                    ),
                },
                add = &adw::ActionRow {
                    set_title: &tr("Focus Pane Right"),
                    set_subtitle: &tr("Move keyboard focus to the pane on the right"),
                    add_suffix = &ShortcutPicker::new(
                        config.shortcuts.focus_pane_right.clone(),
                        move |val| {
                            if let Some(s) = crate::model::SENDER.get() {
                                let _ = s.send(AppMsg::SetShortcut("focus_pane_right".to_string(), val));
                            }
                        },
                    ),
                },
            },

            add = &adw::PreferencesGroup {
                set_title: &tr("Tabs"),
                set_description: Some(&tr("Shortcuts for managing browser tabs")),
                add = &adw::ActionRow {
                    set_title: &tr("New Tab"),
                    set_subtitle: &tr("Open a new tab"),
                    add_suffix = &ShortcutPicker::new(
                        config.shortcuts.new_tab.clone(),
                        move |val| {
                            if let Some(s) = crate::model::SENDER.get() {
                                let _ = s.send(AppMsg::SetShortcut("new_tab".to_string(), val));
                            }
                        },
                    ),
                },
                add = &adw::ActionRow {
                    set_title: &tr("Close Tab"),
                    set_subtitle: &tr("Close current tab"),
                    add_suffix = &ShortcutPicker::new(
                        config.shortcuts.close_tab.clone(),
                        move |val| {
                            if let Some(s) = crate::model::SENDER.get() {
                                let _ = s.send(AppMsg::SetShortcut("close_tab".to_string(), val));
                            }
                        },
                    ),
                },
                add = &adw::ActionRow {
                    set_title: &tr("Next Tab"),
                    set_subtitle: &tr("Switch to next tab"),
                    add_suffix = &ShortcutPicker::new(
                        config.shortcuts.next_tab.clone(),
                        move |val| {
                            if let Some(s) = crate::model::SENDER.get() {
                                let _ = s.send(AppMsg::SetShortcut("next_tab".to_string(), val));
                            }
                        },
                    ),
                },
                add = &adw::ActionRow {
                    set_title: &tr("Previous Tab"),
                    set_subtitle: &tr("Switch to previous tab"),
                    add_suffix = &ShortcutPicker::new(
                        config.shortcuts.prev_tab.clone(),
                        move |val| {
                            if let Some(s) = crate::model::SENDER.get() {
                                let _ = s.send(AppMsg::SetShortcut("prev_tab".to_string(), val));
                            }
                        },
                    ),
                },
            },

            add = &adw::PreferencesGroup {
                set_title: &tr("File Operations"),
                set_description: Some(&tr("Shortcuts for managing files")),
                add = &adw::ActionRow {
                    set_title: &tr("Delete"),
                    set_subtitle: &tr("Move selected items to trash"),
                    add_suffix = &ShortcutPicker::new(
                        config.shortcuts.delete.clone(),
                        move |val| {
                            if let Some(s) = crate::model::SENDER.get() {
                                let _ = s.send(AppMsg::SetShortcut("delete".to_string(), val));
                            }
                        },
                    ),
                },
                add = &adw::ActionRow {
                    set_title: &tr("Rename"),
                    set_subtitle: &tr("Rename selected item"),
                    add_suffix = &ShortcutPicker::new(
                        config.shortcuts.rename.clone(),
                        move |val| {
                            if let Some(s) = crate::model::SENDER.get() {
                                let _ = s.send(AppMsg::SetShortcut("rename".to_string(), val));
                            }
                        },
                    ),
                },
                add = &adw::ActionRow {
                    set_title: &tr("Properties"),
                    set_subtitle: &tr("Inspect file properties or metadata"),
                    add_suffix = &ShortcutPicker::new(
                        config.shortcuts.open_properties.clone(),
                        move |val| {
                            if let Some(s) = crate::model::SENDER.get() {
                                let _ = s.send(AppMsg::SetShortcut("open_properties".to_string(), val));
                            }
                        },
                    ),
                },
                add = &adw::ActionRow {
                    set_title: &tr("Copy Path"),
                    set_subtitle: &tr("Copy absolute path of selected items to clipboard"),
                    add_suffix = &ShortcutPicker::new(
                        config.shortcuts.copy_path.clone(),
                        move |val| {
                            if let Some(s) = crate::model::SENDER.get() {
                                let _ = s.send(AppMsg::SetShortcut("copy_path".to_string(), val));
                            }
                        },
                    ),
                },
                add = &adw::ActionRow {
                    set_title: &tr("Create Symbolic Link"),
                    set_subtitle: &tr("Create symlink from clipboard path"),
                    add_suffix = &ShortcutPicker::new(
                        config.shortcuts.create_symlink.clone(),
                        move |val| {
                            if let Some(s) = crate::model::SENDER.get() {
                                let _ = s.send(AppMsg::SetShortcut("create_symlink".to_string(), val));
                            }
                        },
                    ),
                },
                add = &adw::ActionRow {
                    set_title: &tr("Create Hard Link"),
                    set_subtitle: &tr("Create hardlink from clipboard path"),
                    add_suffix = &ShortcutPicker::new(
                        config.shortcuts.create_hardlink.clone(),
                        move |val| {
                            if let Some(s) = crate::model::SENDER.get() {
                                let _ = s.send(AppMsg::SetShortcut("create_hardlink".to_string(), val));
                            }
                        },
                    ),
                },
                add = &adw::ActionRow {
                    set_title: &tr("Copy"),
                    set_subtitle: &tr("Copy selected items to clipboard"),
                    add_suffix = &ShortcutPicker::new(
                        config.shortcuts.copy.clone(),
                        move |val| {
                            if let Some(s) = crate::model::SENDER.get() {
                                let _ = s.send(AppMsg::SetShortcut("copy".to_string(), val));
                            }
                        },
                    ),
                },
                add = &adw::ActionRow {
                    set_title: &tr("Cut"),
                    set_subtitle: &tr("Cut selected items to clipboard"),
                    add_suffix = &ShortcutPicker::new(
                        config.shortcuts.cut.clone(),
                        move |val| {
                            if let Some(s) = crate::model::SENDER.get() {
                                let _ = s.send(AppMsg::SetShortcut("cut".to_string(), val));
                            }
                        },
                    ),
                },
                add = &adw::ActionRow {
                    set_title: &tr("Paste"),
                    set_subtitle: &tr("Paste items from clipboard"),
                    add_suffix = &ShortcutPicker::new(
                        config.shortcuts.paste.clone(),
                        move |val| {
                            if let Some(s) = crate::model::SENDER.get() {
                                let _ = s.send(AppMsg::SetShortcut("paste".to_string(), val));
                            }
                        },
                    ),
                },
                add = &adw::ActionRow {
                    set_title: &tr("Undo"),
                    set_subtitle: &tr("Undo the last file operation"),
                    add_suffix = &ShortcutPicker::new(
                        config.shortcuts.undo.clone(),
                        move |val| {
                            if let Some(s) = crate::model::SENDER.get() {
                                let _ = s.send(AppMsg::SetShortcut("undo".to_string(), val));
                            }
                        },
                    ),
                },
                add = &adw::ActionRow {
                    set_title: &tr("Redo"),
                    set_subtitle: &tr("Redo the last undone file operation"),
                    add_suffix = &ShortcutPicker::new(
                        config.shortcuts.redo.clone(),
                        move |val| {
                            if let Some(s) = crate::model::SENDER.get() {
                                let _ = s.send(AppMsg::SetShortcut("redo".to_string(), val));
                            }
                        },
                    ),
                },
            },

            add = &adw::PreferencesGroup {
                set_title: &tr("View & Customization"),
                set_description: Some(&tr("Shortcuts for controlling the interface")),
                add = &adw::ActionRow {
                    set_title: &tr("Toggle Header Bar"),
                    set_subtitle: &tr("Show or hide the top navigation toolbar"),
                    add_suffix = &ShortcutPicker::new(
                        config.shortcuts.toggle_header.clone(),
                        move |val| {
                            if let Some(s) = crate::model::SENDER.get() {
                                let _ = s.send(AppMsg::SetShortcut("toggle_header".to_string(), val));
                            }
                        },
                    ),
                },
                add = &adw::ActionRow {
                    set_title: &tr("Refresh"),
                    set_subtitle: &tr("Reload current directory"),
                    add_suffix = &ShortcutPicker::new(
                        config.shortcuts.refresh.clone(),
                        move |val| {
                            if let Some(s) = crate::model::SENDER.get() {
                                let _ = s.send(AppMsg::SetShortcut("refresh".to_string(), val));
                            }
                        },
                    ),
                },
                add = &adw::ActionRow {
                    set_title: &tr("Search"),
                    set_subtitle: &tr("Focus search/filter bar"),
                    add_suffix = &ShortcutPicker::new(
                        config.shortcuts.search.clone(),
                        move |val| {
                            if let Some(s) = crate::model::SENDER.get() {
                                let _ = s.send(AppMsg::SetShortcut("search".to_string(), val));
                            }
                        },
                    ),
                },
                add = &adw::ActionRow {
                    set_title: &tr("Toggle Hidden"),
                    set_subtitle: &tr("Show/hide hidden files and folders"),
                    add_suffix = &ShortcutPicker::new(
                        config.shortcuts.toggle_hidden.clone(),
                        move |val| {
                            if let Some(s) = crate::model::SENDER.get() {
                                let _ = s.send(AppMsg::SetShortcut("toggle_hidden".to_string(), val));
                            }
                        },
                    ),
                },
                add = &adw::ActionRow {
                    set_title: &tr("Toggle Folders First"),
                    set_subtitle: &tr("Toggle folders first placement in current directory"),
                    add_suffix = &ShortcutPicker::new(
                        config.shortcuts.toggle_folders_first.clone(),
                        move |val| {
                            if let Some(s) = crate::model::SENDER.get() {
                                let _ = s.send(AppMsg::SetShortcut("toggle_folders_first".to_string(), val));
                            }
                        },
                    ),
                },
                add = &adw::ActionRow {
                    set_title: &tr("Cycle Sort"),
                    set_subtitle: &tr("Cycle through sorting modes"),
                    add_suffix = &ShortcutPicker::new(
                        config.shortcuts.cycle_sort.clone(),
                        move |val| {
                            if let Some(s) = crate::model::SENDER.get() {
                                let _ = s.send(AppMsg::SetShortcut("cycle_sort".to_string(), val));
                            }
                        },
                    ),
                },
                add = &adw::ActionRow {
                    set_title: &tr("Toggle Sort Order"),
                    set_subtitle: &tr("Toggle ascending/descending order"),
                    add_suffix = &ShortcutPicker::new(
                        config.shortcuts.toggle_sort_order.clone(),
                        move |val| {
                            if let Some(s) = crate::model::SENDER.get() {
                                let _ = s.send(AppMsg::SetShortcut("toggle_sort_order".to_string(), val));
                            }
                        },
                    ),
                },
                add = &adw::ActionRow {
                    set_title: &tr("Change Icon"),
                    set_subtitle: &tr("Open icon picker for the current folder"),
                    add_suffix = &ShortcutPicker::new(
                        config.shortcuts.change_icon.clone(),
                        move |val| {
                            if let Some(s) = crate::model::SENDER.get() {
                                let _ = s.send(AppMsg::SetShortcut("change_icon".to_string(), val));
                            }
                        },
                    ),
                },
                add = &adw::ActionRow {
                    set_title: &tr("Reset Icon"),
                    set_subtitle: &tr("Reset folder icon to system default"),
                    add_suffix = &ShortcutPicker::new(
                        config.shortcuts.reset_icon.clone(),
                        move |val| {
                            if let Some(s) = crate::model::SENDER.get() {
                                let _ = s.send(AppMsg::SetShortcut("reset_icon".to_string(), val));
                            }
                        },
                    ),
                },
                add = &adw::ActionRow {
                    set_title: &tr("Toggle Status Bar"),
                    set_subtitle: &tr("Show or hide the status bar"),
                    add_suffix = &ShortcutPicker::new(
                        config.shortcuts.toggle_statusbar.clone(),
                        move |val| {
                            if let Some(s) = crate::model::SENDER.get() {
                                let _ = s.send(AppMsg::SetShortcut("toggle_statusbar".to_string(), val));
                            }
                        },
                    ),
                },
                add = &adw::ActionRow {
                    set_title: &tr("Toggle Sidebar"),
                    set_subtitle: &tr("Show or hide the sidebar"),
                    add_suffix = &ShortcutPicker::new(
                        config.shortcuts.toggle_sidebar.clone(),
                        move |val| {
                            if let Some(s) = crate::model::SENDER.get() {
                                let _ = s.send(AppMsg::SetShortcut("toggle_sidebar".to_string(), val));
                            }
                        },
                    ),
                },
                add = &adw::ActionRow {
                    set_title: &tr("Toggle Terminal"),
                    set_subtitle: &tr("Show or hide the embedded terminal"),
                    add_suffix = &ShortcutPicker::new(
                        config.shortcuts.toggle_terminal.clone(),
                        move |val| {
                            if let Some(s) = crate::model::SENDER.get() {
                                let _ = s.send(AppMsg::SetShortcut("toggle_terminal".to_string(), val));
                            }
                        },
                    ),
                },
                add = &adw::ActionRow {
                    set_title: &tr("Toggle List Mode"),
                    set_subtitle: &tr("Switch between grid and list view"),
                    add_suffix = &ShortcutPicker::new(
                        config.shortcuts.toggle_list_mode.clone(),
                        move |val| {
                            if let Some(s) = crate::model::SENDER.get() {
                                let _ = s.send(AppMsg::SetShortcut("toggle_list_mode".to_string(), val));
                            }
                        },
                    ),
                },
                add = &adw::ActionRow {
                    set_title: &tr("Toggle Wide Preview"),
                    set_subtitle: &tr("Toggle the wide preview layout"),
                    add_suffix = &ShortcutPicker::new(
                        config.shortcuts.toggle_wide_preview.clone(),
                        move |val| {
                            if let Some(s) = crate::model::SENDER.get() {
                                let _ = s.send(AppMsg::SetShortcut("toggle_wide_preview".to_string(), val));
                            }
                        },
                    ),
                },
                add = &adw::ActionRow {
                    set_title: &tr("Toggle Filter Bar"),
                    set_subtitle: &tr("Show or hide the glob-pattern filter bar"),
                    add_suffix = &ShortcutPicker::new(
                        config.shortcuts.toggle_filter_bar.clone(),
                        move |val| {
                            if let Some(s) = crate::model::SENDER.get() {
                                let _ = s.send(AppMsg::SetShortcut("toggle_filter_bar".to_string(), val));
                            }
                        },
                    ),
                },
                add = &adw::ActionRow {
                    set_title: &tr("Cycle Filter Mode"),
                    set_subtitle: &tr("Cycle through the filter modes"),
                    add_suffix = &ShortcutPicker::new(
                        config.shortcuts.cycle_filter_mode.clone(),
                        move |val| {
                            if let Some(s) = crate::model::SENDER.get() {
                                let _ = s.send(AppMsg::SetShortcut("cycle_filter_mode".to_string(), val));
                            }
                        },
                    ),
                },
                add = &adw::ActionRow {
                    set_title: &tr("Filter by MIME Type"),
                    set_subtitle: &tr("Toggle MIME filter on the selected item"),
                    add_suffix = &ShortcutPicker::new(
                        config.shortcuts.filter_by_mime.clone(),
                        move |val| {
                            if let Some(s) = crate::model::SENDER.get() {
                                let _ = s.send(AppMsg::SetShortcut("filter_by_mime".to_string(), val));
                            }
                        },
                    ),
                },
            },

            add = &adw::PreferencesGroup {
                set_title: &tr("Application"),
                set_description: Some(&tr("Shortcuts for opening preferences and application tools")),
                add = &adw::ActionRow {
                    set_title: &tr("Settings"),
                    set_subtitle: &tr("Open preferences window"),
                    add_suffix = &ShortcutPicker::new(
                        config.shortcuts.settings.clone(),
                        move |val| {
                            if let Some(s) = crate::model::SENDER.get() {
                                let _ = s.send(AppMsg::SetShortcut("settings".to_string(), val));
                            }
                        },
                    ),
                },
                add = &adw::ActionRow {
                    set_title: &tr("Menu Editor"),
                    set_subtitle: &tr("Open context menu editor"),
                    add_suffix = &ShortcutPicker::new(
                        config.shortcuts.menu_editor.clone(),
                        move |val| {
                            if let Some(s) = crate::model::SENDER.get() {
                                let _ = s.send(AppMsg::SetShortcut("menu_editor".to_string(), val));
                            }
                        },
                    ),
                },
                add = &adw::ActionRow {
                    set_title: &tr("Quit"),
                    set_subtitle: &tr("Exit the application"),
                    add_suffix = &ShortcutPicker::new(
                        config.shortcuts.quit.clone(),
                        move |val| {
                            if let Some(s) = crate::model::SENDER.get() {
                                let _ = s.send(AppMsg::SetShortcut("quit".to_string(), val));
                            }
                        },
                    ),
                },
                add = &adw::ActionRow {
                    set_title: &tr("Debug Window"),
                    set_subtitle: &tr("Open the debug window"),
                    add_suffix = &ShortcutPicker::new(
                        config.shortcuts.open_debug_window.clone(),
                        move |val| {
                            if let Some(s) = crate::model::SENDER.get() {
                                let _ = s.send(AppMsg::SetShortcut("open_debug_window".to_string(), val));
                            }
                        },
                    ),
                },
            },

            add = &adw::PreferencesGroup {
                set_title: &tr("Panels"),
                set_description: Some(&tr("Shortcuts for toggling side panels")),
                add = &adw::ActionRow {
                    set_title: &tr("Tag Panel"),
                    set_subtitle: &tr("Show or hide the tag panel"),
                    add_suffix = &ShortcutPicker::new(
                        config.shortcuts.toggle_tag_panel.clone(),
                        move |val| {
                            if let Some(s) = crate::model::SENDER.get() {
                                let _ = s.send(AppMsg::SetShortcut("toggle_tag_panel".to_string(), val));
                            }
                        },
                    ),
                },
                add = &adw::ActionRow {
                    set_title: &tr("Actions Panel"),
                    set_subtitle: &tr("Show or hide the contextual actions panel"),
                    add_suffix = &ShortcutPicker::new(
                        config.shortcuts.toggle_actions_panel.clone(),
                        move |val| {
                            if let Some(s) = crate::model::SENDER.get() {
                                let _ = s.send(AppMsg::SetShortcut("toggle_actions_panel".to_string(), val));
                            }
                        },
                    ),
                },
                add = &adw::ActionRow {
                    set_title: &tr("Connect Panel"),
                    set_subtitle: &tr("Show or hide the connect-to-server panel"),
                    add_suffix = &ShortcutPicker::new(
                        config.shortcuts.toggle_connect_panel.clone(),
                        move |val| {
                            if let Some(s) = crate::model::SENDER.get() {
                                let _ = s.send(AppMsg::SetShortcut("toggle_connect_panel".to_string(), val));
                            }
                        },
                    ),
                },
                add = &adw::ActionRow {
                    set_title: &tr("Location Panel"),
                    set_subtitle: &tr("Show or hide the location panel"),
                    add_suffix = &ShortcutPicker::new(
                        config.shortcuts.toggle_location_panel.clone(),
                        move |val| {
                            if let Some(s) = crate::model::SENDER.get() {
                                let _ = s.send(AppMsg::SetShortcut("toggle_location_panel".to_string(), val));
                            }
                        },
                    ),
                },
            },
        }
    }

    page
}
