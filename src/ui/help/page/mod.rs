//! Page builders for the Help window.
//!
//! Each submodule exposes `pub fn build(config: &Config) -> adw::PreferencesPage`.
//! Building pages imperatively (rather than inline in `view!`) keeps the macro
//! tree in `help/mod.rs` small and lets each page live in its own file.

pub mod application;
pub mod navigation;
pub mod quick_list;
pub mod search;
pub mod system_view;

use adw::prelude::*;
use relm4::prelude::*;

/// Build an `adw::ActionRow` with a title and a keycap-style suffix label.
pub fn row(title: &str, keycap: &str) -> adw::ActionRow {
    let row = adw::ActionRow::builder().title(title).build();
    let label = gtk::Label::builder()
        .label(keycap)
        .css_classes(["keycap"])
        .build();
    row.add_suffix(&label);
    row
}

/// Build an `adw::ActionRow` with a title, subtitle, and keycap suffix.
pub fn row_sub(title: &str, subtitle: &str, keycap: &str) -> adw::ActionRow {
    let row = adw::ActionRow::builder()
        .title(title)
        .subtitle(subtitle)
        .build();
    let label = gtk::Label::builder()
        .label(keycap)
        .css_classes(["keycap"])
        .build();
    row.add_suffix(&label);
    row
}

/// Build an `adw::PreferencesGroup` from a title and a list of rows.
pub fn group(title: &str, rows: Vec<adw::ActionRow>) -> adw::PreferencesGroup {
    let group = adw::PreferencesGroup::builder().title(title).build();
    for r in rows {
        group.add(&r);
    }
    group
}

/// Build a page with a title, icon, and one or more groups.
pub fn page(title: &str, icon: &str, groups: Vec<adw::PreferencesGroup>) -> adw::PreferencesPage {
    let page = adw::PreferencesPage::builder()
        .title(title)
        .icon_name(icon)
        .build();
    for g in groups {
        page.add(&g);
    }
    page
}
