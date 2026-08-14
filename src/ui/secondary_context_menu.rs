use crate::model::{AppMsg, CustomAction, FluxApp};
use crate::ui::constants;
use crate::utils;
use crate::utils::config::split_mime_cmd;
use adw::gdk;
use adw::prelude::*;
use gtk::gio;
use relm4::prelude::*;
use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;

// ══════════════════════════════════════════════════════════════════════════════
//  1  MIME helper utilities
// ══════════════════════════════════════════════════════════════════════════════

/// Splits `"image/png"` → `("image", "png")`.
#[inline]
fn split_mime(mime: &str) -> (&str, &str) {
    match mime.find('/') {
        Some(idx) => (&mime[..idx], &mime[idx + 1..]),
        None => (mime, ""),
    }
}

/// Replaces characters that are illegal in filenames with `_`.
#[inline]
fn sanitise_for_filename(s: &str) -> String {
    s.chars()
        .map(|c| {
            if c.is_alphanumeric() || matches!(c, '-' | '+' | '.') {
                c
            } else {
                '_'
            }
        })
        .collect()
}

// ══════════════════════════════════════════════════════════════════════════════
//  2  Template resolver
// ══════════════════════════════════════════════════════════════════════════════

/// Returns `~/.config/flux/menus/` (or `~/.config/flux/` as fallback)
fn flux_menus_dir() -> PathBuf {
    let base = dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("~/.config"))
        .join("flux");

    let sub = base.join("menus");
    if sub.is_dir() {
        sub
    } else {
        base
    }
}

pub fn resolve_secondary_menu_template(mime: &str) -> Option<PathBuf> {
    let menus_dir = flux_menus_dir();
    let (category, subtype) = split_mime(mime);

    let safe_cat = sanitise_for_filename(category);
    let safe_sub = sanitise_for_filename(subtype);

    let mut candidates: Vec<PathBuf> = Vec::with_capacity(3);

    if !safe_sub.is_empty() && safe_sub != "all" {
        candidates.push(menus_dir.join(format!("menu-{safe_cat}-{safe_sub}.rs")));
    }

    if !safe_cat.is_empty() {
        candidates.push(menus_dir.join(format!("menu-{safe_cat}-all.rs")));
    }

    candidates.push(menus_dir.join("menu-all.rs"));

    candidates.into_iter().find(|p| p.is_file())
}

// ══════════════════════════════════════════════════════════════════════════════
//  3  Template parser
// ══════════════════════════════════════════════════════════════════════════════

pub fn parse_secondary_template(path: &Path) -> Vec<CustomAction> {
    let src = match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("[SecondaryMenu] cannot read {:?}: {}", path, e);
            return Vec::new();
        }
    };

    let stem = path
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy()
        .replace(['-', ' '], "_");

    let mut actions = Vec::new();

    for (line_no, raw_line) in src.lines().enumerate() {
        let line = raw_line.trim();

        if line.is_empty() || line.starts_with("//") || line.starts_with('#') {
            continue;
        }

        let Some((left, right)) = line.split_once("=>") else {
            eprintln!(
                "[SecondaryMenu] {:?}:{} – missing `=>`, skipping: {}",
                path,
                line_no + 1,
                line
            );
            continue;
        };

        let full_label = left.trim().trim_matches('"');

        let (submenu, label) = if let Some(pos) = full_label.find(" > ") {
            (
                Some(full_label[..pos].trim().to_string()),
                full_label[pos + 3..].trim().to_string(),
            )
        } else {
            (None, full_label.to_string())
        };

        let Some((mimes_part, cmd_part, toast, no_command_dialog)) = split_mime_cmd(right) else {
            eprintln!(
                "[SecondaryMenu] {:?}:{} – malformed RHS, skipping: {}",
                path,
                line_no + 1,
                line
            );
            continue;
        };

        let mime_types: Vec<String> = mimes_part
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();

        actions.push(CustomAction {
            label,
            submenu,
            action_name: format!("sec_{}_{}", stem, line_no),
            command: cmd_part,
            mime_types,
            toast,
            no_command_dialog,
        });
    }

    actions
}

// ══════════════════════════════════════════════════════════════════════════════
//  4  GestureClick controller
// ══════════════════════════════════════════════════════════════════════════════

pub fn setup_secondary_menu_gesture(
    widget: &impl IsA<gtk::Widget>,
    sender: relm4::AsyncComponentSender<FluxApp>,
) {
    let gesture = gtk::GestureClick::new();
    gesture.set_button(0);
    gesture.set_propagation_phase(gtk::PropagationPhase::Capture);

    let sender_g = sender.clone();
    let widget_weak = widget.as_ref().downgrade();

    gesture.connect_pressed(|g, _n_press, _x, _y| {
        if g.current_button() == 3 {
            let state = g
                .current_event()
                .map(|e| e.modifier_state())
                .unwrap_or(gdk::ModifierType::empty());

            if state.contains(gdk::ModifierType::CONTROL_MASK) {
                g.set_state(gtk::EventSequenceState::Claimed);
            }
        }
    });

    gesture.connect_released(move |g, _n_press, x, y| {
        if g.current_button() != 3 {
            return;
        }

        let state = g
            .current_event()
            .map(|e| e.modifier_state())
            .unwrap_or(gdk::ModifierType::empty());

        if !state.contains(gdk::ModifierType::CONTROL_MASK) {
            return;
        }

        g.set_state(gtk::EventSequenceState::Claimed);

        let mut path: Option<PathBuf> = None;
        let mut rel_x = x;
        let mut rel_y = y;

        if let Some(root) = widget_weak.upgrade() {
            if let Some(picked) = root.pick(x, y, gtk::PickFlags::DEFAULT) {
                // Translate window-relative coordinates (x, y) into GridView space
                if let Some(popover_parent) = picked.ancestor(gtk::GridView::static_type()) {
                    if let Some((tx, ty)) = root.translate_coordinates(&popover_parent, x, y) {
                        rel_x = tx;
                        rel_y = ty;
                    }
                }

                let mut cur: Option<gtk::Widget> = Some(picked);
                while let Some(w) = cur {
                    let data_path: Option<PathBuf> = unsafe {
                        w.data::<Rc<RefCell<Option<PathBuf>>>>("active_path_cell")
                            .map(|ptr| ptr.as_ref().clone())
                            .and_then(|rc| rc.borrow().clone())
                    };
                    if let Some(p) = data_path {
                        path = Some(p);
                        break;
                    }

                    let name = w.widget_name().to_string();
                    if name.starts_with('/')
                        || name.starts_with("trash://")
                        || name.starts_with("smb://")
                        || name.starts_with("sftp://")
                        || name.starts_with("ftp://")
                        || name.starts_with("nfs://")
                        || name.starts_with("archive://")
                    {
                        path = Some(PathBuf::from(name));
                        break;
                    }
                    cur = w.parent();
                }
            }
        }

        sender_g.input(AppMsg::PrepareSecondaryMenu {
            x: rel_x,
            y: rel_y,
            path,
        });
    });

    widget.as_ref().add_controller(gesture);
}

// ══════════════════════════════════════════════════════════════════════════════
//  5  FluxApp impls
// ══════════════════════════════════════════════════════════════════════════════

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
            let mime = target_path_bg
                .as_ref()
                .map(|p| utils::get_mime_type(p))
                .unwrap_or_else(|| constants::MIME_DIR.to_string());

            let actions = match resolve_secondary_menu_template(&mime) {
                Some(template_path) => parse_secondary_template(&template_path),
                None => Vec::new(),
            };

            sender_bg.input(AppMsg::ShowSecondaryMenu {
                x,
                y,
                path: target_path_bg,
                mime,
                actions,
            });
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
        if actions.is_empty() {
            return;
        }

        if path.is_some() {
            self.active_item_path = path;
        }

        let root_menu = gio::Menu::new();
        let main_section = gio::Menu::new();
        let mut submenu_map: indexmap::IndexMap<String, gio::Menu> = indexmap::IndexMap::new();

        for action in &actions {
            if !self
                .menu_actions
                .iter()
                .any(|a| a.action_name == action.action_name)
            {
                self.menu_actions.push(action.clone());
            }

            let mut matches = false;
            'outer: for allowed_mime in &action.mime_types {
                let requirements: Vec<&str> = allowed_mime.split('+').collect();
                for req in &requirements {
                    let hit = match req.trim() {
                        "all" | constants::FILTER_ALL => true,
                        "image/all" | "image/*" => mime.starts_with("image/"),
                        "video/all" | "video/*" => mime.starts_with("video/"),
                        "audio/all" | "audio/*" => mime.starts_with("audio/"),
                        "font/all" | "font/*" => mime.starts_with("font/"),
                        "application/all" | "application/*" => mime.starts_with("application/"),
                        "text/all" | "text/*" => {
                            mime.starts_with("text/")
                                || gio::content_type_is_a(&mime, constants::MIME_TEXT)
                                || mime == constants::MIME_EMPTY
                        }
                        constants::FILTER_FOLDER | "directory" => mime == constants::MIME_DIR,
                        constants::FILTER_FILE => mime != constants::MIME_DIR,
                        t if t.ends_with('/') => mime.starts_with(t),
                        t => t == mime,
                    };
                    if hit {
                        matches = true;
                        break 'outer;
                    }
                }
            }

            if !matches {
                continue;
            }

            let gio_action = gio::SimpleAction::new(&action.action_name, None);
            let cmd_template = action.command.clone();
            let toast_msg = action.toast.clone();
            let sender_click = sender.clone();

            gio_action.connect_activate(move |_, _| {
                sender_click.input(AppMsg::ExecuteCommand(cmd_template.clone()));
                if let Some(ref msg) = toast_msg {
                    sender_click.input(AppMsg::ShowToast(msg.clone()));
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
