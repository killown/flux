use std::io::BufRead;
use std::path::PathBuf;

fn uri_to_path(s: &str) -> PathBuf {
    if let Some(stripped) = s.strip_prefix("file://") {
        PathBuf::from(stripped)
    } else {
        PathBuf::from(s)
    }
}

/// Describes the action the process should take after argument parsing.
#[derive(Debug, PartialEq)]
pub enum StartupAction {
    /// Launch the main file-manager window starting at the given path.
    Launch {
        path: PathBuf,
        no_sidebar: bool,
        no_header: bool,
        no_statusbar: bool,
    },
    /// Launch the main window with an active tag search filter applied.
    TagSearch {
        tag: String,
        no_sidebar: bool,
        no_header: bool,
        no_statusbar: bool,
    },
    /// Launch the main window with pre-seeded quick-panel triage items.
    QuickList {
        paths: Vec<PathBuf>,
        no_sidebar: bool,
        no_header: bool,
        no_statusbar: bool,
    },
    /// Open the archive-explorer window for the given path.
    OpenArchive(PathBuf),
    /// Open the standalone file-properties dialog for the given path.
    FileProperties(PathBuf),
    /// Set a custom icon/image override for a target path.
    SetIcon { target: PathBuf, image: PathBuf },
    /// Read TAB/space-separated `<TARGET>\t<IMAGE>` pairs from standard input.
    SetIconsStdin,
    /// Reset a custom icon override for a target path back to default.
    ResetIcon(PathBuf),
    /// Read TARGET paths from standard input to reset custom icons.
    ResetIconsStdin,
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
pub fn resolve_startup_action(args: &[String], home_dir: PathBuf) -> StartupAction {
    resolve_startup_action_with_reader(args, home_dir, std::io::stdin().lock())
}

/// Parses process arguments using a custom buffered reader stream for stdin operations,
/// enabling non-blocking unit testing.
pub fn resolve_startup_action_with_reader<R: BufRead>(
    args: &[String],
    home_dir: PathBuf,
    reader: R,
) -> StartupAction {
    let mut no_sidebar = false;
    let mut no_header = false;
    let mut no_statusbar = false;
    let mut filtered_args = Vec::new();

    for (i, arg) in args.iter().enumerate() {
        if i == 0 {
            continue;
        }
        match arg.as_str() {
            "--no-sidebar" => no_sidebar = true,
            "--no-header" => no_header = true,
            "--no-statusbar" => no_statusbar = true,
            _ => filtered_args.push(arg.as_str()),
        }
    }

    let positional = filtered_args.first().copied();
    match positional {
        None => StartupAction::Launch {
            path: home_dir,
            no_sidebar,
            no_header,
            no_statusbar,
        },
        Some("--version" | "-v") => StartupAction::PrintVersion,
        Some("--help" | "-h") => StartupAction::PrintHelp,
        Some("--menu-editor") => StartupAction::MenuEditor,
        Some("--file-properties") => match filtered_args.get(1) {
            Some(path) => StartupAction::FileProperties(uri_to_path(path)),
            None => StartupAction::PrintHelp,
        },
        Some("--set-icon") => match (filtered_args.get(1), filtered_args.get(2)) {
            (Some(target), Some(image)) => StartupAction::SetIcon {
                target: uri_to_path(target),
                image: uri_to_path(image),
            },
            _ => StartupAction::PrintHelp,
        },
        Some("--set-icons-stdin") => StartupAction::SetIconsStdin,
        Some("--reset-icon") => match filtered_args.get(1) {
            Some(target) => StartupAction::ResetIcon(uri_to_path(target)),
            None => StartupAction::PrintHelp,
        },
        Some("--reset-icons-stdin") => StartupAction::ResetIconsStdin,
        Some("--quick-list") => {
            let list: Vec<PathBuf> = filtered_args[1..]
                .iter()
                .filter(|a| !a.starts_with('-'))
                .map(|p| uri_to_path(p))
                .collect();
            if list.is_empty() {
                StartupAction::PrintHelp
            } else {
                StartupAction::QuickList {
                    paths: list,
                    no_sidebar,
                    no_header,
                    no_statusbar,
                }
            }
        }
        Some("--quick-list-stdin") => {
            let mut list = Vec::new();
            for line in reader.lines().map_while(Result::ok) {
                let trimmed = line.trim();
                if !trimmed.is_empty() && !trimmed.starts_with('#') {
                    list.push(uri_to_path(trimmed));
                }
            }
            if list.is_empty() {
                StartupAction::Launch {
                    path: home_dir,
                    no_sidebar,
                    no_header,
                    no_statusbar,
                }
            } else {
                StartupAction::QuickList {
                    paths: list,
                    no_sidebar,
                    no_header,
                    no_statusbar,
                }
            }
        }
        Some(arg) if arg.starts_with('-') => StartupAction::UnknownFlag(arg.to_string()),
        Some(path) => {
            if path.starts_with('#')
                || path.starts_with(":tag:")
                || path.starts_with(":t:")
                || path.starts_with("tags://")
            {
                let clean = path
                    .trim_start_matches("tags://")
                    .trim_start_matches(":tag:")
                    .trim_start_matches(":t:")
                    .trim_start_matches('#');
                StartupAction::TagSearch {
                    tag: format!("#{clean}"),
                    no_sidebar,
                    no_header,
                    no_statusbar,
                }
            } else {
                let p = uri_to_path(path);
                if p.is_file() && crate::services::archive::is_supported_archive(&p) {
                    StartupAction::OpenArchive(p)
                } else {
                    StartupAction::Launch {
                        path: p,
                        no_sidebar,
                        no_header,
                        no_statusbar,
                    }
                }
            }
        }
    }
}
