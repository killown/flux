use crate::ui::conflict_policy::{auto_rename_dest, ConflictContext};
use adw::prelude::*;

use super::util::{format_mtime, format_size, resolve_icon_name};

// ─── Extra child builder ──────────────────────────────────────────────────────

pub(super) fn build_extra_child(
    ctx: &ConflictContext,
    file_name: &str,
    op_word: &str,
) -> (gtk::Box, Option<gtk::CheckButton>) {
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
        .label(format!(
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

    (root, Some(apply_all))
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
            .label(format!("{} · {}", size_str, mtime_str))
            .css_classes(["caption", "dim-label"])
            .xalign(0.0)
            .build();
        side.append(&meta_label);
    }

    side
}
