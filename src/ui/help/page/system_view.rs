use super::{group, page, row};
use crate::i18n::tr;
use crate::model::Config;
use crate::ui::help::format::sc;

pub fn build(config: &Config) -> adw::PreferencesPage {
    let g = group(
        &tr("System & View Shortcuts"),
        vec![
            row(
                &tr("Rename selected item"),
                &sc(config, |s| s.rename.clone(), "F2"),
            ),
            row(
                &tr("Toggle folders first in current directory"),
                &sc(config, |s| s.toggle_folders_first.clone(), "F3"),
            ),
            row(&tr("Toggle embedded terminal"), "F4"),
            row(
                &tr("Refresh current directory"),
                &sc(config, |s| s.refresh.clone(), "F5"),
            ),
            row(
                &tr("Toggle header bar"),
                &sc(config, |s| s.toggle_header.clone(), "F6"),
            ),
            row(&tr("Toggle status bar"), "F7"),
            row(&tr("Toggle sidebar"), "F8"),
            row(&tr("Resize grid items"), "Ctrl + Scroll"),
            row(&tr("Open folder in new tab"), "Middle Click"),
            row(&tr("Open folder in new window"), "Ctrl + Middle Click"),
            row(
                &tr("Toggle hidden files"),
                &sc(config, |s| s.toggle_hidden.clone(), "Ctrl + H"),
            ),
            row(
                &tr("Move selected items to trash"),
                &sc(config, |s| s.delete.clone(), "Delete"),
            ),
            row(&tr("Undo file operation"), "Ctrl + Z"),
            row(&tr("Redo file operation"), "Ctrl + Shift + Z / Ctrl + Y"),
            row(
                &tr("Copy absolute path of selected items"),
                &sc(config, |s| s.copy_path.clone(), "Ctrl + Shift + C"),
            ),
            row(
                &tr("Create symbolic link from clipboard paths"),
                &sc(config, |s| s.create_symlink.clone(), "Ctrl + Shift + V"),
            ),
            row(
                &tr("Create hard link from clipboard paths"),
                &sc(
                    config,
                    |s| s.create_hardlink.clone(),
                    "Ctrl + Alt + Shift + V",
                ),
            ),
            row(&tr("Open memory debug profiler"), "Ctrl + Shift + F7"),
        ],
    );

    page(&tr("System & View"), "view-grid-symbolic", vec![g])
}
