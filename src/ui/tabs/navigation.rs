use super::state::TabState;
use crate::domain::model::{AppMsg, NavigationMsg, ViewMsg, FluxApp};
use gtk::prelude::*;
use relm4::prelude::*;

impl FluxApp {
    pub fn handle_next_tab(&mut self) {
        crate::hit!("handle_next_tab");
        if self.tabs.len() > 1 {
            let next = (self.active_tab_index + 1) % self.tabs.len();
            if (next as i32) < self.tab_view.n_pages() {
                let page = self.tab_view.nth_page(next as i32);
                self.tab_view.set_selected_page(&page);
            }
        }
    }

    pub fn handle_prev_tab(&mut self) {
        crate::hit!("handle_prev_tab");
        if self.tabs.len() > 1 {
            let prev = if self.active_tab_index == 0 {
                self.tabs.len() - 1
            } else {
                self.active_tab_index - 1
            };
            if (prev as i32) < self.tab_view.n_pages() {
                let page = self.tab_view.nth_page(prev as i32);
                self.tab_view.set_selected_page(&page);
            }
        }
    }

    pub(super) fn wire_tab_signals(&self, tab: &mut TabState, sender: &AsyncComponentSender<Self>) {
        crate::hit!("wire_tab_signals");
        tab.files
            .view
            .set_single_click_activate(self.config.ui.single_click);

        let tab_id = tab.id;

        let s_open = sender.clone();
        tab.files.view.connect_activate(move |_, pos| {
            s_open.input(AppMsg::Navigation(NavigationMsg::Open {
                position: Some(pos),
                tab_id: Some(tab_id),
            }));
        });

        let s_sel = sender.clone();
        if let Some(selection_model) = tab.files.view.model().and_downcast::<gtk::MultiSelection>()
        {
            selection_model.connect_selection_changed(move |_, _, _| {
                s_sel.input(AppMsg::View(ViewMsg::SelectionChanged {
                    tab_id: Some(tab_id),
                }));
            });
        }

        crate::ui::inputs::setup_tab_grid_controllers(&tab.files.view, sender);

        Self::connect_tab_scroller(&tab.scroller, sender);
    }
}
