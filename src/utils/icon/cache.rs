use adw::gio;
use std::cell::RefCell;
use std::collections::HashMap;

thread_local! {
    pub(super) static THEMED_ICON_CACHE: RefCell<HashMap<String, gio::Icon>> =
        RefCell::new(HashMap::new());
}

/// Clears the cached GIO icons so they can be re-resolved under a new GTK theme.
pub fn invalidate_themed_icon_cache() {
    THEMED_ICON_CACHE.with(|cache| {
        cache.borrow_mut().clear();
    });
}
