use adw::prelude::*;
use std::cell::Cell;
use std::rc::Rc;
use tokio::sync::oneshot;

use crate::model::AppMsg;
use crate::ui::conflict_policy::{auto_rename_dest, ConflictChoice, ConflictContext};
use relm4::prelude::*;

// Response IDs as gtk::ResponseType
const RESP_CANCEL: gtk::ResponseType = gtk::ResponseType::Cancel;
const RESP_SKIP: gtk::ResponseType = gtk::ResponseType::Other(1);
const RESP_RENAME: gtk::ResponseType = gtk::ResponseType::Other(2);
const RESP_REPLACE: gtk::ResponseType = gtk::ResponseType::Other(3);

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
    let extra = build_extra_child(&ctx, &file_name, &op_word);
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
    let tx_cell: Rc<Cell<Option<oneshot::Sender<ConflictChoice>>>> = Rc::new(Cell::new(Some(tx)));

    let apply_all_check = extra
        .last_child()
        .and_then(|w| w.downcast::<gtk::CheckButton>().ok());

    let s = sender.clone();

    dialog.connect_response(move |dlg, response_id| {
        dlg.close();

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

        if apply_all_active {
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

// ─── Extra child builder ──────────────────────────────────────────────────────

fn build_extra_child(ctx: &ConflictContext, file_name: &str, op_word: &str) -> gtk::Box {
    let root = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(0)
        .css_classes(["conflict-extra"])
        .build();

    // File comparison card
    let card = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(0)
        .css_classes(["conflict-card"])
        .hexpand(true)
        .build();

    let src_side = build_file_side(&ctx.src, file_name, &format!("{} this", op_word), false);

    let arrow = gtk::Label::builder()
        .label("→")
        .css_classes(["conflict-arrow"])
        .valign(gtk::Align::Center)
        .build();

    let dest_name = ctx
        .dest
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    let dest_side = build_file_side(
        &ctx.dest,
        &dest_name,
        &crate::i18n::tr("Existing file"),
        true,
    );

    card.append(&src_side);
    card.append(&arrow);
    card.append(&dest_side);
    root.append(&card);

    // Auto-rename hint
    let rename_dest = auto_rename_dest(&ctx.dest);
    let rename_name = rename_dest
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();

    let rename_row = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(8)
        .css_classes(["conflict-rename-hint"])
        .build();

    let rename_icon = gtk::Image::builder()
        .icon_name("edit-symbolic")
        .pixel_size(14)
        .css_classes(["dim-label"])
        .build();

    let rename_label = gtk::Label::builder()
        .label(&format!(
            "{}: \"{}\"",
            crate::i18n::tr("Auto-rename will save as"),
            rename_name
        ))
        .css_classes(["caption", "dim-label"])
        .xalign(0.0)
        .wrap(true)
        .hexpand(true)
        .build();

    rename_row.append(&rename_icon);
    rename_row.append(&rename_label);
    root.append(&rename_row);

    // Separator
    let sep = gtk::Separator::builder()
        .orientation(gtk::Orientation::Horizontal)
        .css_classes(["conflict-sep"])
        .build();
    root.append(&sep);

    // "Apply to all" checkbox
    let apply_all = gtk::CheckButton::builder()
        .label(crate::i18n::tr("Apply to all remaining conflicts"))
        .css_classes(["conflict-apply-all"])
        .visible(ctx.batch_total > 1)
        .build();
    root.append(&apply_all);

    root
}

// ─── File side builder ────────────────────────────────────────────────────────

fn build_file_side(
    path: &std::path::Path,
    display_name: &str,
    role_label: &str,
    is_existing: bool,
) -> gtk::Box {
    let side = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(4)
        .hexpand(true)
        .css_classes(if is_existing {
            vec!["conflict-file-side", "conflict-file-existing"]
        } else {
            vec!["conflict-file-side", "conflict-file-incoming"]
        })
        .build();

    // Role label
    let role = gtk::Label::builder()
        .label(role_label)
        .css_classes(["caption", "dim-label"])
        .xalign(0.0)
        .build();
    side.append(&role);

    // File icon
    let icon_name = resolve_icon_name(path);
    let icon = gtk::Image::builder()
        .icon_name(&icon_name)
        .pixel_size(48)
        .css_classes(["conflict-file-icon"])
        .build();
    side.append(&icon);

    // File name
    let name_label = gtk::Label::builder()
        .label(display_name)
        .css_classes(["conflict-file-name"])
        .xalign(0.0)
        .max_width_chars(22)
        .ellipsize(gtk::pango::EllipsizeMode::Middle)
        .wrap(false)
        .build();
    side.append(&name_label);

    // Parent directory
    let parent_str = path
        .parent()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default();
    let dir_label = gtk::Label::builder()
        .label(&parent_str)
        .css_classes(["caption", "dim-label", "conflict-file-dir"])
        .xalign(0.0)
        .max_width_chars(22)
        .ellipsize(gtk::pango::EllipsizeMode::Start)
        .wrap(false)
        .build();
    side.append(&dir_label);

    // Size + mtime
    if let Ok(meta) = std::fs::metadata(path) {
        let size_str = format_size(meta.len());
        let mtime_str = format_mtime(&meta);

        let meta_label = gtk::Label::builder()
            .label(&format!("{} · {}", size_str, mtime_str))
            .css_classes(["caption", "dim-label"])
            .xalign(0.0)
            .build();
        side.append(&meta_label);
    }

    side
}

// ─── Utility helpers ──────────────────────────────────────────────────────────

fn resolve_icon_name(path: &std::path::Path) -> String {
    let gio_file = gtk::gio::File::for_path(path);
    if let Ok(info) = gio_file.query_info(
        "standard::icon",
        gtk::gio::FileQueryInfoFlags::NONE,
        gtk::gio::Cancellable::NONE,
    ) {
        if let Some(icon) = info.icon() {
            if let Some(themed) = icon.downcast_ref::<gtk::gio::ThemedIcon>() {
                if let Some(name) = themed.names().first() {
                    return name.to_string();
                }
            }
        }
    }

    if path.is_dir() {
        return "folder-symbolic".to_string();
    }
    match path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase()
        .as_str()
    {
        "png" | "jpg" | "jpeg" | "webp" | "gif" | "svg" | "avif" => "image-x-generic-symbolic",
        "mp4" | "mkv" | "avi" | "mov" | "webm" => "video-x-generic-symbolic",
        "mp3" | "flac" | "ogg" | "wav" | "aac" => "audio-x-generic-symbolic",
        "pdf" => "x-office-document-symbolic",
        "zip" | "tar" | "gz" | "xz" | "bz2" | "7z" | "rar" => "package-x-generic-symbolic",
        "rs" | "py" | "js" | "ts" | "c" | "cpp" | "h" | "go" | "sh" => "text-x-script-symbolic",
        _ => "text-x-generic-symbolic",
    }
    .to_string()
}

fn format_size(bytes: u64) -> String {
    const KB: u64 = 1_024;
    const MB: u64 = 1_024 * KB;
    const GB: u64 = 1_024 * MB;

    if bytes >= GB {
        format!("{:.1} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.0} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

fn format_mtime(meta: &std::fs::Metadata) -> String {
    use std::time::SystemTime;

    let Ok(modified) = meta.modified() else {
        return String::new();
    };
    let Ok(elapsed) = SystemTime::now().duration_since(modified) else {
        return crate::i18n::tr("just now");
    };

    let secs = elapsed.as_secs();
    if secs < 60 {
        crate::i18n::tr("just now")
    } else if secs < 3_600 {
        let m = secs / 60;
        if m == 1 {
            crate::i18n::tr("1 minute ago")
        } else {
            format!("{} {}", m, crate::i18n::tr("minutes ago"))
        }
    } else if secs < 86_400 {
        let h = secs / 3_600;
        if h == 1 {
            crate::i18n::tr("1 hour ago")
        } else {
            format!("{} {}", h, crate::i18n::tr("hours ago"))
        }
    } else {
        let d = secs / 86_400;
        if d == 1 {
            crate::i18n::tr("Yesterday")
        } else if d < 30 {
            format!("{} {}", d, crate::i18n::tr("days ago"))
        } else {
            let secs_since_epoch = modified
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);
            let days = secs_since_epoch / 86_400;
            let (y, m, d) = days_to_ymd(days);
            format!("{:04}-{:02}-{:02}", y, m, d)
        }
    }
}

fn days_to_ymd(days: u64) -> (u64, u64, u64) {
    let jd = days + 2_440_588;
    let l = jd + 68_569;
    let n = 4 * l / 146_097;
    let l = l - (146_097 * n + 3) / 4;
    let i = 4_000 * (l + 1) / 1_461_001;
    let l = l - 1_461 * i / 4 + 31;
    let j = 80 * l / 2_447;
    let d = l - 2_447 * j / 80;
    let l = j / 11;
    let m = j + 2 - 12 * l;
    let y = 100 * (n - 49) + i + l;
    (y, m, d)
}
