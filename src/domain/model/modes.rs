use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FilterMode {
    #[default]
    All,
    OnlyFolders,
    OnlyFiles,
}

/// State for the `Ctrl+Alt+<letter>` folder cycle.
#[derive(Debug, Clone)]
pub struct AltCycle {
    /// Directory whose children are being cycled.
    pub anchor: PathBuf,
    /// Lowercased letter prefix being cycled.
    pub letter: char,
    /// Candidates captured on the first press, in the loader's sort order.
    pub candidates: Vec<PathBuf>,
    /// Index of the candidate currently entered / last chosen.
    pub index: usize,
    /// Candidates that are archives drawn as folders (`is_virtual_dir`), entered
    /// with `EnterArchive`. Recorded when the candidates are built, because by
    /// the time they are chosen the grid may show another folder or an archive.
    pub virtual_targets: Vec<PathBuf>,
}

/// State for Navigate mode, a session stance where plain letters enter the
/// folders starting with that letter (the `Ctrl+Alt+<letter>` cycle without
/// the chord) instead of filtering.
///
/// Lives on `FluxApp`, not on a tab and not in `config.toml`: it is entered
/// and left by a chord (or Esc), survives navigation and tab switches, and is
/// gone when Flux closes. The cycle itself is [`AltCycle`].
#[derive(Debug, Clone, Default)]
pub struct NavigateMode {
    /// Folders the cycle was based in before Enter made a deeper folder the
    /// new base, innermost last. Backspace walks back through them.
    pub anchors: Vec<PathBuf>,
    /// Search-bar filter that was active when the mode was entered, put back
    /// on exit.
    pub stashed_filter: Option<String>,
    /// A first-item selection is owed: set on entering the mode and when a
    /// navigation starts inside it, consumed by the next finished folder load.
    /// A refresh never sets it, so it cannot pull the selection back to the top.
    pub select_first: bool,
    /// Path of the item the mode selected on its own, to tell that selection
    /// apart from one the user moved with the arrow keys.
    pub auto_selected: Option<PathBuf>,
}
