use super::{group, page, row};
use crate::i18n::tr;
use crate::model::Config;
use crate::ui::help::format::sc;

pub fn build(config: &Config) -> adw::PreferencesPage {
    let g = group(
        &tr("Application Shortcuts"),
        vec![
            row(
                &tr("Open context menu editor"),
                &sc(config, |s| s.menu_editor.clone(), "F9"),
            ),
            row(
                &tr("Open preferences"),
                &sc(config, |s| s.settings.clone(), "F10"),
            ),
            row(
                &tr("Cycle through sorting modes"),
                &sc(config, |s| s.cycle_sort.clone(), "Ctrl + S"),
            ),
            row(
                &tr("Toggle ascending/descending sort order"),
                &sc(config, |s| s.toggle_sort_order.clone(), "Ctrl + Shift + S"),
            ),
        ],
    );

    page(
        &tr("Application"),
        "application-default-icon-symbolic",
        vec![g],
    )
}
