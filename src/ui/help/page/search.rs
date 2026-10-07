use super::{group, page, row, row_sub};
use crate::i18n::tr;
use crate::model::Config;
use crate::ui::help::format::sc;
use adw::prelude::*;

pub fn build(config: &Config) -> adw::PreferencesPage {
    let search_group = group(
        &tr("Search Shortcuts"),
        vec![row(
            &tr("Search files"),
            &sc(config, |s| s.search.clone(), "Ctrl + F"),
        )],
    );

    let content_group = group(
        &tr("Content Search"),
        vec![
            row_sub(
                &tr("Start content search"),
                &tr("Type :term to search all files, or :.ext:term to filter by extension"),
                ":term  or  :.ext:term",
            ),
            row_sub(
                &tr("Cancel content search"),
                &tr("Press Escape while in search view"),
                "Esc",
            ),
        ],
    );
    content_group.set_description(Some(&tr(
        "Search inside file contents (not just filenames)",
    )));

    page(
        &tr("Search"),
        "search-symbolic",
        vec![search_group, content_group],
    )
}
