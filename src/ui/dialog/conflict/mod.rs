use adw::prelude::*;
use std::cell::Cell;
use std::rc::Rc;
use tokio::sync::oneshot;

use crate::model::AppMsg;
use crate::ui::conflict_policy::{ConflictChoice, ConflictContext};
use relm4::prelude::*;

mod card;
mod util;

use card::build_extra_child;

// Standard Response IDs
const RESP_CANCEL: gtk::ResponseType = gtk::ResponseType::Cancel;
const RESP_SKIP: gtk::ResponseType = gtk::ResponseType::Reject;
const RESP_RENAME: gtk::ResponseType = gtk::ResponseType::Apply;
const RESP_REPLACE: gtk::ResponseType = gtk::ResponseType::Accept;

pub fn show_conflict_dialog(
    ctx: ConflictContext,
    tx: oneshot::Sender<(ConflictChoice, bool)>,
    sender: AsyncComponentSender<crate::model::FluxApp>,
) {
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

    // Heading & body
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

    // Create dialog
    let dialog = gtk::MessageDialog::new(
        window.as_ref(),
        gtk::DialogFlags::MODAL | gtk::DialogFlags::DESTROY_WITH_PARENT,
        gtk::MessageType::Question,
        gtk::ButtonsType::None,
        &heading,
    );
    dialog.set_secondary_text(Some(&body));

    if let Some(ref win) = window {
        dialog.set_transient_for(Some(win));
    }

    // Add buttons in desired order
    dialog.add_button(&crate::i18n::tr("Cancel"), RESP_CANCEL);
    dialog.add_button(&crate::i18n::tr("Skip"), RESP_SKIP);
    dialog.add_button(&crate::i18n::tr("Auto-Rename"), RESP_RENAME);
    dialog.add_button(&crate::i18n::tr("Replace"), RESP_REPLACE);

    // Style buttons: Replace → destructive, Auto‑Rename → suggested
    if let Some(btn) = dialog.widget_for_response(RESP_REPLACE) {
        btn.style_context().add_class("destructive-action");
    }
    if let Some(btn) = dialog.widget_for_response(RESP_RENAME) {
        btn.style_context().add_class("suggested-action");
    }

    // Default response
    dialog.set_default_response(RESP_SKIP);

    // ── Extra child: file preview card ──────────────────────────────────────
    let (extra, apply_all) = build_extra_child(&ctx, &file_name, &op_word);
    if let Some(content_area) = dialog
        .content_area()
        .first_child()
        .and_then(|w| w.downcast::<gtk::Box>().ok())
    {
        content_area.append(&extra);
    } else {
        dialog.content_area().append(&extra);
    }

    // ── Response handler ────────────────────────────────────────────────────
    #[allow(clippy::type_complexity)]
    let tx_cell: Rc<Cell<Option<oneshot::Sender<(ConflictChoice, bool)>>>> =
        Rc::new(Cell::new(Some(tx)));

    let apply_all_check = apply_all;
    let s = sender.clone();

    dialog.connect_response(move |dlg, response_id| {
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

        s.input(AppMsg::ConflictDialogClosed);

        if let Some(tx) = tx_cell.take() {
            let _ = tx.send((choice, apply_all_active));
        }

        dlg.close();
    });

    dialog.present();
}
