//! Toolkit-free handle that aborts a background operation.

use std::sync::Arc;

/// Cancels the operation it was created for. Cloning shares the same operation.
///
/// The code that owns the real cancellation token (for example a
/// `gio::Cancellable`) builds the handle with [`CancelHandle::new`], so
/// messages and the task queue can carry it without naming that token's type.
#[derive(Clone)]
pub struct CancelHandle(Arc<dyn Fn() + Send + Sync>);

impl CancelHandle {
    /// Wraps `cancel`, which is called every time [`CancelHandle::cancel`] is.
    pub fn new(cancel: impl Fn() + Send + Sync + 'static) -> Self {
        Self(Arc::new(cancel))
    }

    /// Aborts the underlying operation.
    pub fn cancel(&self) {
        (self.0)();
    }
}

impl std::fmt::Debug for CancelHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("CancelHandle")
    }
}
