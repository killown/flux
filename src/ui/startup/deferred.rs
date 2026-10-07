use crate::model::{AppMsg, FluxApp};
use crate::services::db::StateManager;
use crate::ui::constants;
use gtk::glib;
use relm4::prelude::*;
use std::sync::Arc;

impl FluxApp {
    /// Refreshes the status bar from the task queue on a throttled timer.
    pub(super) fn start_task_queue_tick(sender: &AsyncComponentSender<Self>) {
        // Throttled task queue UI refresh, deferred by 150ms, updates status bar only.
        let tick_sender = sender.clone();
        glib::timeout_add_local(std::time::Duration::from_millis(150), move || {
            let sender_inner = tick_sender.clone();
            gtk::glib::timeout_add_local(std::time::Duration::from_millis(100), move || {
                sender_inner.input(AppMsg::TaskQueueTick);
                glib::ControlFlow::Continue
            });
            glib::ControlFlow::Break
        });
    }

    /// Defers DB scrub, sidebar refresh and the first directory load past the first frame.
    pub(super) fn schedule_deferred_startup(
        state_db: &Arc<StateManager>,
        initial_tag_search: Option<String>,
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
        glib::timeout_add_local_once(std::time::Duration::from_millis(75), move || {
            crate::hit!("init_components:deferred");
            std::thread::spawn(move || {
                if let Err(e) = scrub_db.scrub_orphans() {
                    eprintln!("[DB] Scrub failed: {}", e);
                }
            });

            // Warm the Nerd Font OnceLock off the main thread so the first
            // `bind_git_badge` doesn't stall on fontconfig enumeration.
            std::thread::spawn(|| {
                let _ = crate::services::git::is_nerd_font_available();
            });

            crate::services::archive::purge_stale_scratch_dirs();

            s_init.input(AppMsg::RefreshSidebar);

            if let Some(tag) = tag_to_apply {
                s_init.input(AppMsg::SwitchHeader(constants::VIEW_SEARCH.to_string()));
                s_init.input(AppMsg::UpdateFilter(tag));
            } else {
                s_init.input(AppMsg::Refresh);
            }
        });
    }
}
