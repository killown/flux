//! Sidebar rebuilding and the right-side panel toggle.

use crate::model::{AppMsg, FluxApp, RightPanelType};
use crate::ui::{constants, SidebarPlace};
use adw::prelude::*;
use relm4::prelude::*;
use std::path::PathBuf;

impl FluxApp {
    /// Rebuilds the sidebar navigation list from XDG directories, mounts, and configuration.
    pub fn refresh_sidebar(&mut self) {
        let mut guard = self.sidebar.guard();
        guard.clear();

        let recents_place = || crate::ui::SidebarPlace {
            name: crate::i18n::tr("Recents"),
            icon: "document-open-recent-symbolic".to_string(),
            path: std::path::PathBuf::from(crate::ui::constants::RECENT_URI),
            is_mount: false,
            is_section_label: false,
        };

        let get_xdg_name = |p: &PathBuf| {
            p.file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| p.to_string_lossy().to_string())
        };

        //Core XDG Directories - Conditioned on show_xdg_dirs config
        if self.config.ui.show_xdg_dirs {
            if let Some(p) = dirs::home_dir() {
                guard.push_back(SidebarPlace {
                    name: get_xdg_name(&p),
                    icon: "user-home-symbolic".to_string(),
                    path: p,
                    is_mount: false,
                    is_section_label: false,
                });
            }
            if let Some(p) = dirs::desktop_dir() {
                guard.push_back(SidebarPlace {
                    name: get_xdg_name(&p),
                    icon: "user-desktop-symbolic".to_string(),
                    path: p,
                    is_mount: false,
                    is_section_label: false,
                });
            }
            if let Some(p) = dirs::download_dir() {
                guard.push_back(SidebarPlace {
                    name: get_xdg_name(&p),
                    icon: "folder-download-symbolic".to_string(),
                    path: p,
                    is_mount: false,
                    is_section_label: false,
                });
            }
            if let Some(p) = dirs::document_dir() {
                guard.push_back(SidebarPlace {
                    name: get_xdg_name(&p),
                    icon: "folder-documents-symbolic".to_string(),
                    path: p,
                    is_mount: false,
                    is_section_label: false,
                });
            }
            if let Some(p) = dirs::picture_dir() {
                guard.push_back(SidebarPlace {
                    name: get_xdg_name(&p),
                    icon: "folder-pictures-symbolic".to_string(),
                    path: p,
                    is_mount: false,
                    is_section_label: false,
                });
            }
            if let Some(p) = dirs::video_dir() {
                guard.push_back(SidebarPlace {
                    name: get_xdg_name(&p),
                    icon: "folder-videos-symbolic".to_string(),
                    path: p,
                    is_mount: false,
                    is_section_label: false,
                });
            }
        }

        // Custom Sidebar logic
        let show_recents = self.config.ui.show_recents;
        let recents_row = self.config.ui.recents_row;
        for (idx, custom) in self.config.sidebar.iter().enumerate() {
            if show_recents && idx == recents_row {
                guard.push_back(recents_place());
            }
            if custom.kind.as_deref() == Some("label") {
                guard.push_back(SidebarPlace {
                    name: custom.name.clone(),
                    icon: String::new(),
                    path: PathBuf::new(),
                    is_mount: false,
                    is_section_label: true,
                });
                continue;
            }

            let path = if custom.path.starts_with('~') {
                dirs::home_dir()
                    .map(|h| PathBuf::from(custom.path.replace('~', &h.to_string_lossy())))
                    .unwrap_or_else(|| PathBuf::from(&custom.path))
            } else {
                PathBuf::from(&custom.path)
            };

            let mut name = custom.name.clone();
            // Translate Trash if it's the default English name
            if custom.path == constants::TRASH_URI && name == "Trash" {
                name = crate::i18n::tr("Trash");
            } else if custom.path == "tags://" && name == "Tags" {
                name = crate::i18n::tr("Tags");
            }

            guard.push_back(SidebarPlace {
                name,
                icon: custom.icon.clone(),
                path,
                is_mount: false,
                is_section_label: false,
            });
        }

        if show_recents && recents_row >= self.config.sidebar.len() {
            guard.push_back(recents_place());
        }

        // Mounts - Offloaded to background thread to prevent blocking the UI loop
        if let Some(sender) = crate::model::SENDER.get().cloned() {
            relm4::spawn_blocking(move || {
                let mounts = crate::services::mounts::get_system_mounts();
                let _ = sender.send(AppMsg::SystemMountsReady(mounts));
            });
        }
    }

    /// Hides every right-side panel except `keep`, so only one is open at a time.
    fn hide_other_right_panels(&mut self, keep: RightPanelType) {
        if keep != RightPanelType::Tag && self.tag_panel_visible {
            if let Some(ref r) = self.tag_panel_revealer {
                r.set_reveal_child(false);
                r.set_visible(false);
            }
            self.tag_panel_visible = false;
        }
        if keep != RightPanelType::Search && self.search_panel_visible {
            if let Some(ref r) = self.search_panel_revealer {
                r.set_reveal_child(false);
                r.set_visible(false);
            }
            self.search_panel_visible = false;
        }
        if keep != RightPanelType::Diff && self.diff_panel_visible {
            if let Some(ref r) = self.diff_panel_revealer {
                r.set_reveal_child(false);
                r.set_visible(false);
            }
            self.diff_panel_visible = false;
            self.active_diff_target = None;
        }
        if keep != RightPanelType::Location && self.location_panel_visible {
            if let Some(ref r) = self.location_panel_revealer {
                r.set_reveal_child(false);
                r.set_visible(false);
            }
            self.location_panel_visible = false;
        }
    }

    pub fn toggle_sidebar_right_panel(
        &mut self,
        panel_type: RightPanelType,
        sender: &AsyncComponentSender<Self>,
    ) {
        // Ensure mutually exclusive right-side panels
        self.hide_other_right_panels(panel_type);

        let (revealer, initialized, visible, is_search) = match panel_type {
            RightPanelType::Tag => (
                self.tag_panel_revealer.clone(),
                &mut self.tag_panel_initialized,
                &mut self.tag_panel_visible,
                false,
            ),
            RightPanelType::Search => (
                self.search_panel_revealer.clone(),
                &mut self.search_panel_initialized,
                &mut self.search_panel_visible,
                true,
            ),
            RightPanelType::Diff => (
                self.diff_panel_revealer.clone(),
                &mut self.diff_panel_initialized,
                &mut self.diff_panel_visible,
                false,
            ),
            RightPanelType::Location => (
                self.location_panel_revealer.clone(),
                &mut self.location_panel_initialized,
                &mut self.location_panel_visible,
                false,
            ),
        };

        let Some(revealer) = revealer else {
            return;
        };

        if !*initialized {
            let panel = match panel_type {
                RightPanelType::Tag => {
                    let tags = self.state_db.list_all_tags().unwrap_or_default();
                    crate::ui::panels::build_tag_panel(
                        tags,
                        self.config.ui.tag_panel_width,
                        sender.clone(),
                    )
                }
                RightPanelType::Search => crate::ui::panels::build_search_panel(
                    self.config.ui.search_panel_width,
                    sender.clone(),
                ),
                RightPanelType::Diff => {
                    let buf = gtk::TextBuffer::new(None);
                    self.diff_text_buffer = Some(buf.clone());
                    crate::ui::panels::build_diff_panel(
                        self.config.ui.diff_panel_width,
                        buf,
                        sender.clone(),
                    )
                }
                RightPanelType::Location => {
                    let (panel, entry) = crate::ui::panels::build_location_panel(
                        &self.current_path.to_string_lossy(),
                        self.state_db.clone(),
                        sender.clone(),
                    );
                    self.location_entry = Some(entry);
                    panel
                }
            };
            revealer.set_child(Some(&panel));
            *initialized = true;
        } else if panel_type == RightPanelType::Location && !*visible {
            // Panel is built once, so refresh the entry with the current path on each open.
            if let Some(ref entry) = self.location_entry {
                entry.set_text(&self.current_path.to_string_lossy());
            }
        }

        *visible = !*visible;
        revealer.set_visible(*visible);
        revealer.set_reveal_child(*visible);

        if *visible {
            if let Some(panel_box) = revealer.child() {
                let mut next = panel_box.first_child();
                let mut focused = false;
                while let Some(w) = next {
                    if let Some(entry) = w.downcast_ref::<gtk::Entry>() {
                        entry.grab_focus();
                        focused = true;
                        break;
                    }
                    if let Some(entry) = w.downcast_ref::<gtk::SearchEntry>() {
                        entry.grab_focus();
                        focused = true;
                        break;
                    }
                    if let Some(inner) = w.first_child() {
                        if let Some(entry) = inner.downcast_ref::<gtk::Entry>() {
                            entry.grab_focus();
                            focused = true;
                            break;
                        }
                        if let Some(entry) = inner.downcast_ref::<gtk::SearchEntry>() {
                            entry.grab_focus();
                            focused = true;
                            break;
                        }
                    }
                    next = w.next_sibling();
                }
                if !focused {
                    panel_box.grab_focus();
                }
            }
        } else if is_search {
            if self.is_content_searching || self.last_search_was_advanced {
                self.last_search_was_advanced = false;
                sender.input(AppMsg::CancelContentSearch);
                sender.input(AppMsg::ClearExtensionFilter);
                sender.input(AppMsg::Refresh);
            }
        } else if panel_type == RightPanelType::Diff {
            self.active_diff_target = None;
        } else if panel_type == RightPanelType::Tag {
            sender.input(AppMsg::CancelContentSearch);
            sender.input(AppMsg::Refresh);
        }
    }
}
