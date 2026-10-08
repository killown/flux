use adw::gio;
use adw::glib;
use adw::prelude::*;
use std::cell::OnceCell;

thread_local! {
    static CONFIG_MONITOR: OnceCell<gio::FileMonitor> = const { OnceCell::new() };
    static TEMPLATE_MONITOR: OnceCell<gio::FileMonitor> = const { OnceCell::new() };
}

/// Sets up a GIO directory monitor to watch for config or style changes and
/// refreshes UI components.
///
/// Idempotent: subsequent calls are no-ops. The monitor is stored in the
/// thread-local [`CONFIG_MONITOR`] and remains active for the entire process
/// lifetime, which is the correct scope for a config-directory watcher.
/// Re-entrant calls (e.g. from a UI restart) will not create a second monitor
/// or cause a memory leak.
pub(super) fn setup_config_watcher() {
    let config_dir = dirs::config_dir().unwrap_or_default().join("flux");
    let file = gio::File::for_path(&config_dir);

    if let Ok(monitor) = file.monitor_directory(
        gio::FileMonitorFlags::WATCH_MOVES | gio::FileMonitorFlags::WATCH_MOUNTS,
        gio::Cancellable::NONE,
    ) {
        monitor.connect_changed(|_, file, other_file, event_type| {
            use gio::FileMonitorEvent::*;

            let file_name = file.basename().map(|n| n.to_string_lossy().to_string());
            let other_name = other_file
                .and_then(|f| f.basename())
                .map(|n| n.to_string_lossy().to_string());

            let matched = matches!(
                (file_name.as_deref(), other_name.as_deref()),
                (Some("config.toml" | "style.css"), _) | (_, Some("config.toml" | "style.css"))
            );

            if matched {
                match event_type {
                    Changed | ChangesDoneHint | Created | MovedIn | Renamed | Moved => {
                        glib::timeout_add_local_once(std::time::Duration::from_millis(50), || {
                            crate::services::loader::invalidate_extension_icon_cache();
                            crate::utils::helpers::load_custom_css();
                            crate::utils::helpers::load_custom_background_images();
                            crate::utils::config::invalidate_config_cache();
                            if let Some(app) = gio::Application::default() {
                                app.activate_action("reload-sidebar", None);
                            }
                            if let Some(sender) = crate::model::SENDER.get() {
                                let _ = sender.send(crate::model::AppMsg::Refresh);
                            }
                        });
                    }
                    _ => {}
                }
            }
        });
        CONFIG_MONITOR.with(|cell| {
            let _ = cell.set(monitor);
        });
    }

    // Monitor ~/.local/share/flux/icons/ for template.svg changes.
    if let Some(template_dir) = dirs::data_dir().map(|d| d.join("flux/icons")) {
        let _ = std::fs::create_dir_all(&template_dir);
        let t_file = gio::File::for_path(&template_dir);

        if let Ok(t_monitor) =
            t_file.monitor_directory(gio::FileMonitorFlags::WATCH_MOVES, gio::Cancellable::NONE)
        {
            t_monitor.connect_changed(|_, file, _, event_type| {
                if file
                    .basename()
                    .map(|n| n.to_string_lossy() == "template.svg")
                    .unwrap_or(false)
                {
                    match event_type {
                        gio::FileMonitorEvent::Changed
                        | gio::FileMonitorEvent::ChangesDoneHint
                        | gio::FileMonitorEvent::Created => {
                            glib::timeout_add_local_once(
                                std::time::Duration::from_millis(50),
                                || {
                                    crate::services::loader::invalidate_extension_icon_cache();
                                    if let Some(sender) = crate::model::SENDER.get() {
                                        let _ = sender.send(crate::model::AppMsg::Refresh);
                                    }
                                },
                            );
                        }
                        _ => {}
                    }
                }
            });
            TEMPLATE_MONITOR.with(|cell| {
                let _ = cell.set(t_monitor);
            });
        }
    }
}
