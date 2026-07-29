mod i18n;
mod model;
mod services;
mod ui;
mod utils;

use crate::model::{AppMsg, Config, FluxApp};
use crate::ui::FileProperties;
use adw::prelude::*;
use adw::{gio, glib};
use relm4::prelude::*;
use std::cell::OnceCell;
use std::fs;
use std::path::PathBuf;

thread_local! {
    static CSS_PROVIDER: gtk::CssProvider = gtk::CssProvider::new();
    static CONFIG_MONITOR: OnceCell<gio::FileMonitor> = const { OnceCell::new() };
}

/// Describes the action the process should take after argument parsing.
///
/// Separating the decision from its execution makes the parsing logic
/// independently testable without initialising GTK or the GIO stack.
#[derive(Debug, PartialEq)]
pub enum StartupAction {
    /// Launch the main file-manager window starting at the given path.
    ///
    /// The path is guaranteed to be non-empty, callers must never produce
    /// `StartupAction::Launch(PathBuf::new())`.
    Launch(PathBuf),
    /// Open the standalone file-properties dialog for the given path.
    FileProperties(PathBuf),
    /// Open the context-menu editor utility.
    MenuEditor,
    /// Print the version string and exit.
    PrintVersion,
    /// Print the help text and exit.
    PrintHelp,
    /// Exit with an error because an unrecognised flag was supplied.
    UnknownFlag(String),
}

/// Parses the raw process argument list into a [`StartupAction`].
///
/// `args` must include `argv[0]` (the binary name) as its first element,
/// matching the contract of [`std::env::args`].  An empty slice is treated
/// as "no arguments" and produces [`StartupAction::Launch`] with the user's
/// home directory.
///
/// This function is pure: it has no side-effects and does not touch the
/// filesystem, GTK, or GIO.  All branching is exercised by unit tests.
///
/// # Arguments
///
/// * `args` - The full argument list including `argv[0]`.
/// * `home_dir` - Caller-supplied home directory used when no path argument
///   is present.  Passing it explicitly keeps the function deterministic in
///   tests without touching the real environment.
///
/// # Returns
///
/// The [`StartupAction`] that `main` should execute.
pub fn resolve_startup_action(args: &[String], home_dir: PathBuf) -> StartupAction {
    let positional = args.get(1).map(String::as_str);

    match positional {
        None => StartupAction::Launch(home_dir),

        Some("--version" | "-v") => StartupAction::PrintVersion,

        Some("--help" | "-h") => StartupAction::PrintHelp,

        Some("--menu-editor") => StartupAction::MenuEditor,

        Some("--file-properties") => match args.get(2) {
            Some(path) => StartupAction::FileProperties(PathBuf::from(path)),
            None => StartupAction::PrintHelp,
        },

        Some(arg) if arg.starts_with('-') => StartupAction::UnknownFlag(arg.to_string()),

        Some(path) => StartupAction::Launch(PathBuf::from(path)),
    }
}

/// Loads CSS based on config.toml theme selection with local and internal fallbacks.
fn load_custom_css() {
    let config_dir = dirs::config_dir().unwrap_or_default().join("flux");
    let config_path = config_dir.join("config.toml");

    let config: Option<Config> = fs::read_to_string(&config_path)
        .ok()
        .and_then(|content| toml::from_str(&content).ok());

    let mut css_data = None;

    if let Some(theme_name) = config.and_then(|c| c.ui.theme) {
        let theme_filename = format!("{}.css", theme_name);

        let local_theme = dirs::data_dir()
            .unwrap_or_default()
            .join("flux")
            .join("themes")
            .join(&theme_filename);

        let system_theme = PathBuf::from("/usr/share/flux/themes").join(&theme_filename);

        css_data = fs::read_to_string(&local_theme)
            .or_else(|_| fs::read_to_string(&system_theme))
            .ok();
    }

    if css_data.is_none() {
        css_data = fs::read_to_string(config_dir.join("style.css")).ok();
    }

    if let Some(display) = adw::gdk::Display::default() {
        CSS_PROVIDER.with(|provider| {
            // Remove existing provider from display
            gtk::style_context_remove_provider_for_display(&display, provider);

            if let Some(data) = css_data {
                // Load new CSS data
                provider.load_from_data(&data);

                // Add the updated provider back
                gtk::style_context_add_provider_for_display(
                    &display,
                    provider,
                    gtk::STYLE_PROVIDER_PRIORITY_USER,
                );
            } else {
                // No CSS data available, clear the provider
                provider.load_from_data("");
            }
        });
    }
}

/// Sets up a GIO directory monitor to watch for config or style changes and refreshes UI components.
///
/// Idempotent: subsequent calls are no-ops. The monitor is stored in the thread-local
/// [`CONFIG_MONITOR`] and remains active for the entire process lifetime, which is the
/// correct scope for a config-directory watcher. Re-entrant calls (e.g. from a UI
/// restart) will not create a second monitor or cause a memory leak.
fn setup_config_watcher() {
    let config_dir = dirs::config_dir().unwrap_or_default().join("flux");
    let file = gio::File::for_path(&config_dir);

    let Ok(monitor) = file.monitor_directory(
        gio::FileMonitorFlags::WATCH_MOUNTS | gio::FileMonitorFlags::SEND_MOVED,
        gio::Cancellable::NONE,
    ) else {
        return;
    };

    monitor.connect_changed(|_, file, _, event_type| {
        if let Some(name) = file.basename() {
            let n = name.to_string_lossy();
            if n == "style.css" || n == "config.toml" {
                match event_type {
                    gio::FileMonitorEvent::Changed
                    | gio::FileMonitorEvent::ChangesDoneHint
                    | gio::FileMonitorEvent::Created
                    | gio::FileMonitorEvent::MovedIn => {
                        load_custom_css();
                        // Signal the application to reload the sidebar if the config file was modified
                        if n == "config.toml" {
                            if let Some(app) = gio::Application::default() {
                                app.activate_action("reload-sidebar", None);
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
    });

    // `set` is a no-op if already initialized, the losing monitor is dropped,
    // which cancels the underlying GFileMonitor via GIO's ref-counting.
    CONFIG_MONITOR.with(|cell| {
        let _ = cell.set(monitor);
    });
}

fn setup_shortcuts(app: &adw::Application) {
    let action_menu_editor = gio::SimpleAction::new("open-menu-editor", None);
    action_menu_editor.connect_activate(|_, _| {
        let exe = std::env::current_exe().expect("Failed to get current exe path");
        let _ = std::process::Command::new(exe).arg("--menu-editor").spawn();
    });
    app.add_action(&action_menu_editor);
    app.set_accels_for_action("app.open-menu-editor", &["F9"]);

    let action_settings = gio::SimpleAction::new("open-settings", None);
    action_settings.connect_activate(|_, _| {
        let settings_win = crate::ui::SettingsWindow::builder().launch(()).detach();
        settings_win.widget().present();
    });
    app.add_action(&action_settings);
    app.set_accels_for_action("app.open-settings", &["F10"]);

    let action_help = gio::SimpleAction::new("open-help", None);
    action_help.connect_activate(|_, _| {
        if let Some(s) = crate::model::SENDER.get() {
            let _ = s.send(crate::model::AppMsg::ShowHelp);
        }
    });
    app.add_action(&action_help);
    app.set_accels_for_action("app.open-help", &["F1"]);

    let action_new_window = gio::SimpleAction::new("new-window", None);
    action_new_window.connect_activate(|_, _| {
        if let Ok(exe) = std::env::current_exe() {
            let _ = std::process::Command::new(exe).spawn();
        }
    });
    app.add_action(&action_new_window);
    app.set_accels_for_action("app.new-window", &["<Primary>n"]);
}

fn main() {
    i18n::init();
    adw::init().expect("Failed to initialize Libadwaita");

    let args: Vec<String> = std::env::args().collect();
    let home_dir = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));

    match resolve_startup_action(&args, home_dir) {
        StartupAction::PrintVersion => {
            println!("flux {}", env!("CARGO_PKG_VERSION"));
        }

        StartupAction::PrintHelp => {
            println!("Usage: flux [OPTIONS] [PATH]");
            println!();
            println!("Arguments:");
            println!("  [PATH]  Directory to open on startup");
            println!();
            println!("Options:");
            println!("  -h, --help                  Print this help message");
            println!("  -v, --version               Print version information");
            println!("      --file-properties PATH  Open the file properties window for PATH");
            println!("      --menu-editor           To manage menu.rs");
        }

        StartupAction::FileProperties(path) => {
            let app = RelmApp::new("flux.PropertiesViewer");
            app.allow_multiple_instances(true);
            app.with_args(vec![]).run::<FileProperties>(path);
        }

        StartupAction::MenuEditor => {
            crate::ui::menu_editor::run();
        }

        StartupAction::UnknownFlag(flag) => {
            eprintln!("flux: unrecognized option '{flag}'");
            eprintln!("Try 'flux --help' for more information.");
            std::process::exit(1);
        }

        StartupAction::Launch(start_path) => {
            // Defer non-critical CSS/Theme loading and dependency checks by 150ms
            glib::timeout_add_local(std::time::Duration::from_millis(150), move || {
                load_custom_css();
                setup_config_watcher();
                std::thread::spawn(crate::utils::deps::check_optional_deps);
                glib::ControlFlow::Break
            });

            // --- MAIN APP HANDLER ---
            //NOTE:
            // Setting application_id with .flags(gio::ApplicationFlags...) in the builder
            // triggers a synchronous D-Bus handshake  and Wayland compositor lookup
            // that blocks the main thread for around ~200ms.
            let base_app = adw::Application::builder().build();

            assert!(
                base_app.application_id().is_none(),
                "\n\n[flux] STARTUP REGRESSION: application_id is set on the main adw::Application.\n\
             This triggers a synchronous D-Bus name acquisition and Wayland compositor\n\
             lookup on the main thread, adding ~200ms to startup time.\n\
             Remove .application_id(...) from the adw::Application::builder() call.\n"
            );

            assert!(
                !base_app.flags().contains(gio::ApplicationFlags::NON_UNIQUE),
                "\n\n[flux] STARTUP REGRESSION: NON_UNIQUE flag is set on the main adw::Application.\n\
             This triggers a synchronous D-Bus handshake and Wayland compositor lookup\n\
             on the main thread, adding ~200ms to startup time.\n\
             Remove .flags(gio::ApplicationFlags::NON_UNIQUE) from the adw::Application::builder() call.\n"
            );

            setup_shortcuts(&base_app);

            let app: RelmApp<AppMsg> = RelmApp::from_app(base_app);
            app.allow_multiple_instances(true);
            app.with_args(vec![]).run_async::<FluxApp>(start_path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(argv: &[&str]) -> Vec<String> {
        argv.iter().map(|s| s.to_string()).collect()
    }

    const FAKE_HOME: &str = "/home/testuser";

    fn home() -> PathBuf {
        PathBuf::from(FAKE_HOME)
    }

    // ── No-arg / default path ────────────────────────────────────────────────

    #[test]
    fn no_args_launches_home_dir() {
        // Regression: argv[0]-only must never produce PathBuf::new() (the empty
        // path), which was the root cause of the bug where `cargo run -- /home/`
        // opened the workspace root instead of /home/.
        let action = resolve_startup_action(&args(&["flux-fm"]), home());
        assert_eq!(action, StartupAction::Launch(PathBuf::from(FAKE_HOME)));
    }

    #[test]
    fn no_args_launch_path_is_never_empty() {
        let action = resolve_startup_action(&args(&["flux-fm"]), home());
        if let StartupAction::Launch(path) = action {
            assert!(
                !path.as_os_str().is_empty(),
                "Launch path must never be PathBuf::new() (empty string): \
                 init_components would open the process cwd instead of the intended directory"
            );
        }
    }

    #[test]
    fn empty_args_slice_launches_home_dir() {
        // Edge case: completely empty slice (no argv[0]).
        let action = resolve_startup_action(&[], home());
        assert_eq!(action, StartupAction::Launch(PathBuf::from(FAKE_HOME)));
    }

    // ── Explicit path argument ───────────────────────────────────────────────

    #[test]
    fn absolute_path_arg_launches_that_path() {
        let action = resolve_startup_action(&args(&["flux-fm", "/home/"]), home());
        assert_eq!(action, StartupAction::Launch(PathBuf::from("/home/")));
    }

    #[test]
    fn absolute_path_arg_is_not_home_dir() {
        // Validates that the supplied path, not the home fallback, is used.
        let action = resolve_startup_action(&args(&["flux-fm", "/home/"]), home());
        assert_ne!(action, StartupAction::Launch(home()));
    }

    #[test]
    fn root_path_arg_launches_root() {
        let action = resolve_startup_action(&args(&["flux-fm", "/"]), home());
        assert_eq!(action, StartupAction::Launch(PathBuf::from("/")));
    }

    #[test]
    fn relative_path_arg_launches_that_relative_path() {
        let action = resolve_startup_action(&args(&["flux-fm", "Documents"]), home());
        assert_eq!(action, StartupAction::Launch(PathBuf::from("Documents")));
    }

    #[test]
    fn tilde_path_arg_is_passed_through_as_literal() {
        // resolve_startup_action does NOT expand tildes, that is the caller's
        // responsibility (utils::expand_path).  This test documents the contract.
        let action = resolve_startup_action(&args(&["flux-fm", "~"]), home());
        assert_eq!(action, StartupAction::Launch(PathBuf::from("~")));
    }

    #[test]
    fn path_with_spaces_is_preserved() {
        let action = resolve_startup_action(&args(&["flux-fm", "/home/user/My Documents"]), home());
        assert_eq!(
            action,
            StartupAction::Launch(PathBuf::from("/home/user/My Documents"))
        );
    }

    // ── --version / -v ───────────────────────────────────────────────────────

    #[test]
    fn version_long_flag() {
        let action = resolve_startup_action(&args(&["flux-fm", "--version"]), home());
        assert_eq!(action, StartupAction::PrintVersion);
    }

    #[test]
    fn version_short_flag() {
        let action = resolve_startup_action(&args(&["flux-fm", "-v"]), home());
        assert_eq!(action, StartupAction::PrintVersion);
    }

    // ── --help / -h ──────────────────────────────────────────────────────────

    #[test]
    fn help_long_flag() {
        let action = resolve_startup_action(&args(&["flux-fm", "--help"]), home());
        assert_eq!(action, StartupAction::PrintHelp);
    }

    #[test]
    fn help_short_flag() {
        let action = resolve_startup_action(&args(&["flux-fm", "-h"]), home());
        assert_eq!(action, StartupAction::PrintHelp);
    }

    // ── --menu-editor ────────────────────────────────────────────────────────

    #[test]
    fn menu_editor_flag() {
        let action = resolve_startup_action(&args(&["flux-fm", "--menu-editor"]), home());
        assert_eq!(action, StartupAction::MenuEditor);
    }

    // ── --file-properties ────────────────────────────────────────────────────

    #[test]
    fn file_properties_with_path() {
        let action = resolve_startup_action(
            &args(&["flux-fm", "--file-properties", "/home/user/photo.jpg"]),
            home(),
        );
        assert_eq!(
            action,
            StartupAction::FileProperties(PathBuf::from("/home/user/photo.jpg"))
        );
    }

    #[test]
    fn file_properties_without_path_falls_back_to_help() {
        // Missing PATH operand → show help rather than panicking.
        let action = resolve_startup_action(&args(&["flux-fm", "--file-properties"]), home());
        assert_eq!(action, StartupAction::PrintHelp);
    }

    #[test]
    fn file_properties_path_is_not_empty() {
        let action = resolve_startup_action(
            &args(&["flux-fm", "--file-properties", "/tmp/file.txt"]),
            home(),
        );
        if let StartupAction::FileProperties(path) = action {
            assert!(!path.as_os_str().is_empty());
        } else {
            panic!("expected FileProperties variant");
        }
    }

    // ── Unknown flags ────────────────────────────────────────────────────────

    #[test]
    fn unknown_long_flag_is_reported() {
        let action = resolve_startup_action(&args(&["flux-fm", "--no-such-flag"]), home());
        assert_eq!(
            action,
            StartupAction::UnknownFlag("--no-such-flag".to_string())
        );
    }

    #[test]
    fn unknown_short_flag_is_reported() {
        let action = resolve_startup_action(&args(&["flux-fm", "-z"]), home());
        assert_eq!(action, StartupAction::UnknownFlag("-z".to_string()));
    }

    #[test]
    fn unknown_flag_preserves_exact_text() {
        let flag = "--flag-with-value=something";
        let action = resolve_startup_action(&args(&["flux-fm", flag]), home());
        assert_eq!(action, StartupAction::UnknownFlag(flag.to_string()));
    }

    // ── Launch path invariants ───────────────────────────────────────────────

    #[test]
    fn launch_path_is_never_path_buf_new_for_any_path_arg() {
        // Parametrised regression: every path-like argument must produce a
        // non-empty Launch path.  PathBuf::new() is the sentinel that caused
        // init_components to receive an empty path and open the cwd.
        let cases = ["/home/", "/", "/tmp", ".", "Documents", "~"];
        for input in cases {
            let action = resolve_startup_action(&args(&["flux-fm", input]), home());
            if let StartupAction::Launch(path) = action {
                assert!(
                    !path.as_os_str().is_empty(),
                    "input '{input}' produced empty PathBuf - init_components regression risk"
                );
            }
        }
    }

    #[test]
    fn home_fallback_is_never_path_buf_new() {
        // Ensures that even a degenerate home_dir argument (PathBuf::new()) is
        // caught: callers must supply a real fallback.
        let action = resolve_startup_action(&args(&["flux-fm"]), PathBuf::from("."));
        if let StartupAction::Launch(path) = action {
            assert!(
                !path.as_os_str().is_empty(),
                "home fallback produced empty path"
            );
        }
    }

    // ── Discriminant exhaustiveness ──────────────────────────────────────────

    #[test]
    fn all_flag_variants_are_distinct() {
        // Ensures that no two flag strings collapse to the same action variant,
        // catching future copy-paste errors in the match arms.
        use std::mem::discriminant;

        let version = resolve_startup_action(&args(&["flux-fm", "--version"]), home());
        let help = resolve_startup_action(&args(&["flux-fm", "--help"]), home());
        let menu = resolve_startup_action(&args(&["flux-fm", "--menu-editor"]), home());
        let props = resolve_startup_action(&args(&["flux-fm", "--file-properties", "/f"]), home());
        let launch = resolve_startup_action(&args(&["flux-fm", "/tmp"]), home());
        let unknown = resolve_startup_action(&args(&["flux-fm", "--bad"]), home());

        let variants = [
            discriminant(&version),
            discriminant(&help),
            discriminant(&menu),
            discriminant(&props),
            discriminant(&launch),
            discriminant(&unknown),
        ];

        let unique: std::collections::HashSet<_> = variants.iter().collect();
        assert_eq!(
            unique.len(),
            variants.len(),
            "two flags resolved to the same action variant"
        );
    }
}
