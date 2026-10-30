use crate::domain::model::{AppMsg, NavigationMsg, Config, FluxApp};
use crate::ui::FileItem;
use adw::prelude::*;
use relm4::prelude::*;
use relm4::typed_view::grid::TypedGridView;

impl FluxApp {
    /// Builds the main file grid view and wires selection/activation.
    pub(super) fn build_file_grid(
        config: &Config,
        sender: &AsyncComponentSender<Self>,
    ) -> TypedGridView<FileItem, gtk::MultiSelection> {
        crate::hit!("build_file_grid");
        let files = TypedGridView::<FileItem, gtk::MultiSelection>::new();
        files.view.set_enable_rubberband(true);
        files.view.set_single_click_activate(config.ui.single_click);

        let sender_clone = sender.clone();
        files.view.connect_activate(move |_, position| {
            sender_clone.input(AppMsg::Navigation(NavigationMsg::Open {
                position: Some(position),
                tab_id: None,
            }))
        });

        files.view.set_max_columns(20);
        files.view.set_min_columns(1);
        files.view.set_halign(gtk::Align::Fill);
        files.view.set_hexpand(true);

        files
    }
}
