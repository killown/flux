//! Named Quick Lists. Pure data and transitions. No widgets and no I/O.
//!
//! `FluxApp::exclusive_list` / `exclusive_index` always hold the *selected*
//! list, so Tab, Ctrl+PageUp/PageDown and Ctrl+1..9 (see [`super::quick_list`])
//! only ever walk that one list. [`QuickGroups`] holds every list, the selected
//! one included. Its copy of the selected list goes stale while the user
//! navigates, so readers go through [`QuickGroups::with_selected`], which lays
//! the live selected list over it.
//!
//! Invariant G: there is at least one list, `selected < lists.len()`, names are
//! non-empty, trimmed and unique ignoring case, and every list satisfies
//! invariant Q of [`super::quick_list`].

use super::quick_list;
use std::path::{Path, PathBuf};

/// Name of the list that exists when nothing else was saved.
pub const DEFAULT_NAME: &str = "Quick List";

/// Longest list name kept, in characters.
pub const MAX_NAME_CHARS: usize = 64;

/// One named list of folders and its current slot.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QuickGroup {
    pub name: String,
    pub items: Vec<PathBuf>,
    pub index: Option<usize>,
}

impl QuickGroup {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            items: Vec::new(),
            index: None,
        }
    }
}

/// Why a list name was refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NameError {
    /// Blank after trimming.
    Empty,
    /// Another list already has this name (ignoring case).
    Taken,
}

/// Every Quick List plus which one Tab walks.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QuickGroups {
    pub lists: Vec<QuickGroup>,
    pub selected: usize,
}

/// One list as the panel draws it: a header row and the item rows to show.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GroupView {
    /// Position in [`QuickGroups::lists`].
    pub list: usize,
    pub name: String,
    pub selected: bool,
    /// Items in the list, shown or not.
    pub total: usize,
    /// The list's current slot, shown or not.
    pub current: Option<usize>,
    /// `(slot, path)` of every row that passed the filter.
    pub rows: Vec<(usize, PathBuf)>,
}

/// Trims and caps a name. `None` when nothing is left.
fn clean_name(raw: &str) -> Option<String> {
    let t: String = raw.trim().chars().take(MAX_NAME_CHARS).collect();
    let t = t.trim().to_string();
    (!t.is_empty()).then_some(t)
}

fn is_clean(name: &str) -> bool {
    !name.is_empty() && name.trim() == name
}

fn same_name(a: &str, b: &str) -> bool {
    a.to_lowercase() == b.to_lowercase()
}

impl QuickGroups {
    /// One list holding `items`, with its slot clamped like a restored list.
    pub fn single(name: &str, items: Vec<PathBuf>, slot: Option<usize>) -> Self {
        let index = quick_list::restore(&items, slot);
        let name = clean_name(name).unwrap_or_else(|| DEFAULT_NAME.to_string());
        Self {
            lists: vec![QuickGroup { name, items, index }],
            selected: 0,
        }
    }

    pub fn invariant_holds(&self) -> bool {
        if self.lists.is_empty() || self.selected >= self.lists.len() {
            return false;
        }
        for (i, g) in self.lists.iter().enumerate() {
            if !is_clean(&g.name) || !quick_list::invariant_holds(&g.items, g.index) {
                return false;
            }
            if self.lists[..i].iter().any(|o| same_name(&o.name, &g.name)) {
                return false;
            }
        }
        true
    }

    /// Repairs whatever a database or a hand edit left behind so that
    /// invariant G holds again. The first of two equal names keeps its name.
    pub fn normalize(&mut self) {
        if self.lists.is_empty() {
            self.lists.push(QuickGroup::new(DEFAULT_NAME));
        }
        for i in 0..self.lists.len() {
            let base = clean_name(&self.lists[i].name).unwrap_or_else(|| DEFAULT_NAME.to_string());
            let mut name = base.clone();
            let mut n = 2;
            while self.lists[..i].iter().any(|g| same_name(&g.name, &name)) {
                name = format!("{base} {n}");
                n += 1;
            }
            let g = &mut self.lists[i];
            g.name = name;
            g.index = quick_list::restore(&g.items, g.index);
        }
        self.selected = self.selected.min(self.lists.len() - 1);
    }

    /// The selected list as stored. Possibly stale, see the module docs.
    pub fn selected_list(&self) -> &QuickGroup {
        &self.lists[self.selected]
    }

    /// A copy with the live selected list laid over the stored one.
    pub fn with_selected(&self, items: &[PathBuf], index: Option<usize>) -> QuickGroups {
        let mut out = self.clone();
        if let Some(g) = out.lists.get_mut(out.selected) {
            g.items = items.to_vec();
            g.index = index;
        }
        out
    }

    fn name_taken(&self, name: &str, except: Option<usize>) -> bool {
        self.lists
            .iter()
            .enumerate()
            .any(|(i, g)| Some(i) != except && same_name(&g.name, name))
    }

    /// `base`, or `base 2`, `base 3`, ... when `base` is taken.
    pub fn unique_name(&self, base: &str) -> String {
        let base = clean_name(base).unwrap_or_else(|| DEFAULT_NAME.to_string());
        if !self.name_taken(&base, None) {
            return base;
        }
        let mut n = 2;
        loop {
            let candidate = format!("{base} {n}");
            if !self.name_taken(&candidate, None) {
                return candidate;
            }
            n += 1;
        }
    }

    /// Appends an empty list. The selection does not move.
    pub fn add_list(&mut self, name: &str) -> Result<usize, NameError> {
        let name = clean_name(name).ok_or(NameError::Empty)?;
        if self.name_taken(&name, None) {
            return Err(NameError::Taken);
        }
        self.lists.push(QuickGroup::new(name));
        Ok(self.lists.len() - 1)
    }

    /// `Ok(true)` when the name changed. Renaming a list to its own name with
    /// different letter case is allowed.
    pub fn rename(&mut self, i: usize, name: &str) -> Result<bool, NameError> {
        let name = clean_name(name).ok_or(NameError::Empty)?;
        if i >= self.lists.len() {
            return Ok(false);
        }
        if self.name_taken(&name, Some(i)) {
            return Err(NameError::Taken);
        }
        if self.lists[i].name == name {
            return Ok(false);
        }
        self.lists[i].name = name;
        Ok(true)
    }

    /// Removes list `i`. The last remaining list cannot be deleted. When the
    /// selected list goes, the one after it is selected (the one before it
    /// when it was the last).
    pub fn delete(&mut self, i: usize) -> bool {
        if self.lists.len() <= 1 || i >= self.lists.len() {
            return false;
        }
        self.lists.remove(i);
        if i < self.selected {
            self.selected -= 1;
        }
        self.selected = self.selected.min(self.lists.len() - 1);
        true
    }

    /// Makes list `i` the one Tab walks. `true` when the selection changed.
    pub fn select(&mut self, i: usize) -> bool {
        if i < self.lists.len() && i != self.selected {
            self.selected = i;
            true
        } else {
            false
        }
    }

    /// Adds unseen paths to the FRONT of list `i`, see [`quick_list::add_front`].
    pub fn add_to(&mut self, i: usize, paths: impl IntoIterator<Item = PathBuf>) -> usize {
        match self.lists.get_mut(i) {
            Some(g) => quick_list::add_front(&mut g.items, &mut g.index, paths),
            None => 0,
        }
    }

    /// Removes `path` from list `i`. The slot lands one before the removed
    /// item, like removing a button from the old dock did.
    pub fn remove_from(&mut self, i: usize, path: &Path) -> bool {
        let Some(g) = self.lists.get_mut(i) else {
            return false;
        };
        let Some(pos) = g.items.iter().position(|p| p == path) else {
            return false;
        };
        g.items.remove(pos);
        g.index = if g.items.is_empty() {
            None
        } else {
            Some(pos.saturating_sub(1).min(g.items.len() - 1))
        };
        true
    }

    /// Moves `path` from list `from` to the front of list `to`. When `to`
    /// already has it, the two copies merge.
    pub fn move_item(&mut self, path: &Path, from: usize, to: usize) -> bool {
        if from == to || to >= self.lists.len() {
            return false;
        }
        if !self.remove_from(from, path) {
            return false;
        }
        self.add_to(to, std::iter::once(path.to_path_buf()));
        true
    }

    /// Points slot `slot` of list `i` at `path`. Refused when the slot does
    /// not exist or `path` already sits in another slot of the same list.
    pub fn replace_slot(&mut self, i: usize, slot: usize, path: PathBuf) -> bool {
        let Some(g) = self.lists.get_mut(i) else {
            return false;
        };
        if slot >= g.items.len() {
            return false;
        }
        if g.items
            .iter()
            .enumerate()
            .any(|(s, p)| s != slot && *p == path)
        {
            return false;
        }
        g.items[slot] = path;
        g.index = Some(slot);
        true
    }

    /// Selects list `i` and puts its slot on `path`. `false` when either does
    /// not exist, and nothing changes then.
    pub fn open_item(&mut self, i: usize, path: &Path) -> bool {
        let Some(g) = self.lists.get_mut(i) else {
            return false;
        };
        let Some(pos) = g.items.iter().position(|p| p == path) else {
            return false;
        };
        g.index = Some(pos);
        self.selected = i;
        true
    }

    /// Empties list `i` but keeps the list. `true` when it had items.
    pub fn clear_list(&mut self, i: usize) -> bool {
        let Some(g) = self.lists.get_mut(i) else {
            return false;
        };
        let had = !g.items.is_empty();
        g.items.clear();
        g.index = None;
        had
    }

    /// The lists as the panel shows them.
    ///
    /// `hit` decides whether a piece of text matches the filter, pass
    /// `|_| true` for no filter. A list whose name matches is shown with all
    /// of its items. Otherwise it is shown with only the items whose `text`
    /// matches, and hidden when there are none.
    pub fn views(
        &self,
        hit: impl Fn(&str) -> bool,
        text: impl Fn(&Path) -> String,
    ) -> Vec<GroupView> {
        let mut out = Vec::new();
        for (list, g) in self.lists.iter().enumerate() {
            let name_hit = hit(&g.name);
            let rows: Vec<(usize, PathBuf)> = g
                .items
                .iter()
                .enumerate()
                .filter(|(_, p)| name_hit || hit(&text(p.as_path())))
                .map(|(slot, p)| (slot, p.clone()))
                .collect();
            if name_hit || !rows.is_empty() {
                out.push(GroupView {
                    list,
                    name: g.name.clone(),
                    selected: list == self.selected,
                    total: g.items.len(),
                    current: g.index,
                    rows,
                });
            }
        }
        out
    }
}
