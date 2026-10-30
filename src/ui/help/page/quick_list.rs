use super::{group, page, row};
use crate::domain::model::Config;
use crate::i18n::tr;

pub fn build(_config: &Config) -> adw::PreferencesPage {
    let g = group(
        &tr("Quick List Shortcuts"),
        vec![
            row(&tr("Add current folder to list"), "Insert"),
            row(
                &tr("Pin selection or current folder to sidebar permanently"),
                "Ctrl + Shift + Insert",
            ),
            row(&tr("Cycle to the next folder in the selected list"), "Tab"),
            row(
                &tr("Previous / next folder in the selected list"),
                "Ctrl + PageUp / PageDown",
            ),
            row(
                &tr("Jump to folder 1-9 of the selected list"),
                "Ctrl + 1 … 9",
            ),
            row(&tr("Show or hide the Quick List panel"), "Ctrl + Shift + Q"),
            row(&tr("Clear the selected list"), "Ctrl + End"),
        ],
    );

    page(&tr("Quick List"), "view-list-symbolic", vec![g])
}
