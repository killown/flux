use crate::app::state::FluxApp;
use crate::domain::model::{AppMsg, AppSidebarMsg, NavigationMsg, SearchMsg, TabsMsg, TasksMsg};
use crate::infra::db::StateManager;
use crate::ui::constants;
use gtk::glib;
use relm4::prelude::*;
use std::path::PathBuf;
use std::sync::Arc;

/// The saved tabs that still exist as folders, and the index of the active one.
/// `None` when nothing usable was saved.
fn saved_tabs_to_restore(db: &StateManager) -> Option<(Vec<PathBuf>, usize)> {
    let (paths, active) = db.load_open_tabs();
    let kept: Vec<(usize, PathBuf)> = paths
        .into_iter()
        .enumerate()
        .filter(|(_, p)| p.is_dir())
        .collect();
    if kept.is_empty() {
        return None;
    }
    let active = kept.iter().position(|(i, _)| *i == active).unwrap_or(0);
    Some((kept.into_iter().map(|(_, p)| p).collect(), active))
}

impl FluxApp {
    /// Refreshes the status bar from the task queue on a throttled timer.
    pub(super) fn start_task_queue_tick(sender: &AsyncComponentSender<Self>) {
        crate::hit!("start_task_queue_tick");
        // Throttled task queue UI refresh, deferred by 150ms, updates status bar only.
        let tick_sender = sender.clone();
        glib::timeout_add_local(std::time::Duration::from_millis(150), move || {
            let sender_inner = tick_sender.clone();
            gtk::glib::timeout_add_local(std::time::Duration::from_millis(100), move || {
                sender_inner.input(AppMsg::Tasks(TasksMsg::TaskQueueTick));
                glib::ControlFlow::Continue
            });
            glib::ControlFlow::Break
        });
    }

    /// Defers DB scrub, sidebar refresh and the first directory load past the first frame.
    pub(super) fn schedule_deferred_startup(
        state_db: &Arc<StateManager>,
        initial_tag_search: Option<String>,
        restore_tabs: bool,
        sender: &AsyncComponentSender<Self>,
    ) {
        // WARNING: CRITICAL PERFORMANCE GUARD:
        // Defer non-essential I/O (DB orphan scrub, sidebar mount discovery, and directory reads)
        // by 75ms to allow GTK4/GSK to present the initial frame to the compositor immediately.
        // Removing or shortening this timeout adds ~30-55ms of synchronous stall to cold/warm startup.
        // And hey you from the future, dont remove this timeout, it is critical for performance.
        let s_init = sender.clone();
        let scrub_db = state_db.clone();
        let tag_to_apply = initial_tag_search;
        let restore_db = state_db.clone();
        glib::timeout_add_local_once(std::time::Duration::from_millis(75), move || {
            crate::hit!("init_components:deferred");
            std::thread::spawn(move || {
                if let Err(e) = scrub_db.scrub_orphans() {
                    eprintln!("[DB] Scrub failed: {}", e);
                }
            });

            crate::infra::archive::purge_stale_scratch_dirs();

            s_init.input(AppMsg::Sidebar(AppSidebarMsg::RefreshSidebar));

            let restore = (restore_tabs && tag_to_apply.is_none())
                .then(|| saved_tabs_to_restore(&restore_db))
                .flatten();

            if let Some(tag) = tag_to_apply {
                s_init.input(AppMsg::Search(SearchMsg::SwitchHeader(
                    constants::VIEW_SEARCH.to_string(),
                )));
                s_init.input(AppMsg::Search(SearchMsg::UpdateFilter(tag)));
                s_init.input(AppMsg::Tabs(TabsMsg::RestoreSession {
                    paths: Vec::new(),
                    active: 0,
                }));
            } else if let Some((paths, active)) = restore {
                s_init.input(AppMsg::Tabs(TabsMsg::RestoreSession { paths, active }));
            } else {
                s_init.input(AppMsg::Navigation(NavigationMsg::Refresh));
                s_init.input(AppMsg::Tabs(TabsMsg::RestoreSession {
                    paths: Vec::new(),
                    active: 0,
                }));
            }

            glib::timeout_add_seconds_local(5, || {
                crate::infra::archive::thumb_cache::flush_all();
                glib::ControlFlow::Continue
            });
        });
    }
}
