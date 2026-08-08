use crate::model::{AppMsg, FluxApp};
use crate::ui::FileItem;
use adw::gio;
use adw::prelude::*;
use relm4::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::atomic::Ordering;

impl FluxApp {
    pub fn handle_network_loaded(
        &mut self,
        _uri: String,
        load_id: u64,
        contexts: Vec<crate::model::FileLoadContext>,
    ) {
        if load_id != self.load_id.load(Ordering::SeqCst) {
            return;
        }

        self.files.clear();
        for item in contexts {
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

            self.files.append(FileItem {
                name: item.display_name.clone(),
                icon,
                thumbnail: None,
                is_dir: item.is_dir,
                path: item.target_path,
                icon_size: self.current_icon_size,
                size: item.size,
                is_editing: false,
                is_foreign_owner: false,
                expand_labels: item.expand_labels,
                is_list_mode: self.is_list_mode,
                is_custom_icon: item.custom_icon.is_some(),
                active_path: Rc::new(RefCell::new(None)),
            });
        }
        self.update_breadcrumbs();
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
        tokio::task::spawn_blocking(move || {
            if let Err(e) = crate::services::network::unmount_network_location(&uri) {
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
