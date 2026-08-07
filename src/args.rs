use std::path::PathBuf;

/// Describes the action the process should take after argument parsing.
#[derive(Debug, PartialEq)]
pub enum StartupAction {
    /// Launch the main file-manager window starting at the given path.
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
