use crate::model::{AppMsg, FluxApp};
use crate::ui::FileItem;
use adw::gio;
use adw::prelude::*;
use relm4::prelude::*;
use std::sync::OnceLock;

/// Set `FLUX_DEBUG_NETWORK=1` to enable per-entry tracing on stderr.
/// Off by default so a slow mount doesn't drown the journal in
/// thousands of debug lines on every navigation.
fn net_debug_enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| std::env::var_os("FLUX_DEBUG_NETWORK").is_some())
}

macro_rules! net_debug {
    ($($arg:tt)*) => {
        if net_debug_enabled() {
            eprintln!($($arg)*);
        }
    };
}

impl FluxApp {
    pub fn handle_network_loaded(
        &mut self,
        uri: String,
        contexts: Vec<crate::model::FileLoadContext>,
    ) {
        net_debug!(
            "[network] handle_network_loaded uri={uri:?} current_path={:?} contexts={}",
            self.current_path,
            contexts.len()
        );
        if self.current_path != std::path::Path::new(&uri) {
            eprintln!(
                "[network]   BAILING: current_path ({:?}) != uri ({:?})",
                self.current_path,
                std::path::Path::new(&uri)
            );
            return;
        }

        let active_tab = &mut self.tabs[self.active_tab_index];
        let active_files = &mut active_tab.files;
        active_files.clear();

        for (idx, item) in contexts.iter().enumerate() {
            let icon = if item.is_dir {
                item.custom_icon
                    .as_deref()
                    .and_then(|n| gio::Icon::for_string(n).ok())
                    .unwrap_or_else(|| {
                        crate::utils::get_icon_for_path(&item.target_path, item.is_dir)
                    })
            } else {
                crate::utils::get_icon_for_path(&item.target_path, item.is_dir)
            };

            net_debug!(
                "[network]   item#{idx} name={:?} is_dir={}",
                item.display_name,
                item.is_dir
            );

            let size = item.size();
            let mtime = item.mtime();
            let is_foreign_owner = item.is_foreign_owner(unsafe { libc::geteuid() });
            let is_empty = if item.is_dir && self.config.ui.show_empty_dir_emblem {
                item.is_empty()
            } else {
                false
            };
            let grid_idx = active_files.len();

            active_files.append(
                FileItem::builder(item.display_name.clone(), item.target_path.clone(), icon)
                    .is_dir(item.is_dir)
                    .icon_size(if self.is_list_mode {
                        self.current_list_icon_size
                    } else {
                        self.current_icon_size
                    })
                    .size(size)
                    .mtime(mtime)
                    .is_foreign_owner(is_foreign_owner)
                    .is_empty(is_empty)
                    .expand_labels(item.expand_labels)
                    .is_list_mode(self.is_list_mode)
                    .is_custom_icon(item.custom_icon.is_some())
                    .grid_idx(grid_idx)
                    .max_width_chars(self.config.ui.max_width_chars)
                    .grid_spacing(self.config.ui.grid_spacing)
                    .is_symlink(item.is_symlink)
                    .is_broken_symlink(item.is_broken_symlink)
                    .show_symlink_emblem(self.config.ui.show_symlink_emblem)
                    .build(),
            );
        }

        net_debug!(
            "[network]   appended all, active tab files.len()={}",
            self.tabs[self.active_tab_index].files.len()
        );
        self.update_breadcrumbs();
        net_debug!("[network] handle_network_loaded EXIT");
    }

    pub fn handle_connect_to_server(
        &mut self,
        uri: String,
        credentials: Option<crate::services::network::NetworkCredentials>,
        sender: &AsyncComponentSender<Self>,
    ) {
        self.history.push(self.current_path.clone());
        self.forward_stack.clear();
        self.load_network(&uri, credentials, sender.clone());
    }

    pub fn handle_unmount_network(&self, uri: String, sender: &AsyncComponentSender<Self>) {
        let sender_clone = sender.clone();
        relm4::spawn_local(async move {
            if let Err(e) = crate::services::network::unmount_network_location(&uri).await {
                sender_clone.input(AppMsg::ShowToast(e.to_string()));
            } else {
                sender_clone.input(AppMsg::RefreshNetworkSidebar);
            }
        });
    }

    pub fn handle_add_network_bookmark(
        &mut self,
        name: String,
        uri: String,
        sender: &AsyncComponentSender<Self>,
    ) {
        let bookmark = crate::services::network::NetworkBookmark::new(name, uri);
        if !self
            .config
            .network_bookmarks
            .iter()
            .any(|b| b.uri == bookmark.uri)
        {
            self.config.network_bookmarks.push(bookmark);
            crate::utils::save_config(&self.config);
            sender.input(AppMsg::RefreshSidebar);
        }
    }

    pub fn handle_remove_network_bookmark(
        &mut self,
        uri: String,
        sender: &AsyncComponentSender<Self>,
    ) {
        self.config.network_bookmarks.retain(|b| b.uri != uri);
        crate::utils::save_config(&self.config);
        sender.input(AppMsg::RefreshSidebar);
    }

    pub fn handle_refresh_network_sidebar(&mut self, sender: &AsyncComponentSender<Self>) {
        sender.input(AppMsg::RefreshSidebar);

        while let Some(child) = self.network_section.first_child() {
            self.network_section.remove(&child);
        }

        let fresh_section = crate::ui::sidebar_network::build_network_section(
            &self.config.network_bookmarks,
            sender.input_sender().clone(),
        );

        while let Some(child) = fresh_section.first_child() {
            fresh_section.remove(&child);
            self.network_section.append(&child);
        }
    }

    pub fn handle_navigate_network(&mut self, sender: &AsyncComponentSender<Self>) {
        self.history.push(self.current_path.clone());
        self.forward_stack.clear();
        self.load_network(
            crate::services::network::NETWORK_ROOT_URI,
            None,
            sender.clone(),
        );
    }
}
