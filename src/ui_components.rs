use adw::prelude::*;
use relm4::prelude::*;
use std::path::PathBuf;
use adw::gdk;

#[derive(Debug, Clone)]
pub struct FileItem {
    pub name: String,
    pub icon: adw::gio::Icon,
    pub thumbnail: Option<gdk::Texture>,
    #[allow(dead_code)]
    pub is_dir: bool,
    pub path: PathBuf,
    pub icon_size: i32,
}

pub struct FileWidgets {
    pub icon_widget: gtk::Image,
    pub label: gtk::Label,
}

impl relm4::typed_view::grid::RelmGridItem for FileItem {
    type Root = gtk::Box;
    type Widgets = FileWidgets;

    fn setup(_item: &gtk::ListItem) -> (Self::Root, Self::Widgets) {
        relm4::view! {
            #[root]
            root = gtk::Box {
                set_orientation: gtk::Orientation::Vertical,
                set_halign: gtk::Align::Center,
                set_valign: gtk::Align::Center,
                add_css_class: "flux-card",

                add_controller = gtk::GestureClick {
                    set_button: 3,
                    connect_released[sender = crate::model::SENDER.clone()] => move |gesture, _, x, y| {
                        if let Some(sender) = sender.get() {
                            if let Some(widget) = gesture.widget() {
                                let path_str = widget.widget_name();
                                let path = PathBuf::from(path_str.to_string());

                                if let Some(root_widget) = widget.root() {
                                    let (root_x, root_y) = widget.translate_coordinates(&root_widget, x, y).unwrap_or((x, y));
                                    sender.send(crate::model::AppMsg::PrepareContextMenu(
                                        root_x, 
                                        root_y, 
                                        Some(path)
                                    )).ok();
                                }
                            }
                        }
                    }
                },

                #[name = "icon_widget"]
                gtk::Image {
                    add_css_class: "thumbnail",
                },

                #[name = "label"]
                gtk::Label { 
                    set_wrap: true,
                    set_justify: gtk::Justification::Center,
                    set_max_width_chars: 14,
                    set_ellipsize: gtk::pango::EllipsizeMode::End,
                    add_css_class: "flux-label",
                }
            }
        }
        (root, FileWidgets { icon_widget, label })
    }

    fn bind(&mut self, widgets: &mut Self::Widgets, _root: &mut Self::Root) {
        widgets.label.set_label(&self.name);
        widgets.icon_widget.set_pixel_size(self.icon_size);

        if let Some(ref texture) = self.thumbnail {
            widgets.icon_widget.set_paintable(Some(texture));
        } else {
            widgets.icon_widget.set_paintable(Option::<&gdk::Texture>::None);
            widgets.icon_widget.set_from_gicon(&self.icon);
        }
        _root.set_widget_name(&self.path.to_string_lossy());
    }
}

#[derive(Debug)]
pub struct SidebarPlace {
    pub name: String,
    pub icon: String,
    pub path: PathBuf,
}

#[relm4::factory(pub)]
impl FactoryComponent for SidebarPlace {
    type Init = SidebarPlace;
    type Input = ();
    type Output = PathBuf;
    type ParentWidget = gtk::ListBox;
    type CommandOutput = ();

    view! {
        #[root]
        gtk::ListBoxRow {
            add_css_class: "sidebar-row",
            set_selectable: false,
            add_controller = gtk::GestureClick {
                connect_released[sender, path = self.path.clone()] => move |_, _, _, _| {
                    let _ = sender.output(path.clone());
                }
            },
            gtk::Box {
                set_orientation: gtk::Orientation::Horizontal,
                set_spacing: 12,
                gtk::Image { set_icon_name: Some(&self.icon) },
                gtk::Label { set_label: &self.name, add_css_class: "sidebar-label" }
            }
        }
    }

    fn init_model(init: Self::Init, _: &DynamicIndex, _: FactorySender<Self>) -> Self {
        init
    }
}
