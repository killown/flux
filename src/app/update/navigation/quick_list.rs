use crate::domain::model::quick_groups::{NameError, QuickGroups};
use crate::domain::model::quick_list;
use crate::domain::model::{
    AppMsg, FluxApp, NavigationMsg, QuickListMsg, RightPanelType, WindowMsg,
};
use crate::i18n::tr;
use gtk::prelude::*;
use relm4::prelude::*;
use std::path::PathBuf;

impl FluxApp {
    pub fn handle_perform_quick_transfer(
        &mut self,
        dest: PathBuf,
        is_cut: bool,
        sender: &AsyncComponentSender<Self>,
    ) {
        crate::hit!("handle_perform_quick_transfer");
        let sources = self.resolve_command_targets();
        if sources.is_empty() || !dest.is_dir() {
            return;
        }

        if is_cut {
            self.handle_drop_items(sources, dest, sender);
        } else {
            let sender_clone = sender.clone();
            relm4::spawn_blocking(move || {
                let mut count = 0;
                for src_path in sources {
                    if let Some(name) = src_path.file_name() {
                        let dst_path = dest.join(name);
                        if src_path == dst_path {
                            continue;
                        }
                        let src_file = gtk::gio::File::for_path(&src_path);
                        let dst_file = gtk::gio::File::for_path(&dst_path);

                        if src_file
                            .copy(
                                &dst_file,
                                gtk::gio::FileCopyFlags::ALL_METADATA,
                                gtk::gio::Cancellable::NONE,
                                None,
                            )
                            .is_ok()
                        {
                            count += 1;
                        }
                    }
                }

                if count > 0 {
                    sender_clone.input(AppMsg::Window(WindowMsg::ShowToast(format!(
                        "Copied {} item(s) to {}",
                        count,
                        dest.file_name().unwrap_or_default().to_string_lossy()
                    ))));
                    sender_clone.input(AppMsg::Navigation(NavigationMsg::Refresh));
                }
            });
        }
    }

    /// Every list, with the live selected list (`exclusive_list`,
    /// `exclusive_index`) laid over its stored copy.
    pub fn quick_snapshot(&self) -> QuickGroups {
        self.quick_groups
            .with_selected(&self.exclusive_list, self.exclusive_index)
    }

    /// Takes `groups` as the new state and loads its selected list into the
    /// working fields, which is what Tab and the other chords walk.
    fn adopt_quick_groups(&mut self, groups: QuickGroups) {
        debug_assert!(groups.invariant_holds());
        let selected = groups.selected_list();
        self.exclusive_list = selected.items.clone();
        self.exclusive_index = selected.index;
        self.quick_groups = groups;
    }

    /// Runs `edit` on a copy of every list. When it returns `true` the copy
    /// becomes the new state, is saved and the panel is redrawn.
    fn edit_quick_groups(
        &mut self,
        sender: &AsyncComponentSender<Self>,
        edit: impl FnOnce(&mut QuickGroups) -> bool,
    ) -> bool {
        crate::hit!("edit_quick_groups");
        let mut groups = self.quick_snapshot();
        if !edit(&mut groups) {
            return false;
        }
        self.adopt_quick_groups(groups);
        self.persist_quick_list();
        sender.input(AppMsg::QuickList(QuickListMsg::RebuildQuickPanel));
        true
    }

    /// Saves every list, the selection and each list's slot. Write-through,
    /// so what is on screen at exit is what is on disk at the next launch.
    pub(crate) fn persist_quick_list(&self) {
        debug_assert!(quick_list::invariant_holds(
            &self.exclusive_list,
            self.exclusive_index
        ));
        if let Err(e) = self.state_db.save_quick_groups(&self.quick_snapshot()) {
            eprintln!("[flux] Failed to save quick list: {e}");
        }
    }

    /// Adds a path or multiple selected paths to the FRONT of the quick list.
    ///
    /// Invariant Q (`index.is_none() <=> list.is_empty()`, else `index < len`)
    /// holds: `add_front` keeps the index on the same folder, then it follows
    /// the folder shown in the grid when that folder is in the list.
    pub fn handle_add_exclusive(
        &mut self,
        explicit_path: Option<PathBuf>,
        sender: &AsyncComponentSender<Self>,
    ) {
        crate::hit!("handle_add_exclusive");
        let mut paths_to_add = Vec::new();

        if let Some(path) = explicit_path {
            paths_to_add.push(path);
        } else {
            let selection = self.get_selection();
            if selection.is_empty() {
                paths_to_add.push(self.current_path.clone());
            } else {
                paths_to_add.extend(selection);
            }
        }

        let canon = paths_to_add
            .into_iter()
            .map(|p| p.canonicalize().unwrap_or(p));
        let added =
            quick_list::add_front(&mut self.exclusive_list, &mut self.exclusive_index, canon);

        if added > 0 {
            quick_list::sync_to(
                &self.exclusive_list,
                &mut self.exclusive_index,
                &self.current_path,
            );
            self.persist_quick_list();
            sender.input(AppMsg::QuickList(QuickListMsg::RebuildQuickPanel));
        }
    }

    /// Adds the folder shown in the grid to the quick panel list, whatever is
    /// selected. Unlike [`Self::handle_add_exclusive`] with `None`, which prefers
    /// the selection.
    pub fn handle_add_current_folder_exclusive(&mut self, sender: &AsyncComponentSender<Self>) {
        crate::hit!("handle_add_current_folder_exclusive");
        self.handle_add_exclusive(Some(self.current_path.clone()), sender);
    }

    /// Clears all entries from the selected quick list. The list itself stays.
    ///
    /// Silent: no toast, no dialog, and no `Navigate` is sent, so the folder
    /// being shown cannot change. Clearing an empty list changes nothing.
    /// Invariant Q holds: both fields are reset together.
    pub fn handle_clear_exclusive(&mut self, sender: &AsyncComponentSender<Self>) {
        crate::hit!("handle_clear_exclusive");
        if quick_list::clear(&mut self.exclusive_list, &mut self.exclusive_index) {
            self.persist_quick_list();
            sender.input(AppMsg::QuickList(QuickListMsg::RebuildQuickPanel));
        }
    }

    /// Redraws the Quick List panel. Does nothing visible until it was opened once.
    pub fn handle_rebuild_quick_panel(&self, sender: &AsyncComponentSender<Self>) {
        crate::hit!("handle_rebuild_quick_panel");
        crate::ui::panels::quick_list::refresh(self.quick_snapshot(), sender);
    }

    /// Persists the new slot, then navigates to `target`.
    fn quick_navigate_to(&mut self, target: PathBuf, sender: &AsyncComponentSender<Self>) {
        self.persist_quick_list();
        sender.input(AppMsg::Navigation(NavigationMsg::Navigate(target)));
        sender.input(AppMsg::QuickList(QuickListMsg::RebuildQuickPanel));
    }

    /// Switches to the next item in the quick panel list (Ctrl+PageDown).
    /// Invariant Q holds via `quick_list::step`.
    pub fn handle_next_exclusive(&mut self, sender: &AsyncComponentSender<Self>) {
        crate::hit!("handle_next_exclusive");
        if let Some(target) = quick_list::step(
            &self.exclusive_list,
            &mut self.exclusive_index,
            quick_list::Step::Next,
        ) {
            self.quick_navigate_to(target, sender);
        }
    }

    /// Switches to the previous item in the quick panel list (Ctrl+PageUp).
    /// Invariant Q holds via `quick_list::step`.
    pub fn handle_prev_exclusive(&mut self, sender: &AsyncComponentSender<Self>) {
        crate::hit!("handle_prev_exclusive");
        if let Some(target) = quick_list::step(
            &self.exclusive_list,
            &mut self.exclusive_index,
            quick_list::Step::Prev,
        ) {
            self.quick_navigate_to(target, sender);
        }
    }

    /// Tab: go to the current slot, or advance when already in its folder.
    /// Invariant Q holds via `quick_list::tab`.
    pub fn handle_quick_list_tab(&mut self, sender: &AsyncComponentSender<Self>) {
        crate::hit!("handle_quick_list_tab");
        if let Some(target) = quick_list::tab(
            &self.exclusive_list,
            &mut self.exclusive_index,
            &self.current_path,
        ) {
            // An empty list never reaches here, so Tab on an empty list stays silent.
            self.reveal_quick_list_panel(sender);
            self.quick_navigate_to(target, sender);
        }
    }

    /// Ctrl+1..9: jump to slot `n` (1-based). Invariant Q holds via
    /// `quick_list::jump`.
    pub fn handle_quick_list_jump(&mut self, n: usize, sender: &AsyncComponentSender<Self>) {
        crate::hit!("handle_quick_list_jump");
        if let Some(target) = quick_list::jump(&self.exclusive_list, &mut self.exclusive_index, n) {
            // Ctrl+1..9 reveals the panel like Tab.
            self.reveal_quick_list_panel(sender);
            self.quick_navigate_to(target, sender);
        }
    }

    /// Tab and Ctrl+1..9 show the panel. Any other right-side panel that is
    /// open gives way to it (`toggle_sidebar_right_panel` hides the others).
    fn reveal_quick_list_panel(&mut self, sender: &AsyncComponentSender<Self>) {
        if !self.quick_list_panel_visible {
            self.toggle_sidebar_right_panel(RightPanelType::QuickList, sender);
        }
    }

    /// Ctrl+Shift+Q and the panel's close button. Opening puts the cursor in
    /// the filter entry.
    pub fn handle_toggle_quick_list_panel(&mut self, sender: &AsyncComponentSender<Self>) {
        crate::hit!("handle_toggle_quick_list_panel");
        self.toggle_sidebar_right_panel(RightPanelType::QuickList, sender);
        if self.quick_list_panel_visible {
            // The entry is not mapped yet the first time the panel is built.
            gtk::glib::idle_add_local_once(crate::ui::panels::quick_list::focus_search);
        }
    }

    /// Makes list `list` the one Tab and Ctrl+PageUp/PageDown walk.
    pub fn handle_quick_list_select(&mut self, list: usize, sender: &AsyncComponentSender<Self>) {
        crate::hit!("handle_quick_list_select");
        self.edit_quick_groups(sender, |g| g.select(list));
    }

    /// Opens a folder of list `list`: selects the list, puts its slot on the
    /// folder, navigates there and hands the keyboard back to the grid.
    pub fn handle_quick_list_open(
        &mut self,
        list: usize,
        path: PathBuf,
        sender: &AsyncComponentSender<Self>,
    ) {
        crate::hit!("handle_quick_list_open");
        if self.edit_quick_groups(sender, |g| g.open_item(list, &path)) {
            sender.input(AppMsg::Navigation(NavigationMsg::Navigate(path)));
            if let Some(tab) = self.tabs.get(self.active_tab_index) {
                tab.files.view.grab_focus();
            }
        }
    }

    fn quick_name_error(e: NameError) -> String {
        match e {
            NameError::Empty => tr("A list needs a name"),
            NameError::Taken => tr("A list with that name already exists"),
        }
    }

    /// Creates an empty list and selects it, so Insert fills it right away.
    pub fn handle_quick_list_new(&mut self, name: String, sender: &AsyncComponentSender<Self>) {
        crate::hit!("handle_quick_list_new");
        let mut refused = None;
        self.edit_quick_groups(sender, |g| match g.add_list(&name) {
            Ok(i) => {
                g.select(i);
                true
            }
            Err(e) => {
                refused = Some(e);
                false
            }
        });
        if let Some(e) = refused {
            sender.input(AppMsg::Window(WindowMsg::ShowToast(
                Self::quick_name_error(e),
            )));
        }
    }

    pub fn handle_quick_list_rename(
        &mut self,
        list: usize,
        name: String,
        sender: &AsyncComponentSender<Self>,
    ) {
        crate::hit!("handle_quick_list_rename");
        let mut refused = None;
        self.edit_quick_groups(sender, |g| match g.rename(list, &name) {
            Ok(changed) => changed,
            Err(e) => {
                refused = Some(e);
                false
            }
        });
        if let Some(e) = refused {
            sender.input(AppMsg::Window(WindowMsg::ShowToast(
                Self::quick_name_error(e),
            )));
        }
    }

    /// Deletes a list. The folders themselves are untouched.
    pub fn handle_quick_list_delete(&mut self, list: usize, sender: &AsyncComponentSender<Self>) {
        crate::hit!("handle_quick_list_delete");
        self.edit_quick_groups(sender, |g| g.delete(list));
    }

    /// Empties a list but keeps it.
    pub fn handle_quick_list_clear_list(
        &mut self,
        list: usize,
        sender: &AsyncComponentSender<Self>,
    ) {
        crate::hit!("handle_quick_list_clear_list");
        self.edit_quick_groups(sender, |g| g.clear_list(list));
    }

    /// Adds the folder shown in the grid to the front of list `list`.
    pub fn handle_quick_list_add_current(
        &mut self,
        list: usize,
        sender: &AsyncComponentSender<Self>,
    ) {
        crate::hit!("handle_quick_list_add_current");
        let current = self.current_path.clone();
        let canon = current.canonicalize().unwrap_or_else(|_| current.clone());
        self.edit_quick_groups(sender, |g| {
            let added = g.add_to(list, std::iter::once(canon));
            if let Some(l) = g.lists.get_mut(list) {
                quick_list::sync_to(&l.items, &mut l.index, &current);
            }
            added > 0
        });
    }

    pub fn handle_quick_list_remove(
        &mut self,
        list: usize,
        path: PathBuf,
        sender: &AsyncComponentSender<Self>,
    ) {
        crate::hit!("handle_quick_list_remove");
        self.edit_quick_groups(sender, |g| g.remove_from(list, &path));
    }

    pub fn handle_quick_list_move(
        &mut self,
        path: PathBuf,
        from: usize,
        to: usize,
        sender: &AsyncComponentSender<Self>,
    ) {
        crate::hit!("handle_quick_list_move");
        self.edit_quick_groups(sender, |g| g.move_item(&path, from, to));
    }

    /// Points slot `slot` of list `list` at the folder shown in the grid.
    pub fn handle_quick_list_set_slot(
        &mut self,
        list: usize,
        slot: usize,
        sender: &AsyncComponentSender<Self>,
    ) {
        crate::hit!("handle_quick_list_set_slot");
        let current = self.current_path.clone();
        if self.edit_quick_groups(sender, |g| g.replace_slot(list, slot, current)) {
            sender.input(AppMsg::Window(WindowMsg::ShowToast(tr(
                "Quick list slot updated",
            ))));
        } else {
            sender.input(AppMsg::Window(WindowMsg::ShowToast(tr(
                "That folder is already in this list",
            ))));
        }
    }
}
