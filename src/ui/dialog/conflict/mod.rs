use adw::prelude::*;
use std::cell::Cell;
use std::rc::Rc;
use tokio::sync::oneshot;

use crate::domain::model::{AppMsg, ConflictChoice, ConflictContext, TransferMsg};
use relm4::prelude::*;

mod card;
mod util;

use card::build_extra_child;

const RESP_CANCEL: &str = "cancel";
const RESP_SKIP: &str = "skip";
const RESP_RENAME: &str = "rename";
const RESP_REPLACE: &str = "replace";

pub fn show_conflict_dialog(
    ctx: ConflictContext,
    tx: oneshot::Sender<(ConflictChoice, bool)>,
    sender: AsyncComponentSender<crate::app::state::FluxApp>,
) {
    crate::hit!("show_conflict_dialog");
    let window = gtk::Application::default().active_window();

    let file_name = ctx
        .dest
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();

    let op_word = if ctx.is_cut {
        crate::i18n::tr("move")
    } else {
        crate::i18n::tr("copy")
    };

    let heading = crate::i18n::tr("Replace existing file?");

    let body = if ctx.batch_total > 1 {
        format!(
            "{} ({} of {}) - {}",
            crate::i18n::tr("Conflict"),
            ctx.batch_index,
            ctx.batch_total,
            crate::i18n::tr("A file with this name already exists. Choose what to do with it.")
        )
    } else {
        crate::i18n::tr(
            "A file with this name already exists in the destination. \
             Choose what to do with it.",
        )
    };

    let dialog = adw::AlertDialog::new(Some(&heading), Some(&body));

    dialog.add_response(RESP_CANCEL, &crate::i18n::tr("Cancel"));
    dialog.add_response(RESP_SKIP, &crate::i18n::tr("Skip"));
    dialog.add_response(RESP_RENAME, &crate::i18n::tr("Auto-Rename"));
    dialog.add_response(RESP_REPLACE, &crate::i18n::tr("Replace"));

    // Style buttons: Replace → destructive, Auto‑Rename → suggested
    dialog.set_response_appearance(RESP_REPLACE, adw::ResponseAppearance::Destructive);
    dialog.set_response_appearance(RESP_RENAME, adw::ResponseAppearance::Suggested);

    dialog.set_default_response(Some(RESP_SKIP));
    dialog.set_close_response(RESP_CANCEL);

    let (extra, apply_all) = build_extra_child(&ctx, &file_name, &op_word);
    dialog.set_extra_child(Some(&extra));

    #[allow(clippy::type_complexity)]
    let tx_cell: Rc<Cell<Option<oneshot::Sender<(ConflictChoice, bool)>>>> =
        Rc::new(Cell::new(Some(tx)));

    let apply_all_check = apply_all;
    let s = sender.clone();

    dialog.connect_response(None, move |_, response_id| {
        let choice = match response_id {
            RESP_REPLACE => ConflictChoice::Replace,
            RESP_SKIP => ConflictChoice::Skip,
            RESP_RENAME => ConflictChoice::AutoRename,
            _ => ConflictChoice::Cancel,
        };

        let apply_all_active = apply_all_check
            .as_ref()
            .map(|c| c.is_active())
            .unwrap_or(false);

        s.input(AppMsg::Transfer(TransferMsg::ConflictDialogClosed));

        if let Some(tx) = tx_cell.take() {
            let _ = tx.send((choice, apply_all_active));
        }
    });

    dialog.present(window.as_ref());
}
