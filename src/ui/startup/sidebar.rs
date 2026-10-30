use crate::domain::model::{
    AppMsg, AppSidebarMsg, Config, FluxApp, NavigationMsg, QuickListMsg, TabsMsg,
};
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
        crate::hit!("build_sidebar");
        let listbox = gtk::ListBox::default();
        let sidebar = FactoryVecDeque::builder().launch(listbox.clone()).forward(
            sender.input_sender(),
            |msg| match msg {
                crate::ui::SidebarMsg::Navigate(path) => {
                    AppMsg::Navigation(NavigationMsg::Navigate(path))
                }
                crate::ui::SidebarMsg::OpenInNewTab(path) => {
                    AppMsg::Tabs(TabsMsg::NewTab(Some(path)))
                }
                crate::ui::SidebarMsg::OpenInNewWindow(path) => {
                    crate::utils::helpers::open_new_instance(&path);
                    // Idempotent re-render. This used to be `ClearExclusive`, which
                    // would now wipe the saved Quick List.
                    AppMsg::QuickList(QuickListMsg::RebuildQuickPanel)
                }
                crate::ui::SidebarMsg::Remove(path) => {
                    AppMsg::Sidebar(AppSidebarMsg::RemoveFromSidebar(path))
                }
                crate::ui::SidebarMsg::ChangeIcon(path) => {
                    AppMsg::Sidebar(AppSidebarMsg::ShowSidebarIconPicker(path))
                }
                crate::ui::SidebarMsg::Rename { path, current_name } => {
                    AppMsg::Sidebar(AppSidebarMsg::PromptSidebarRename { path, current_name })
                }
                crate::ui::SidebarMsg::Reorder { from, to } => {
                    AppMsg::Sidebar(AppSidebarMsg::ReorderSidebar { from, to })
                }
                crate::ui::SidebarMsg::PinAt {
                    path,
                    before,
                    label_name,
                } => AppMsg::Sidebar(AppSidebarMsg::PinFolderAt {
                    path,
                    before,
                    label_name,
                }),
                crate::ui::SidebarMsg::DropMove {
                    source_paths,
                    dest_path,
                } => AppMsg::Sidebar(AppSidebarMsg::SidebarDropMove {
                    source_paths,
                    dest_path,
                }),
                crate::ui::SidebarMsg::RenameSection(name) => {
                    AppMsg::Sidebar(AppSidebarMsg::PromptSidebarRenameSection {
                        old_name: name.clone(),
                        current_name: name,
                    })
                }
                crate::ui::SidebarMsg::RemoveSection(name) => {
                    AppMsg::Sidebar(AppSidebarMsg::RemoveSidebarSection(name))
                }
            },
        );

        let volume_monitor = {
            crate::hit!("build_sidebar:volume_monitor_get");
            gio::VolumeMonitor::get()
        };
        {
            crate::hit!("build_sidebar:volume_monitor_signals");
            crate::ui::sidebar_network::connect_volume_monitor_signals(
                sender.input_sender().clone(),
            );
        }

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
                            let (rx, _) =
                                crate::utils::helpers::translate_point(&handle, &root, x, 0.0)
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
                                    crate::utils::helpers::translate_point(
                                        &handle, &root, curr_x, 0.0,
                                    )
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
                    sender_c.input(AppMsg::Sidebar(AppSidebarMsg::SetSidebarWidth(final_width)));
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
