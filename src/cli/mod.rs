mod icons;
mod launch;
mod shortcuts;
mod watcher;

use crate::infra::window_control;
use crate::model::Config;
use flux::args::{parse_icon_pair, resolve_startup_action, StartupAction};
use std::io::BufRead;
use std::path::PathBuf;

use icons::{reset_icon_on_target, set_icon_on_target};

/// The binary's real entry point. `main.rs` calls this.
pub fn run() {
    let args: Vec<String> = std::env::args().collect();
    let home_dir = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    let action = resolve_startup_action(&args, home_dir.clone());

    // Runs before GTK initialisation so it needs no display and stays cheap
    // enough to be called from every `$F` expansion in a shell.
    if let StartupAction::PrintSelection { null } = action {
        std::process::exit(crate::infra::terminal::print_selection(null));
    }

    // Talks to a running window over its socket, so it needs no display either.
    match action {
        StartupAction::Toggle if window_control::toggle_running() == 0 => std::process::exit(0),
        StartupAction::Show if window_control::show_running() == 0 => std::process::exit(0),
        StartupAction::Hide => std::process::exit(window_control::hide_running()),
        _ => {}
    }

    // Bring the `--cwd` IPC server up before any client can connect. Every
    // process that may create the main window runs this, including the one
    // that receives `--cwd` and falls through to `launch_main_app` below,
    // otherwise a `--cwd` racing with a cold start would open a second window.
    let cwd_rx = crate::ipc::start_server();

    // `--cwd`: talk to a running window. If `cwd_rx` is `None`, another Flux
    // instance already owns the socket, so act as a client and exit. If it is
    // `Some`, we are the first instance: fall through to launch a window at
    // the requested directory. That window will serve subsequent CLI calls.
    if let StartupAction::Cwd(ref path) = action {
        let canonical = match path.canonicalize() {
            Ok(p) if p.is_dir() => p,
            _ => {
                eprintln!("flux: --cwd: not a directory: {}", path.display());
                std::process::exit(1);
            }
        };
        if cwd_rx.is_none() {
            match crate::ipc::send_cwd(&canonical) {
                crate::ipc::ClientOutcome::Ok => std::process::exit(0),
                crate::ipc::ClientOutcome::Rejected => {
                    eprintln!("flux: --cwd: running window rejected the request");
                    std::process::exit(1);
                }
                crate::ipc::ClientOutcome::NotRunning => {
                    // Socket vanished between our bind attempt and connect,
                    // fall through to launching rather than silently failing.
                }
            }
        }
    }

    crate::i18n::init();
    adw::init().expect("Failed to initialize Libadwaita");

    // Defer icon setup past the first frame so the synchronous icon-theme
    // lookup doesn't block cold start.
    glib::idle_add_local_once(|| {
        gtk::Window::set_default_icon_name("io.github.killown.flux");
    });

    match action {
        StartupAction::PrintVersion => {
            let commit = env!("FLUX_GIT_HASH");
            if commit.is_empty() {
                println!("flux {}", env!("CARGO_PKG_VERSION"));
            } else {
                println!("flux {} ({})", env!("CARGO_PKG_VERSION"), commit);
            }
        }

        StartupAction::PrintHelp => {
            println!("Usage: flux [OPTIONS] [PATH]");
            println!();
            println!("Arguments:");
            println!("  [PATH]  Directory or archive file to open on startup");
            println!();
            println!("Options:");
            for (flags, description) in flux::args::CLI_OPTIONS {
                println!("  {flags:<28}{description}");
            }
        }

        StartupAction::PrintSelection { .. } => unreachable!("handled before GTK initialisation"),

        StartupAction::Toggle | StartupAction::Show => {
            launch::launch_main_app(
                home_dir,
                None,
                None,
                None,
                flux::args::WindowOptions::default(),
                cwd_rx,
            );
        }

        StartupAction::Hide => unreachable!("handled before GTK initialisation"),

        StartupAction::UnknownFlag(flag) => {
            eprintln!("flux: unrecognized option '{flag}'");
            eprintln!("Try 'flux --help' for more information.");
            std::process::exit(1);
        }

        StartupAction::InvalidArgument(msg) => {
            eprintln!("flux: {msg}");
            eprintln!("Try 'flux --help' for more information.");
            std::process::exit(1);
        }

        StartupAction::OpenArchive(archive_path) => {
            let start_path = archive_path.parent().unwrap_or(&archive_path).to_path_buf();
            launch::launch_main_app(
                start_path,
                Some(archive_path),
                None,
                None,
                flux::args::WindowOptions::default(),
                cwd_rx,
            );
        }

        StartupAction::Launch { path, opts } => {
            launch::launch_main_app(path, None, None, None, opts, cwd_rx);
        }

        // Reached when this is the first Flux instance (or no window answered
        // the IPC): open a window at the requested directory.
        StartupAction::Cwd(path) => {
            launch::launch_main_app(
                path,
                None,
                None,
                None,
                flux::args::WindowOptions::default(),
                cwd_rx,
            );
        }

        StartupAction::Preview { path, opts } => {
            launch::launch_main_app(path, None, None, None, opts, cwd_rx);
        }

        StartupAction::TagSearch { tag, opts } => {
            launch::launch_main_app(home_dir, None, None, Some(tag), opts, cwd_rx);
        }

        StartupAction::QuickList { paths, opts } => {
            let first = paths.first().cloned().unwrap_or_else(|| home_dir.clone());
            launch::launch_main_app(first, None, Some(paths), None, opts, cwd_rx);
        }

        StartupAction::SetIcon { target, image } => {
            let target_canon = target.canonicalize().unwrap_or(target);
            let image_canon = image.canonicalize().unwrap_or(image);

            if !image_canon.exists() {
                eprintln!("flux: icon file not found: {}", image_canon.display());
                std::process::exit(1);
            }

            let mut config: Config = crate::utils::load_config();
            set_icon_on_target(&mut config, &target_canon, &image_canon);
            crate::utils::save_config(&config);
            println!("Custom icon applied to {}", target_canon.display());
        }

        StartupAction::SetIconsStdin => {
            let mut config = crate::utils::load_config();
            let stdin = std::io::stdin();
            let mut count = 0;
            let mut skipped = 0;

            for (idx, line) in stdin.lock().lines().map_while(Result::ok).enumerate() {
                let trimmed = line.trim();
                if trimmed.is_empty() || trimmed.starts_with('#') {
                    continue;
                }
                let line_no = idx + 1;

                let (target, image) = match parse_icon_pair(trimmed) {
                    Ok(pair) => pair,
                    Err(msg) => {
                        eprintln!("flux: line {line_no}: {msg}");
                        skipped += 1;
                        continue;
                    }
                };

                let target_canon = target.canonicalize().unwrap_or(target);
                let image_canon = image.canonicalize().unwrap_or(image);

                if !image_canon.exists() {
                    eprintln!(
                        "flux: line {line_no}: icon file not found: {}",
                        image_canon.display()
                    );
                    skipped += 1;
                    continue;
                }

                set_icon_on_target(&mut config, &target_canon, &image_canon);
                count += 1;
            }

            crate::utils::save_config(&config);
            println!("Batch updated {} custom icon(s)", count);
            if skipped > 0 {
                eprintln!("flux: skipped {skipped} line(s)");
                std::process::exit(1);
            }
        }

        StartupAction::SetTheme(name) => {
            let available = crate::utils::helpers::list_available_themes();
            if name != "default" && !available.contains(&name) {
                eprintln!("flux: unknown theme '{name}'");
                let mut names = vec!["default"];
                names.extend(available.iter().map(String::as_str));
                eprintln!("Available: {}", names.join(", "));
                std::process::exit(1);
            }
            let mut config = crate::utils::load_config();
            config.ui.theme = (name != "default").then(|| name.clone());
            crate::utils::save_config(&config);
            println!("Theme set to {name}");
        }

        StartupAction::ResetIcon(target) => {
            let target_canon = target.canonicalize().unwrap_or(target);
            let mut config = crate::utils::load_config();
            reset_icon_on_target(&mut config, &target_canon);
            crate::utils::save_config(&config);
            println!("Reset icon for {}", target_canon.display());
        }

        StartupAction::ResetIconsStdin => {
            let mut config = crate::utils::load_config();
            let stdin = std::io::stdin();
            let mut count = 0;

            for line in stdin.lock().lines().map_while(Result::ok) {
                let trimmed = line.trim();
                if trimmed.is_empty() || trimmed.starts_with('#') {
                    continue;
                }

                let target = PathBuf::from(trimmed);
                let target_canon = target.canonicalize().unwrap_or(target);
                reset_icon_on_target(&mut config, &target_canon);
                count += 1;
            }

            crate::utils::save_config(&config);
            println!("Batch reset {} custom icon(s)", count);
        }
    }
}
