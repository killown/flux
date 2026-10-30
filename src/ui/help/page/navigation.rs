use super::{group, page, row};
use crate::domain::model::Config;
use crate::i18n::tr;
use crate::ui::help::format::sc;

pub fn build(config: &Config) -> adw::PreferencesPage {
    let nav_group = group(
        &tr("Navigation Shortcuts"),
        vec![
            row(
                &tr("Go back in history"),
                &sc(config, |s| s.back.clone(), "Backspace"),
            ),
            row(
                &tr("Go forward in history"),
                &sc(config, |s| s.forward.clone(), "Alt + Right"),
            ),
            row(
                &tr("Navigate to home directory"),
                &sc(config, |s| s.home.clone(), "Ctrl + Home"),
            ),
            row(
                &tr("Open selected file or directory"),
                &sc(config, |s| s.open.clone(), "Enter"),
            ),
            row(
                &tr("Navigate to root directory"),
                &sc(config, |s| s.root.clone(), "/"),
            ),
            row(&tr("Show/hide location panel"), "Ctrl + L"),
            row(&tr("Show/hide connect to server"), "Ctrl + Shift + L"),
            row(
                &tr("Toggle preview panel and MIME filter"),
                "Ctrl + Space / Space",
            ),
            row(&tr("Open tag navigator"), "Ctrl + Shift + T"),
            row(
                &tr("Cycle through folders starting with a letter"),
                "Ctrl + Alt + <letter>",
            ),
            row(
                &tr("Toggle Navigate mode: type letters to jump to a folder"),
                "Ctrl + Insert",
            ),
            row(
                &tr("Re-anchor the folder cycle at the current directory"),
                "Ctrl + Alt + /",
            ),
            row(
                &tr("Cancel the folder cycle and return to its anchor"),
                "Ctrl + Alt + \\",
            ),
        ],
    );

    let tabs_group = group(
        &tr("Tabs"),
        vec![
            row(&tr("New tab"), "Ctrl + T"),
            row(&tr("Close tab"), "Ctrl + W"),
            row(&tr("Next tab"), "Ctrl + Tab / Shift + L"),
            row(&tr("Previous tab"), "Ctrl + Shift + Tab / Shift + H"),
        ],
    );

    page(
        &tr("Navigation"),
        "compass-symbolic",
        vec![nav_group, tabs_group],
    )
}
