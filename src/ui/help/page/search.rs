use super::{group, page, row, row_sub};
use crate::domain::model::Config;
use crate::i18n::tr;
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

    let filter_group = group(
        &tr("Quick Filters"),
        vec![
            row(
                &tr("Show only files matching the selected item's MIME type"),
                "Ctrl + M",
            ),
            row(
                &tr("Clear the active MIME filter or close the preview panel"),
                "Esc / Ctrl + M / Space",
            ),
        ],
    );
    filter_group.set_description(Some(&tr(
        "Select a file, then press Space or Ctrl+M to preview it and hide other files. \
        Pressing Space or Esc again closes the preview and removes the filter.",
    )));

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
        vec![search_group, filter_group, content_group],
    )
}
