//! Pure Quick List transitions. No widgets and no I/O.
//!
//! Invariant Q: `index.is_none() <=> list.is_empty()`, else `index < list.len()`.

use gtk::gdk::Key;
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    Next,
    Prev,
}

/// Quick List chords handled by the file grid.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QuickKey {
    AddCurrent,
    Clear,
    Prev,
    Next,
    Tab,
    Jump(usize),
    /// Ctrl+Shift+Q: show or hide the Quick List panel.
    TogglePanel,
}

pub fn invariant_holds(list: &[PathBuf], index: Option<usize>) -> bool {
    match index {
        None => list.is_empty(),
        Some(i) => i < list.len(),
    }
}

/// Maps a key press to a Quick List chord. Mirrors the arm order in
/// `setup_capture`: Ctrl+Insert (Navigate mode), Ctrl+Shift+Insert and Ctrl+Tab
/// are other features and never classify here.
pub fn classify_key(key: Key, ctrl: bool, shift: bool, alt: bool) -> Option<QuickKey> {
    if key == Key::Insert && !ctrl {
        return Some(QuickKey::AddCurrent);
    }
    if key == Key::End && ctrl && !shift && !alt {
        return Some(QuickKey::Clear);
    }
    if key == Key::Page_Up && ctrl {
        return Some(QuickKey::Prev);
    }
    if key == Key::Page_Down && ctrl {
        return Some(QuickKey::Next);
    }
    if key == Key::Tab && !ctrl {
        return Some(QuickKey::Tab);
    }
    if ctrl && shift && !alt && (key == Key::q || key == Key::Q) {
        return Some(QuickKey::TogglePanel);
    }
    if ctrl && !shift && !alt {
        let n = match key {
            Key::_1 => 1,
            Key::_2 => 2,
            Key::_3 => 3,
            Key::_4 => 4,
            Key::_5 => 5,
            Key::_6 => 6,
            Key::_7 => 7,
            Key::_8 => 8,
            Key::_9 => 9,
            _ => return None,
        };
        return Some(QuickKey::Jump(n));
    }
    None
}

/// The one place the terminal yield is decided: when the embedded terminal has
/// focus no Quick List chord is claimed, so the key reaches the shell.
pub fn dispatch_key(
    key: Key,
    ctrl: bool,
    shift: bool,
    alt: bool,
    terminal_focused: bool,
) -> Option<QuickKey> {
    if terminal_focused {
        return None;
    }
    classify_key(key, ctrl, shift, alt)
}

/// Clamps a restored slot into the restored list.
pub fn restore(list: &[PathBuf], saved: Option<usize>) -> Option<usize> {
    if list.is_empty() {
        None
    } else {
        Some(saved.filter(|&i| i < list.len()).unwrap_or(0))
    }
}

/// Inserts unseen paths at the front, keeping their relative order. The index
/// keeps pointing at the same folder. Returns how many were added.
pub fn add_front(
    list: &mut Vec<PathBuf>,
    index: &mut Option<usize>,
    paths: impl IntoIterator<Item = PathBuf>,
) -> usize {
    let mut fresh: Vec<PathBuf> = Vec::new();
    for p in paths {
        if !list.contains(&p) && !fresh.contains(&p) {
            fresh.push(p);
        }
    }
    let n = fresh.len();
    if n == 0 {
        return 0;
    }
    list.splice(0..0, fresh);
    *index = Some(match *index {
        Some(i) => i + n,
        None => 0,
    });
    n
}

/// Points the index at `path` when it is in the list.
pub fn sync_to(list: &[PathBuf], index: &mut Option<usize>, path: &Path) {
    if let Some(pos) = list.iter().position(|p| p == path) {
        *index = Some(pos);
    }
}

/// Empties the list. Returns whether there was anything to clear.
pub fn clear(list: &mut Vec<PathBuf>, index: &mut Option<usize>) -> bool {
    let had = !list.is_empty();
    list.clear();
    *index = None;
    had
}

/// Ctrl+PageDown / Ctrl+PageUp. A one-item list lands on that item.
pub fn step(list: &[PathBuf], index: &mut Option<usize>, dir: Step) -> Option<PathBuf> {
    let len = list.len();
    if len == 0 {
        return None;
    }
    let new = match (dir, *index) {
        (Step::Next, Some(i)) => (i + 1) % len,
        (Step::Next, None) => 0,
        (Step::Prev, Some(i)) if i > 0 => i - 1,
        (Step::Prev, _) => len - 1,
    };
    *index = Some(new);
    Some(list[new].clone())
}

/// Tab: go to the current slot, or advance when already exactly in it.
pub fn tab(list: &[PathBuf], index: &mut Option<usize>, current: &Path) -> Option<PathBuf> {
    match *index {
        Some(i) if i < list.len() && current == list[i] => step(list, index, Step::Next),
        Some(i) if i < list.len() => Some(list[i].clone()),
        _ if !list.is_empty() => {
            *index = Some(0);
            Some(list[0].clone())
        }
        _ => None,
    }
}

/// Ctrl+1..9 (`n` is 1-based). A one-item list takes every digit, with more
/// items a digit past the end does nothing.
pub fn jump(list: &[PathBuf], index: &mut Option<usize>, n: usize) -> Option<PathBuf> {
    let len = list.len();
    if len == 0 || n == 0 {
        return None;
    }
    let slot = if len == 1 {
        0
    } else if n <= len {
        n - 1
    } else {
        return None;
    };
    *index = Some(slot);
    Some(list[slot].clone())
}
