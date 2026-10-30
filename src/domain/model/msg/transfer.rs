//! Transfer dialog and conflict resolution.

use crate::domain::model::ConflictResolver;
use crate::ui::conflict_policy::ConflictContext;

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum TransferMsg {
    /// A background copy/move worker detected that its destination path already
    /// exists and needs user input before it can proceed.
    FileConflictDetected {
        context: ConflictContext,
        resolver: ConflictResolver,
    },
    /// Sent by the conflict dialog's response closure immediately before it
    /// resolves the oneshot sender.  Clears `FluxApp::conflict_dialog_active`
    /// so the transfer-progress dialog is re-enabled.
    ConflictDialogClosed,
    /// Update (or reset) the session-scoped conflict-resolution policy for the
    /// currently executing batch.  Sent by `conflict_dialog` when the user
    /// enables "Apply to all".
    SetConflictPolicy(crate::ui::conflict_policy::ConflictPolicy),
    /// Open the transfer progress dialog (idempotent - no-op if already open
    /// or if the queue drained before the message was processed).
    ShowTransferDialog,
    /// Show the dialog only when task `id` is still active.
    ///
    /// Dispatched by the 2-second delay timer in `paste_ops.rs`, prevents
    /// opening an empty dialog when a fast copy already completed.
    ShowTransferDialogIfActive(u64),
    /// The transfer dialog window was closed (by the user or because the
    /// queue drained).  Clears `FluxApp::transfer_dialog`.
    TransferDialogClosed,
}
