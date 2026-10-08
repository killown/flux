mod icons;
mod launch;
mod shortcuts;
mod watcher;

use crate::model::Config;
use crate::ui::FileProperties;
use flux::args::{resolve_startup_action, StartupAction};
use relm4::prelude::*;
use std::io::BufRead;
use std::path::PathBuf;

use icons::{reset_icon_on_target, set_icon_on_target};

/// The binary's real entry point. `main.rs` calls this.
pub fn run() {
    crate::i18n::init();
    adw::init().expect("Failed to initialize Libadwaita");

    let args: Vec<String> = std::env::args().collect();
    let home_dir = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));

    match resolve_startup_action(&args, home_dir.clone()) {
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
            println!("  -h, --help                  Print this help message");
            println!("  -v, --version               Print version information");
            println!("      --no-sidebar            Disable sidebar for this session");
            println!("      --no-header             Disable header bar for this session");
            println!("      --no-statusbar          Disable statusbar for this session");
            println!("      --file-properties PATH  Open the file properties window for PATH");
            println!("      --set-icon TARGET IMAGE Set a custom icon/image for TARGET");
            println!(
                "      --set-icons-stdin       Read TARGET<TAB>IMAGE pairs from standard input"
            );
            println!("      --reset-icon TARGET     Reset custom icon for TARGET back to default");
            println!(
                "      --reset-icons-stdin     Read TARGET paths from standard input to reset"
            );
            println!(
                "      --quick-list PATH...    Open with quick-panel pre-populated with PATHs"
            );
            println!(
                "      --quick-list-stdin      Open with quick-panel populated from standard input"
            );
            println!("      --menu-editor           To manage menu.rs");
        }

        StartupAction::FileProperties(path) => {
            let app = RelmApp::new("flux.PropertiesViewer");
            app.allow_multiple_instances(true);
            app.with_args(vec![]).run::<FileProperties>(path);
        }

        StartupAction::MenuEditor => {
            crate::ui::menu::editor::run();
        }

        StartupAction::UnknownFlag(flag) => {
            eprintln!("flux: unrecognized option '{flag}'");
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
                false,
                false,
                false,
            );
        }

        StartupAction::Launch {
            path,
            no_sidebar,
            no_header,
            no_statusbar,
        } => {
            launch::launch_main_app(path, None, None, None, no_sidebar, no_header, no_statusbar);
        }

        StartupAction::TagSearch {
            tag,
            no_sidebar,
            no_header,
            no_statusbar,
        } => {
            launch::launch_main_app(
                home_dir,
                None,
                None,
                Some(tag),
                no_sidebar,
                no_header,
                no_statusbar,
            );
        }

        StartupAction::QuickList {
            paths,
            no_sidebar,
            no_header,
            no_statusbar,
        } => {
            let first = paths.first().cloned().unwrap_or_else(|| home_dir.clone());
            launch::launch_main_app(
                first,
                None,
                Some(paths),
                None,
                no_sidebar,
                no_header,
                no_statusbar,
            );
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

            for line in stdin.lock().lines().map_while(Result::ok) {
                let trimmed = line.trim();
                if trimmed.is_empty() || trimmed.starts_with('#') {
                    continue;
                }
                let parts: Vec<&str> = if trimmed.contains('\t') {
                    trimmed.split('\t').collect()
                } else {
                    trimmed.split_whitespace().collect()
                };

                if parts.len() >= 2 {
                    let target = PathBuf::from(parts[0]);
                    let image = PathBuf::from(parts[1]);
                    let target_canon = target.canonicalize().unwrap_or(target);
                    let image_canon = image.canonicalize().unwrap_or(image);

                    if image_canon.exists() {
                        set_icon_on_target(&mut config, &target_canon, &image_canon);
                        count += 1;
                    }
                }
            }

            crate::utils::save_config(&config);
            println!("Batch updated {} custom icon(s)", count);
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
