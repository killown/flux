//! Thread-local handle to the live contextual-actions panel widgets.
//!
//! Mirrors [`super::preview_registry`]: `build_actions_panel` installs the
//! handles once, and `FluxApp::handle_actions_panel_ready` reads them back so
//! no widget has to be threaded through `FluxApp`.

use std::cell::RefCell;
use std::path::PathBuf;

/// The list, search entry and page stack of the contextual-actions panel.
type Handle = (gtk::ListBox, gtk::SearchEntry, gtk::Stack);

thread_local! {
    static CURRENT: RefCell<Option<Handle>> = const { RefCell::new(None) };
    static MIME: RefCell<String> = const { RefCell::new(String::new()) };
    static TEMPLATE_PATH: RefCell<Option<PathBuf>> = const { RefCell::new(None) };
}

/// Registers the widgets of the freshly built panel as the current ones.
pub(crate) fn set_current(list: gtk::ListBox, search: gtk::SearchEntry, stack: gtk::Stack) {
    CURRENT.with(|c| *c.borrow_mut() = Some((list, search, stack)));
}

/// Runs `f` on the current handle, or returns `None` when no panel is built.
///
/// Callers that touch widgets should pass `Clone::clone` and work on the
/// returned copy, so the registry is not borrowed while GTK code runs.
pub(crate) fn with<R>(f: impl FnOnce(&Handle) -> R) -> Option<R> {
    CURRENT.with(|c| c.borrow().as_ref().map(f))
}

/// Remembers the MIME type the panel is currently showing actions for.
pub(crate) fn set_mime(mime: &str) {
    MIME.with(|m| *m.borrow_mut() = mime.to_string());
}

/// The MIME type of the last list the panel displayed, empty before the first load.
pub(crate) fn mime() -> String {
    MIME.with(|m| m.borrow().clone())
}

/// Remembers the template file the panel is currently showing actions from.
///
/// Called from `handle_actions_panel_ready` with the path returned by
/// `resolve_secondary_menu_template`. `None` means the panel is showing the
/// empty page (no template matched the selection's MIME type), the Edit
/// button then has nothing to open and falls back to a toast.
pub(crate) fn set_template_path(path: Option<PathBuf>) {
    TEMPLATE_PATH.with(|p| *p.borrow_mut() = path);
}
