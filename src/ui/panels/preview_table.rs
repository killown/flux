//! CSV / TSV viewer for the peek-preview panel.
//!
//! Parses with the `csv` crate (correct quoting, embedded delimiters,
//! escaped quotes) and renders with `gtk::ColumnView` + `gio::ListStore`
//! so 100k-row files virtualise correctly. Row/column counts are capped so
//! a pathological file cannot allocate unbounded memory before the first
//! paint.

use csv::ReaderBuilder;
use gtk::gio;
use gtk::prelude::*;
use std::path::Path;

#[derive(Debug, Clone)]
pub struct TableData {
    pub headers: Vec<String>,
    pub rows: Vec<Vec<String>>,
    pub truncated: bool,
    pub delimiter: u8,
}

const MAX_ROWS: usize = 10_000;
const MAX_COLS: usize = 256;

pub fn parse(path: &Path, ext: &str) -> Option<TableData> {
    crate::hit!("parse");
    let delimiter = match ext {
        "tsv" | "tab" => b'\t',
        _ => b',',
    };

    let file = std::fs::File::open(path).ok()?;
    let mut rdr = ReaderBuilder::new()
        .delimiter(delimiter)
        .has_headers(false)
        .flexible(true)
        .from_reader(std::io::BufReader::new(file));

    let mut headers: Vec<String> = Vec::new();
    let mut rows: Vec<Vec<String>> = Vec::new();
    let mut truncated = false;

    let mut record = csv::StringRecord::new();
    let mut is_first = true;

    loop {
        match rdr.read_record(&mut record) {
            Ok(true) => {}
            Ok(false) => break,
            Err(_) => {
                truncated = true;
                break;
            }
        }

        let mut cols: Vec<String> = record.iter().map(|s| s.to_string()).collect();
        if cols.len() > MAX_COLS {
            cols.truncate(MAX_COLS);
            truncated = true;
        }

        if is_first {
            is_first = false;
            headers = cols;
            continue;
        }

        rows.push(cols);
        if rows.len() >= MAX_ROWS {
            truncated = true;
            break;
        }
    }

    if headers.is_empty() {
        return None;
    }

    // Pad headers to the widest row so ColumnView has a consistent schema.
    let width = headers
        .len()
        .max(rows.iter().map(|r| r.len()).max().unwrap_or(0));
    headers.resize(width, String::new());
    for r in &mut rows {
        r.resize(width, String::new());
    }

    Some(TableData {
        headers,
        rows,
        truncated,
        delimiter,
    })
}

pub fn build_store(data: &TableData) -> gio::ListStore {
    crate::hit!("build_store");
    let outer = gio::ListStore::new::<gio::ListStore>();

    // Row 0 is the header, rendered through the same factory as body rows.
    let header_row = gio::ListStore::new::<gtk::StringObject>();
    for h in &data.headers {
        header_row.append(&gtk::StringObject::new(h));
    }
    outer.append(&header_row);

    for row in &data.rows {
        let inner = gio::ListStore::new::<gtk::StringObject>();
        for cell in row {
            inner.append(&gtk::StringObject::new(cell));
        }
        outer.append(&inner);
    }
    outer
}

pub fn build_columns(view: &gtk::ColumnView, data: &TableData) {
    crate::hit!("build_columns");
    while let Some(col) = view
        .columns()
        .item(0)
        .and_downcast::<gtk::ColumnViewColumn>()
    {
        view.remove_column(&col);
    }

    let sample_rows = data.rows.len().min(200);
    let char_w = 9.5_f64;

    for (idx, header) in data.headers.iter().enumerate() {
        let mut longest = header.chars().count();
        for row in data.rows.iter().take(sample_rows) {
            if let Some(cell) = row.get(idx) {
                longest = longest.max(cell.chars().count());
            }
        }
        let chars = longest.clamp(8, 40);
        let width = (chars as f64 * char_w + 16.0).round() as i32;

        let factory = gtk::SignalListItemFactory::new();
        factory.connect_setup(|_, list_item| {
            let item = list_item.downcast_ref::<gtk::ListItem>().unwrap();
            let lbl = gtk::Label::builder()
                .xalign(0.5)
                .ellipsize(gtk::pango::EllipsizeMode::End)
                .build();
            lbl.add_css_class("flux-table-cell");
            item.set_child(Some(&lbl));
        });
        factory.connect_bind(move |_, list_item| {
            let item = list_item.downcast_ref::<gtk::ListItem>().unwrap();
            let row_store = item.item().and_downcast::<gio::ListStore>();
            let lbl = item.child().and_downcast::<gtk::Label>();
            if let (Some(store), Some(lbl)) = (row_store, lbl) {
                // Row 0 of the store is the header row. Style it so it reads
                // as a header rather than as data.
                if item.position() == 0 {
                    lbl.add_css_class("flux-table-header");
                } else {
                    lbl.remove_css_class("flux-table-header");
                }

                // Pin the width on every bind so GTK's cell recycling
                // cannot carry a previous column's width into this one.
                lbl.set_width_chars(chars as i32);
                lbl.set_max_width_chars(chars as i32);
                if let Some(obj) = store.item(idx as u32).and_downcast::<gtk::StringObject>() {
                    lbl.set_label(obj.string().as_str());
                } else {
                    lbl.set_label("");
                }
            }
        });

        let column = gtk::ColumnViewColumn::builder()
            .title("")
            .factory(&factory)
            .resizable(true)
            .expand(false)
            .fixed_width(width)
            .build();
        view.append_column(&column);
    }
}
