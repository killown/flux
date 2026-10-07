use crate::model::{AppMsg, Config, FluxApp};
use crate::ui::SidebarPlace;
use adw::prelude::*;
use gtk::gio;
use relm4::factory::FactoryVecDeque;
use relm4::prelude::*;

impl FluxApp {
    /// Builds the sidebar list, volume monitor, network section and resizable sidebar root.
    pub(super) fn build_sidebar(
        config: &Config,
        sender: &AsyncComponentSender<Self>,
    ) -> (
        FactoryVecDeque<SidebarPlace>,
        gio::VolumeMonitor,
        gtk::Box,
        gtk::Box,
    ) {
        let listbox = gtk::ListBox::default();
        let sidebar = FactoryVecDeque::builder().launch(listbox.clone()).forward(
            sender.input_sender(),
            |msg| match msg {
                crate::ui::SidebarMsg::Navigate(path) => AppMsg::Navigate(path),
                crate::ui::SidebarMsg::OpenInNewTab(path) => AppMsg::NewTab(Some(path)),
                crate::ui::SidebarMsg::OpenInNewWindow(path) => {
                    crate::utils::helpers::open_new_instance(&path);
                    AppMsg::ClearExclusive
                }
                crate::ui::SidebarMsg::Remove(path) => AppMsg::RemoveFromSidebar(path),
                crate::ui::SidebarMsg::ChangeIcon(path) => AppMsg::ShowSidebarIconPicker(path),
                crate::ui::SidebarMsg::Rename { path, current_name } => {
                    AppMsg::PromptSidebarRename { path, current_name }
                }
                crate::ui::SidebarMsg::Reorder { from, to } => AppMsg::ReorderSidebar { from, to },
                crate::ui::SidebarMsg::PinAt {
                    path,
                    before,
                    label_name,
                } => AppMsg::PinFolderAt {
                    path,
                    before,
                    label_name,
                },
                crate::ui::SidebarMsg::DropMove {
                    source_paths,
                    dest_path,
                } => AppMsg::SidebarDropMove {
                    source_paths,
                    dest_path,
                },
                crate::ui::SidebarMsg::RenameSection(name) => AppMsg::PromptSidebarRenameSection {
                    old_name: name.clone(),
                    current_name: name,
                },
                crate::ui::SidebarMsg::RemoveSection(name) => AppMsg::RemoveSidebarSection(name),
            },
        );

        let volume_monitor = gio::VolumeMonitor::get();
        crate::ui::sidebar_network::connect_volume_monitor_signals(sender.input_sender().clone());

        let network_section = crate::ui::sidebar_network::build_network_section(
            &config.network_bookmarks,
            sender.input_sender().clone(),
        );

        let sidebar_container = gtk::Box::new(gtk::Orientation::Vertical, 0);
        let initial_sidebar_width = config.ui.sidebar_width.clamp(160, 500);
        sidebar_container.set_width_request(initial_sidebar_width);

        // ── Drag Handle (Right Edge) ─────────────────────────────────────────
        let sidebar_resize_handle = gtk::Separator::builder()
            .orientation(gtk::Orientation::Vertical)
            .css_classes(["sidebar-resize-handle"])
            .cursor(&gtk::gdk::Cursor::from_name("col-resize", None).unwrap())
            .build();

        let sidebar_drag_gesture = gtk::GestureDrag::new();
        let start_sidebar_width = std::rc::Rc::new(std::cell::Cell::new(initial_sidebar_width));
        let start_sidebar_root_x = std::rc::Rc::new(std::cell::Cell::new(0.0));

        {
            let sidebar_weak = sidebar_container.downgrade();
            let start_w = start_sidebar_width.clone();
            let start_rx = start_sidebar_root_x.clone();

            sidebar_drag_gesture.connect_drag_begin(move |gesture, x, _| {
                if let Some(s) = sidebar_weak.upgrade() {
                    start_w.set(s.width());
                    if let Some(root) = s.root() {
                        if let Some(handle) = gesture.widget() {
                            let (rx, _) = handle
                                .translate_coordinates(&root, x, 0.0)
                                .unwrap_or((x, 0.0));
                            start_rx.set(rx);
                        }
                    }
                }
            });
        }

        {
            let sidebar_weak = sidebar_container.downgrade();
            let start_w = start_sidebar_width.clone();
            let start_rx = start_sidebar_root_x.clone();

            sidebar_drag_gesture.connect_drag_update(move |gesture, _, _| {
                if let Some(s) = sidebar_weak.upgrade() {
                    if let Some(root) = s.root() {
                        if let Some(handle) = gesture.widget() {
                            if let Some((curr_x, _)) = gesture.point(None) {
                                if let Some((curr_root_x, _)) =
                                    handle.translate_coordinates(&root, curr_x, 0.0)
                                {
                                    // Moving to the right expands the left sidebar
                                    let delta_x = curr_root_x - start_rx.get();
                                    let new_w = (start_w.get() + delta_x as i32).clamp(160, 500);

                                    if s.width_request() != new_w {
                                        s.set_width_request(new_w);
                                    }
                                }
                            }
                        }
                    }
                }
            });
        }

        {
            let sidebar_weak = sidebar_container.downgrade();
            let sender_c = sender.clone();
            sidebar_drag_gesture.connect_drag_end(move |_, _, _| {
                if let Some(s) = sidebar_weak.upgrade() {
                    let final_width = s.width().clamp(160, 500);
                    s.set_width_request(final_width);
                    sender_c.input(AppMsg::SetSidebarWidth(final_width));
                }
            });
        }

        sidebar_resize_handle.add_controller(sidebar_drag_gesture);

        let sidebar_root = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(0)
            .hexpand(false)
            .halign(gtk::Align::Start)
            .build();

        sidebar_root.append(&sidebar_container);
        sidebar_root.append(&sidebar_resize_handle);
        (sidebar, volume_monitor, network_section, sidebar_root)
    }
}
