use super::matching::action_matches_mime;
use super::template::{parse_secondary_template, resolve_secondary_menu_template};
use crate::domain::model::{ActionsMsg, AppMsg, ArchiveMsg, CustomAction, FluxApp, WindowMsg};
use crate::ui::constants;
use crate::utils;
use adw::gdk;
use adw::prelude::*;
use gtk::gio;
use relm4::prelude::*;
use std::path::PathBuf;

impl FluxApp {
    pub fn handle_prepare_secondary_menu(
        &mut self,
        x: f64,
        y: f64,
        path: Option<PathBuf>,
        sender: &AsyncComponentSender<Self>,
    ) {
        self.active_item_path = path
            .clone()
            .or_else(|| self.get_selected_path())
            .or_else(|| Some(self.current_path.clone()));

        if let Some(ref target_path) = path {
            if let Some(model) = self
                .files
                .view
                .model()
                .and_then(|m| m.downcast::<gtk::MultiSelection>().ok())
            {
                let target_str = target_path
                    .to_string_lossy()
                    .trim_end_matches('/')
                    .to_string();

                for i in 0..self.files.len() {
                    if let Some(wrapper) = self.files.get(i) {
                        let item_str = wrapper
                            .borrow()
                            .path
                            .to_string_lossy()
                            .trim_end_matches('/')
                            .to_string();

                        if item_str == target_str {
                            if !model.selection().contains(i) {
                                model.select_item(i, true);
                            }
                            break;
                        }
                    }
                }
            }
        }

        let sender_bg = sender.clone();
        let target_path_bg = self.active_item_path.clone();

        relm4::spawn_blocking(move || {
            crate::hit!("secondary_menu::parse_template");
            let mime = target_path_bg
                .as_ref()
                .map(|p| utils::media::get_mime_type(p))
                .unwrap_or_else(|| constants::MIME_DIR.to_string());

            let actions = match resolve_secondary_menu_template(&mime) {
                Some(template_path) => parse_secondary_template(&template_path),
                None => Vec::new(),
            };

            sender_bg.input(AppMsg::Actions(ActionsMsg::ShowSecondaryMenu {
                x,
                y,
                path: target_path_bg,
                mime,
                actions,
            }));
        });
    }

    pub fn build_and_show_secondary_menu(
        &mut self,
        x: f64,
        y: f64,
        path: Option<PathBuf>,
        mime: String,
        actions: Vec<CustomAction>,
        sender: &AsyncComponentSender<Self>,
    ) {
        crate::hit!("secondary_menu::build_and_show");
        if actions.is_empty() {
            sender.input(AppMsg::Window(WindowMsg::ShowToast(crate::i18n::tr(
                "No contextual actions for this type",
            ))));
            return;
        }

        if path.is_some() {
            self.active_item_path = path.clone();
            self.active_item_line = path
                .as_ref()
                .and_then(|p| {
                    (0..self.files.len()).find_map(|i| {
                        self.files.get(i).and_then(|w| {
                            let item = w.borrow();
                            if item.path == *p {
                                Some(item.line_number)
                            } else {
                                None
                            }
                        })
                    })
                })
                .unwrap_or(0);
        }

        let root_menu = gio::Menu::new();
        let main_section = gio::Menu::new();
        let mut submenu_map: indexmap::IndexMap<String, gio::Menu> = indexmap::IndexMap::new();

        for action in &actions {
            if !action_matches_mime(action, &mime) {
                continue;
            }

            if action.command.contains("%l") && self.active_item_line == 0 {
                continue;
            }

            let gio_action = gio::SimpleAction::new(&action.action_name, None);
            let cmd_template = action.command.clone();
            let toast_msg = action.toast.clone();
            let sender_click = sender.clone();
            let target_path = path.clone();

            gio_action.connect_activate(move |_, _| {
                match (cmd_template.as_str(), &target_path) {
                    ("builtin::forget_archive_password", Some(target)) => {
                        sender_click.input(AppMsg::Archive(ArchiveMsg::ForgetArchivePassword(
                            target.clone(),
                        )));
                    }
                    ("builtin::forget_archive_password", None) => {}
                    _ => sender_click.input(AppMsg::Actions(ActionsMsg::ExecuteCommand(
                        cmd_template.clone(),
                    ))),
                }
                if let Some(ref msg) = toast_msg {
                    sender_click.input(AppMsg::Window(WindowMsg::ShowToast(msg.clone())));
                }
            });

            self.action_group.add_action(&gio_action);

            let full_name = format!("win.{}", action.action_name);

            if let Some(group_name) = &action.submenu {
                let menu = submenu_map.entry(group_name.clone()).or_default();
                menu.append(Some(&action.label), Some(&full_name));
            } else {
                main_section.append(Some(&action.label), Some(&full_name));
            }
        }

        root_menu.append_section(None, &main_section);
        for (name, menu) in submenu_map {
            root_menu.append_submenu(Some(&name), &menu);
        }

        self.context_menu_popover.set_menu_model(Some(&root_menu));
        self.context_menu_popover
            .set_pointing_to(Some(&gdk::Rectangle::new(x as i32, y as i32, 1, 1)));
        self.context_menu_popover.popup();
    }
}
