use super::{group, page, row};
use crate::i18n::tr;
use crate::model::Config;

pub fn build(_config: &Config) -> adw::PreferencesPage {
    let g = group(
        &tr("Quick List Shortcuts"),
        vec![
            row(&tr("Add selection or current folder to list"), "Insert"),
            row(
                &tr("Pin selection or current folder to sidebar permanently"),
                "Ctrl + Insert",
            ),
            row(&tr("Cycle to the next folder in the list"), "Tab"),
            row(&tr("Clear the entire list"), "Ctrl + End"),
        ],
    );

    page(&tr("Quick List"), "view-list-symbolic", vec![g])
}
