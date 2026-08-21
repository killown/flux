use adw::prelude::*;
use std::cell::Cell;
use std::rc::Rc;
use tokio::sync::oneshot;

use crate::model::AppMsg;
use crate::ui::conflict_policy::{auto_rename_dest, ConflictChoice, ConflictContext};
use relm4::prelude::*;

pub fn show_conflict_dialog(
    ctx: ConflictContext,
    tx: oneshot::Sender<ConflictChoice>,
    sender: AsyncComponentSender<crate::model::FluxApp>,
) {
    let window = gtk::Application::default().active_window();

    let file_name = ctx
        .dest
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();

    let op_label = if ctx.is_cut {
        crate::i18n::tr("Moving")
    } else {
        crate::i18n::tr("Copying")
    };
    let title = crate::i18n::tr("Replace \"{}\"?").replace("{}", &file_name);

    let subtitle = if ctx.batch_total > 1 {
        crate::i18n::tr(
            "File {} of {} already exists in the destination folder. Choose what to do:",
        )
        .replace("{}", &ctx.batch_index.to_string())
        .replace("{}", &ctx.batch_total.to_string())
    } else {
        crate::i18n::tr("A file with the same name already exists in the destination folder. Choose what to do:")
            .to_string()
    };

    let apply_all = gtk::CheckButton::builder()
        .label(crate::i18n::tr(
            "Apply this action to all remaining conflicts",
        ))
        .margin_top(6)
        .build();

    let body_box = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(8)
        .margin_top(4)
        .margin_bottom(4)
        .build();

    let subtitle_label = gtk::Label::builder()
        .label(&subtitle)
        .wrap(true)
        .xalign(0.0)
        .build();

    let rename_dest = auto_rename_dest(&ctx.dest);
    let rename_name = rename_dest
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    let rename_hint = gtk::Label::builder()
        .label(crate::i18n::tr("Auto-rename will save as: \"{}\"").replace("{}", &rename_name))
        .css_classes(["dim-label", "caption"])
        .xalign(0.0)
        .wrap(true)
        .build();

    body_box.append(&subtitle_label);
    body_box.append(&rename_hint);
    body_box.append(&apply_all);

    let dialog = gtk::MessageDialog::new(
        window.as_ref(),
        gtk::DialogFlags::MODAL | gtk::DialogFlags::DESTROY_WITH_PARENT,
        gtk::MessageType::Warning,
        gtk::ButtonsType::None,
        &title,
    );
    dialog.set_secondary_text(Some(&format!("{} - {}", op_label, file_name)));

    if let Some(content_area) = dialog
        .content_area()
        .first_child()
        .and_then(|w| w.downcast::<gtk::Box>().ok())
    {
        content_area.append(&body_box);
    } else {
        dialog.content_area().append(&body_box);
    }

    dialog.add_button(&crate::i18n::tr("Cancel"), gtk::ResponseType::Cancel);
    dialog.add_button(&crate::i18n::tr("Skip"), gtk::ResponseType::Other(1));
    dialog.add_button(&crate::i18n::tr("Auto-Rename"), gtk::ResponseType::Other(2));

    let replace_btn = dialog.add_button(&crate::i18n::tr("Replace"), gtk::ResponseType::Accept);
    replace_btn.style_context().add_class("destructive-action");

    let tx_cell: Rc<Cell<Option<oneshot::Sender<ConflictChoice>>>> = Rc::new(Cell::new(Some(tx)));

    let apply_all_ref = apply_all.clone();
    let s = sender.clone();

    dialog.connect_response(move |dlg, response| {
        dlg.close();

        let choice = match response {
            gtk::ResponseType::Accept => ConflictChoice::Replace,
            gtk::ResponseType::Other(1) => ConflictChoice::Skip,
            gtk::ResponseType::Other(2) => ConflictChoice::AutoRename,
            _ => ConflictChoice::Cancel,
        };

        if apply_all_ref.is_active() {
            let policy = match &choice {
                ConflictChoice::Replace => crate::ui::conflict_policy::ConflictPolicy::ReplaceAll,
                ConflictChoice::Skip => crate::ui::conflict_policy::ConflictPolicy::SkipAll,
                ConflictChoice::AutoRename => {
                    crate::ui::conflict_policy::ConflictPolicy::AutoRenameAll
                }
                ConflictChoice::Cancel => crate::ui::conflict_policy::ConflictPolicy::Ask,
            };
            s.input(AppMsg::SetConflictPolicy(policy));
        }

        s.input(AppMsg::ConflictDialogClosed);

        if let Some(tx) = tx_cell.take() {
            let _ = tx.send(choice);
        }
    });

    dialog.present();
}
