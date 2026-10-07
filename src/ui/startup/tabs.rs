use crate::model::{AppMsg, Config, FluxApp};
use adw::prelude::*;
use gtk::glib;
use relm4::prelude::*;
use std::path::PathBuf;

impl FluxApp {
    /// Builds the tab view, tab bar and the initial tab, wiring tab signals.
    pub(super) fn build_tabs(
        start_path: PathBuf,
        config: &Config,
        sender: &AsyncComponentSender<Self>,
    ) -> (adw::TabView, adw::TabBar, crate::ui::TabState) {
        let tab_view = adw::TabView::new();
        let tab_bar = adw::TabBar::builder()
            .view(&tab_view)
            .autohide(true)
            .expand_tabs(false)
            .css_classes(["inline"])
            .build();

        let mut initial_tab = crate::ui::TabState::new(
            1,
            start_path.clone(),
            config.default_list_mode,
            config.ui.default_sort,
        );
        initial_tab.is_initialized = true;

        initial_tab
            .files
            .view
            .set_single_click_activate(config.ui.single_click);
        let s_open = sender.clone();
        initial_tab.files.view.connect_activate(move |_, pos| {
            s_open.input(AppMsg::Open(Some(pos)));
        });
        let s_sel = sender.clone();
        if let Some(selection_model) = initial_tab
            .files
            .view
            .model()
            .and_downcast::<gtk::MultiSelection>()
        {
            selection_model.connect_selection_changed(move |_, _, _| {
                s_sel.input(AppMsg::SelectionChanged);
            });
        }

        Self::connect_tab_scroller(&initial_tab.scroller, sender);

        let initial_page = tab_view.append(&initial_tab.scroller);
        initial_page.set_title(&initial_tab.title);

        let s_tab_select = sender.clone();
        tab_view.connect_selected_page_notify(move |tv| {
            if let Some(selected_page) = tv.selected_page() {
                let position = tv.page_position(&selected_page) as usize;
                // Only request SwitchTab if the user clicked a different tab header
                s_tab_select.input(AppMsg::SwitchTab(position));
            }
        });

        tab_view.connect_close_page(move |tv, page| {
            if tv.n_pages() <= 1 {
                let app = gtk::Application::default();
                if let Some(win) = app.active_window() {
                    win.close();
                }
                return glib::Propagation::Stop;
            }
            // Let AdwTabView close and detach the page normally
            tv.close_page_finish(page, true);
            glib::Propagation::Stop
        });

        let s_tab_detached = sender.clone();
        tab_view.connect_page_detached(move |_tv, _page, position| {
            // Only notify our model AFTER the widget is fully detached
            s_tab_detached.input(AppMsg::CloseTab(Some(position as usize)));
        });
        (tab_view, tab_bar, initial_tab)
    }
}
