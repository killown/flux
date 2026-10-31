use super::compute::compute_preview_guarded;
use crate::app::state::FluxApp;
use crate::domain::model::{AppMsg, NavigationMsg, PreviewMsg, RightPanelType, SearchMsg};
use crate::ui::panels::preview::{registry, PreviewContent};
use adw::prelude::*;
use gtk::glib;
use relm4::prelude::*;
use std::path::PathBuf;
use std::sync::atomic::Ordering;

impl FluxApp {
    pub fn handle_toggle_preview(&mut self, sender: &AsyncComponentSender<Self>) {
        crate::hit!("handle_toggle_preview");
        if !self.panels.preview.visible {
            self.stop_video_preview();
            self.toggle_sidebar_right_panel(RightPanelType::Preview, sender);
            if self.config.ui.wide_preview {
                self.enter_wide_preview();
            }

            let Some(path) = self.get_selected_path() else {
                let view = self.tabs[self.active_tab_index].files.view.clone();
                glib::idle_add_local_once(move || {
                    if let Some(model) = view
                        .model()
                        .and_then(|m| m.downcast::<gtk::MultiSelection>().ok())
                    {
                        if model.n_items() > 0 {
                            model.select_item(0, true);
                        }
                    }
                });
                return;
            };

            let session = self
                .panels
                .preview
                .extras
                .preview_session
                .fetch_add(1, Ordering::SeqCst)
                + 1;
            sender.input(AppMsg::Preview(PreviewMsg::PreviewLoad { path, session }));
            return;
        }

        self.panels
            .preview
            .extras
            .preview_session
            .fetch_add(1, Ordering::SeqCst);

        if self.filters.mime.is_some() {
            self.filters.mime = None;
            self.filters.mime_from_space = false; // Still good practice to reset
            registry::set_mime_filter_active(false);
            sender.input(AppMsg::Navigation(NavigationMsg::Refresh));
        }

        self.stop_preview_media();
        let _ = registry::with(|h| h.reset_mute());
        let _ = registry::with(|h| h.release_folder_grid());
        self.exit_wide_preview();
        self.toggle_sidebar_right_panel(RightPanelType::Preview, sender);
    }

    pub fn handle_toggle_preview_and_filter_by_selected_mime(
        &mut self,
        sender: &AsyncComponentSender<Self>,
    ) {
        crate::hit!("handle_toggle_preview_and_filter_by_selected_mime");
        if !self.panels.preview.visible {
            self.handle_toggle_preview(sender);
        }

        if self.filters.mime.is_some() {
            return;
        }

        self.filters.mime_from_space = true;

        if self.get_selected_path().is_none() {
            let view = self.tabs[self.active_tab_index].files.view.clone();
            let sender_clone = sender.clone();
            glib::idle_add_local_once(move || {
                let Some(model) = view
                    .model()
                    .and_then(|m| m.downcast::<gtk::MultiSelection>().ok())
                else {
                    return;
                };
                if model.n_items() == 0 {
                    return;
                }
                model.select_item(0, true);
                sender_clone.input(AppMsg::Search(SearchMsg::FilterBySelectedMime));
            });
            return;
        }

        sender.input(AppMsg::Search(SearchMsg::FilterBySelectedMime));
    }

    pub fn handle_startup_preview(&mut self, sender: &AsyncComponentSender<Self>) {
        crate::hit!("handle_startup_preview");
        if !self.panels.preview.visible {
            self.handle_toggle_preview(sender);
        }
        self.enter_wide_preview();
        self.schedule_startup_preview_selection();
    }

    pub fn schedule_startup_preview_selection(&mut self) {
        crate::hit!("schedule_startup_preview_selection");
        if !self.preview.startup || !self.panels.preview.visible {
            return;
        }
        let view = self.tabs[self.active_tab_index].files.view.clone();
        let has_items = view
            .model()
            .and_then(|m| m.downcast::<gtk::MultiSelection>().ok())
            .is_some_and(|m| m.n_items() > 0);
        if !has_items {
            return;
        }
        self.preview.startup = false;
        glib::idle_add_local_once(move || {
            if let Some(model) = view
                .model()
                .and_then(|m| m.downcast::<gtk::MultiSelection>().ok())
            {
                if model.n_items() > 0 {
                    model.select_item(0, true);
                }
            }
        });
    }

    pub fn stop_preview_media(&mut self) {
        let _ = registry::with(|h| h.stop_media());
        self.stop_video_preview();
    }

    pub fn handle_toggle_preview_grid_focus(&mut self) {
        crate::hit!("handle_toggle_preview_grid_focus");
        let in_folder_grid = registry::with(|h| h.folder_grid_has_focus()).unwrap_or(false);

        if in_folder_grid {
            if let Some(tab) = self.tabs.get(self.active_tab_index) {
                tab.files.view.grab_focus();
            }
            return;
        }

        if !self.quick.list.is_empty() || !self.panels.preview.visible {
            return;
        }
        let entered = registry::with(|h| h.focus_folder_grid()).unwrap_or(false);
        let _ = entered; // focus_folder_grid should return bool
    }

    pub fn handle_focus_pane(&mut self, step: isize) {
        crate::hit!("handle_focus_pane");
        let Some(grid) = self
            .tabs
            .get(self.active_tab_index)
            .map(|tab| tab.files.view.clone().upcast::<gtk::Widget>())
        else {
            return;
        };

        let mut panes: Vec<(gtk::Widget, gtk::Widget)> = Vec::new();
        if self.sidebar_visible {
            if let Some(container) = self.sidebar_widget.clone() {
                panes.push((container, self.sidebar.widget().clone().upcast()));
            }
        }
        let grid_idx = panes.len();
        panes.push((grid.clone(), grid.clone()));

        let mut preview_idx = None;
        let right_panels = [
            (&self.panels.search.revealer, false),
            (&self.panels.tag.revealer, false),
            (&self.panels.diff.revealer, false),
            (&self.panels.location.revealer, false),
            (&self.panels.connect.revealer, false),
            (&self.panels.preview.revealer, true),
            (&self.panels.actions.revealer, false),
            (&self.panels.quick_list.revealer, false),
            (&self.panels.terminal.revealer, false),
        ];
        for (revealer, is_preview) in right_panels {
            let Some(revealer) = revealer.as_ref().filter(|r| r.reveals_child()) else {
                continue;
            };
            let widget = revealer.clone().upcast::<gtk::Widget>();
            panes.push((widget.clone(), widget));
            if is_preview {
                preview_idx = Some(panes.len() - 1);
            }
        }

        let focused = grid.root().and_then(|root| root.focus());
        let current = focused
            .as_ref()
            .and_then(|f| panes.iter().position(|(c, _)| f == c || f.is_ancestor(c)))
            .unwrap_or(grid_idx);

        let len = panes.len() as isize;
        let mut idx = current as isize;
        loop {
            idx += step;
            if idx < 0 || idx >= len {
                return;
            }
            let i = idx as usize;
            let (_, target) = &panes[i];
            let focused_ok = if i == grid_idx {
                target.grab_focus()
            } else if Some(i) == preview_idx
                && registry::with(|h| h.focus_folder_grid()).unwrap_or(false)
            {
                true
            } else {
                target.child_focus(gtk::DirectionType::TabForward)
            };
            if focused_ok {
                return;
            }
        }
    }

    pub fn handle_preview_load(&self, path: PathBuf, session: u64) {
        crate::hit!("handle_preview_load");
        if session
            != self
                .panels
                .preview
                .extras
                .preview_session
                .load(Ordering::SeqCst)
        {
            return;
        }

        if let Some(handle) = registry::with(Clone::clone) {
            handle.apply(&PreviewContent::Loading);
        }

        let session_arc = self.panels.preview.extras.preview_session.clone();
        let sender = crate::app::state::SENDER.get().cloned();
        relm4::spawn_blocking(move || {
            if session_arc.load(Ordering::SeqCst) != session {
                return;
            }
            let content = compute_preview_guarded(&path);
            if session_arc.load(Ordering::SeqCst) != session {
                return;
            }
            if let Some(s) = sender {
                let _ = s.send(AppMsg::Preview(PreviewMsg::PreviewReady {
                    path,
                    session,
                    content,
                }));
            }
        });
    }

    pub fn handle_preview_ready(&self, path: PathBuf, session: u64, content: PreviewContent) {
        crate::hit!("handle_preview_ready");
        if session
            != self
                .panels
                .preview
                .extras
                .preview_session
                .load(Ordering::SeqCst)
        {
            return;
        }
        if !self.panels.preview.visible {
            return;
        }
        if let Some(handle) = registry::with(Clone::clone) {
            // `apply` runs on the GTK main thread. Log anything that blocks
            // the UI so offenders are visible instead of guessed at.
            let t = std::time::Instant::now();
            handle.apply(&content);
            handle.set_mime_filter_active(self.filters.mime.is_some());
            let ms = t.elapsed().as_millis();
            if ms > 30 {
                eprintln!(
                    "[flux][preview] apply blocked UI {ms} ms: {}",
                    path.display()
                );
            }
        }
    }

    pub fn handle_set_preview_panel_width(&mut self, val: i32) {
        crate::hit!("handle_set_preview_panel_width");
        let clamped = val.clamp(300, 2400);
        self.config.ui.preview_panel_width = clamped;
        crate::utils::save_config(&self.config);
    }
}
