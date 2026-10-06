/// Collection of GTK widgets utilized by a [FileItem] within the grid view.
pub struct FileWidgets {
    pub card_box: gtk::Box,
    pub icon_widget: gtk::Image,
    pub video_widget: gtk::Video,
    pub preview_stack: gtk::Stack,
    pub lock_icon: gtk::Image,
    pub label: gtk::Label,
    pub stack: gtk::Stack,
    pub drag_source: gtk::DragSource,
    pub drop_target: gtk::DropTarget,
    pub info_label: gtk::Label,
    pub label_scroller: gtk::ScrolledWindow,
    pub scale_css_provider: Option<gtk::CssProvider>,
    pub git_badge: gtk::Label,
}
