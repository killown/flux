use super::preview::PreviewPanelHandle;
use std::cell::RefCell;

thread_local! {
    static CURRENT: RefCell<Option<PreviewPanelHandle>> = const { RefCell::new(None) };
}

pub(crate) fn set_current(handle: PreviewPanelHandle) {
    CURRENT.with(|c| *c.borrow_mut() = Some(handle));
}

pub(crate) fn with<R>(f: impl FnOnce(&PreviewPanelHandle) -> R) -> Option<R> {
    CURRENT.with(|c| c.borrow().as_ref().map(f))
}

pub(crate) fn get_view_switcher() -> Option<adw::ViewSwitcher> {
    with(|h| h.switcher.clone())
}

pub(crate) fn get_view_stack() -> Option<adw::ViewStack> {
    with(|h| h.view_stack.clone())
}

pub(crate) fn set_mime_filter_active(on: bool) {
    with(|h| h.set_mime_filter_active(on));
}
