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
    Launch(PathBuf),
    /// open the archive-explorer window for the given path.
    OpenArchive(PathBuf),
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
pub fn resolve_startup_action(args: &[String], home_dir: PathBuf) -> StartupAction {
    let positional = args.get(1).map(String::as_str);
    match positional {
        None => StartupAction::Launch(home_dir),
        Some("--version" | "-v") => StartupAction::PrintVersion,
        Some("--help" | "-h") => StartupAction::PrintHelp,
        Some("--menu-editor") => StartupAction::MenuEditor,
        Some("--file-properties") => match args.get(2) {
            Some(path) => StartupAction::FileProperties(uri_to_path(path)),
            None => StartupAction::PrintHelp,
        },
        Some(arg) if arg.starts_with('-') => StartupAction::UnknownFlag(arg.to_string()),
        Some(path) => {
            let p = uri_to_path(path);
            if p.is_file() && crate::services::archive::is_supported_archive(&p) {
                StartupAction::OpenArchive(p)
            } else {
                StartupAction::Launch(p)
            }
        }
    }
}
