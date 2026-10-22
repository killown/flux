use std::io::BufRead;
use std::path::PathBuf;

pub use crate::model::WindowOptions;

/// Every command-line option as `(flags, description)`, shared by `--help` and `docs/cli.md`.
pub const CLI_OPTIONS: &[(&str, &str)] = &[
    ("-h, --help", "Print this help message"),
    ("-v, --version", "Print version information"),
    ("    --no-sidebar", "Disable sidebar for this session"),
    ("    --no-header", "Disable header bar for this session"),
    ("    --no-statusbar", "Disable statusbar for this session"),
    (
        "    --terminal",
        "Start with the terminal open in the configured position",
    ),
    (
        "    --file-properties PATH",
        "Open the file properties window for PATH",
    ),
    (
        "    --set-icon TARGET IMAGE",
        "Set a custom icon/image for TARGET",
    ),
    (
        "    --cwd PATH",
        "Navigate the running window to PATH (starts a new window if none is running)",
    ),
    (
        "    --set-icons-stdin",
        "Read TARGET<TAB>IMAGE pairs from standard input",
    ),
    (
        "    --reset-icon TARGET",
        "Reset custom icon for TARGET back to default",
    ),
    (
        "    --reset-icons-stdin",
        "Read TARGET paths from standard input to reset",
    ),
    (
        "    --quick-list PATH...",
        "Open with quick-panel pre-populated with PATHs",
    ),
    (
        "    --quick-list-stdin",
        "Open with quick-panel populated from standard input",
    ),
    (
        "    --preview PATH",
        "Open folder PATH with the preview panel in wide mode",
    ),
    (
        "    --selection",
        "Print the paths selected in the running window, one per line (-0: NUL-separated)",
    ),
    ("    --toggle", "Show or hide the running window"),
    ("    --show", "Bring the running window to the front"),
    ("    --hide", "Hide the running window"),
    (
        "    --theme NAME",
        "Set the theme to NAME, or 'default' to remove it",
    ),
    ("    --menu-editor", "To manage menu.rs"),
];

fn uri_to_path(s: &str) -> PathBuf {
    if let Some(stripped) = s.strip_prefix("file://") {
        PathBuf::from(stripped)
    } else {
        PathBuf::from(s)
    }
}

/// Splits shell-style text into words, honouring quotes and backslash escapes.
fn split_words(line: &str) -> Result<Vec<String>, String> {
    let mut words = Vec::new();
    let mut cur = String::new();
    let mut in_word = false;
    let mut chars = line.chars();

    while let Some(c) = chars.next() {
        match c {
            '\'' => {
                in_word = true;
                loop {
                    match chars.next() {
                        Some('\'') => break,
                        Some(ch) => cur.push(ch),
                        None => return Err("unterminated single quote".to_string()),
                    }
                }
            }
            '"' => {
                in_word = true;
                loop {
                    match chars.next() {
                        Some('"') => break,
                        Some('\\') => match chars.next() {
                            Some(ch @ ('"' | '\\')) => cur.push(ch),
                            Some(ch) => {
                                cur.push('\\');
                                cur.push(ch);
                            }
                            None => return Err("unterminated double quote".to_string()),
                        },
                        Some(ch) => cur.push(ch),
                        None => return Err("unterminated double quote".to_string()),
                    }
                }
            }
            '\\' => {
                in_word = true;
                cur.push(chars.next().unwrap_or('\\'));
            }
            c if c.is_whitespace() => {
                if in_word {
                    words.push(std::mem::take(&mut cur));
                    in_word = false;
                }
            }
            c => {
                in_word = true;
                cur.push(c);
            }
        }
    }
    if in_word {
        words.push(cur);
    }
    Ok(words)
}

/// Parses one `--set-icons-stdin` line into `(target, image)`, tab-separated, or two quoted/escaped words.
pub fn parse_icon_pair(line: &str) -> Result<(PathBuf, PathBuf), String> {
    let line = line.trim();

    let (target, image) = if line.contains('\t') {
        let mut fields = line.splitn(3, '\t');
        let target = fields.next().unwrap_or("").trim().to_string();
        let image = fields.next().unwrap_or("").trim().to_string();
        (target, image)
    } else {
        let mut words = split_words(line)?;
        if words.len() != 2 {
            return Err(format!(
                "expected TARGET and IMAGE, found {} value(s), separate them with a tab or quote paths containing spaces",
                words.len()
            ));
        }
        let image = words.pop().unwrap_or_default();
        let target = words.pop().unwrap_or_default();
        (target, image)
    };

    if target.is_empty() || image.is_empty() {
        return Err("TARGET and IMAGE must both be non-empty".to_string());
    }
    Ok((uri_to_path(&target), uri_to_path(&image)))
}

/// Describes the action the process should take after argument parsing.
#[derive(Debug, PartialEq)]
pub enum StartupAction {
    /// Launch the main file-manager window starting at the given path.
    Launch { path: PathBuf, opts: WindowOptions },
    /// Launch the main window at the given path with the preview panel open in wide mode.
    Preview { path: PathBuf, opts: WindowOptions },
    /// Launch the main window with an active tag search filter applied.
    TagSearch { tag: String, opts: WindowOptions },
    /// Launch the main window with pre-seeded quick-panel triage items.
    QuickList {
        paths: Vec<PathBuf>,
        opts: WindowOptions,
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
    /// Store a theme name in the config and exit.
    SetTheme(String),
    /// Ask the running window to navigate to PATH, or launch a new window there.
    Cwd(PathBuf),
    /// Open the context-menu editor utility.
    MenuEditor,
    /// Print the paths selected in the running window and exit.
    PrintSelection {
        /// Terminate each path with NUL instead of a newline.
        null: bool,
    },
    /// Show or hide the running window, or launch one when none is running.
    Toggle,
    /// Bring the running window to the front, or launch one when none is running.
    Show,
    /// Hide the running window, doing nothing when none is running.
    Hide,
    /// Print the version string and exit.
    PrintVersion,
    /// Print the help text and exit.
    PrintHelp,
    /// Exit with an error because an unrecognised flag was supplied.
    UnknownFlag(String),
    /// Exit with an error because a flag received a missing or invalid value.
    InvalidArgument(String),
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
    let mut opts = WindowOptions::default();
    let mut filtered_args = Vec::new();

    for (i, arg) in args.iter().enumerate() {
        if i == 0 {
            continue;
        }
        match arg.as_str() {
            "--no-sidebar" => opts.no_sidebar = true,
            "--no-header" => opts.no_header = true,
            "--no-statusbar" => opts.no_statusbar = true,
            "--terminal" => opts.terminal = true,
            _ => filtered_args.push(arg.as_str()),
        }
    }

    let positional = filtered_args.first().copied();
    match positional {
        None => StartupAction::Launch {
            path: home_dir,
            opts,
        },
        Some("--version" | "-v") => StartupAction::PrintVersion,
        Some("--help" | "-h") => StartupAction::PrintHelp,
        Some("--menu-editor") => StartupAction::MenuEditor,
        Some("--selection") => StartupAction::PrintSelection {
            null: filtered_args.get(1) == Some(&"-0"),
        },
        Some("--toggle") => StartupAction::Toggle,
        Some("--show") => StartupAction::Show,
        Some("--hide") => StartupAction::Hide,
        Some("--cwd") => match filtered_args.get(1) {
            Some(path) => StartupAction::Cwd(uri_to_path(path)),
            None => StartupAction::InvalidArgument("--cwd requires a path".to_string()),
        },
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
        Some("--theme") => match filtered_args.get(1) {
            Some(name) => StartupAction::SetTheme(name.to_string()),
            None => StartupAction::PrintHelp,
        },
        Some("--preview") => match filtered_args.get(1) {
            None => StartupAction::InvalidArgument("--preview requires a folder path".to_string()),
            Some(arg) => {
                let path = uri_to_path(arg);
                if path.is_dir() {
                    StartupAction::Preview {
                        path,
                        opts: WindowOptions {
                            preview: true,
                            ..opts
                        },
                    }
                } else {
                    StartupAction::InvalidArgument(format!(
                        "--preview: not a directory: {}",
                        path.display()
                    ))
                }
            }
        },
        Some("--quick-list") => {
            let list: Vec<PathBuf> = filtered_args[1..]
                .iter()
                .filter(|a| !a.starts_with('-'))
                .map(|p| uri_to_path(p))
                .collect();
            if list.is_empty() {
                StartupAction::PrintHelp
            } else {
                StartupAction::QuickList { paths: list, opts }
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
                    opts,
                }
            } else {
                StartupAction::QuickList { paths: list, opts }
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
                    opts,
                }
            } else {
                let p = uri_to_path(path);
                if p.is_file() && crate::services::archive::is_supported_archive(&p) {
                    StartupAction::OpenArchive(p)
                } else {
                    StartupAction::Launch { path: p, opts }
                }
            }
        }
    }
}
