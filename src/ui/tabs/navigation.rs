use super::state::TabState;
use crate::model::{AppMsg, FluxApp};
use gtk::prelude::*;
use relm4::prelude::*;

impl FluxApp {
    pub fn handle_next_tab(&mut self) {
        if self.tabs.len() > 1 {
            let next = (self.active_tab_index + 1) % self.tabs.len();
            if (next as i32) < self.tab_view.n_pages() {
                let page = self.tab_view.nth_page(next as i32);
                self.tab_view.set_selected_page(&page);
            }
        }
    }

    pub fn handle_prev_tab(&mut self) {
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
        tab.files
            .view
            .set_single_click_activate(self.config.ui.single_click);

        let s_open = sender.clone();
        tab.files.view.connect_activate(move |_, pos| {
            s_open.input(AppMsg::Open(Some(pos)));
        });

        let s_sel = sender.clone();
        if let Some(selection_model) = tab.files.view.model().and_downcast::<gtk::MultiSelection>()
        {
            selection_model.connect_selection_changed(move |_, _, _| {
                s_sel.input(AppMsg::SelectionChanged);
            });
        }

        Self::connect_tab_scroller(&tab.scroller, sender);
    }
}
