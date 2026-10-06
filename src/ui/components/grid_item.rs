use super::file_item::FileItem;
use super::widgets::FileWidgets;

impl relm4::typed_view::grid::RelmGridItem for FileItem {
    type Root = gtk::Overlay;
    type Widgets = FileWidgets;

    /// Builds the widget hierarchy for a grid cell (see `setup.rs`).
    fn setup(item: &gtk::ListItem) -> (Self::Root, Self::Widgets) {
        Self::build_widgets(item)
    }

    /// Syncs the cell's widgets with this item (see `bind.rs`).
    fn bind(&mut self, widgets: &mut Self::Widgets, root: &mut Self::Root) {
        self.bind_widgets(widgets, root)
    }

    /// Resets per-cell state before the cell is recycled (see `bind.rs`).
    fn unbind(&mut self, widgets: &mut Self::Widgets, root: &mut Self::Root) {
        self.unbind_widgets(widgets, root)
    }
}
