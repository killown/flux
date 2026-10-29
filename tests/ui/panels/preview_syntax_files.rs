//! Syntax lookup for files the extension alone cannot identify.

use flux::ui::panels::preview::syntax::highlight_file;
use std::path::Path;

#[test]
fn shebang_scripts_without_an_extension_are_highlighted() {
    let (ranges, language) = highlight_file("#!/bin/sh\necho hi\n", Path::new("deploy"));
    assert!(language.is_some());
    assert!(!ranges.is_empty());
}

#[test]
fn well_known_file_names_are_highlighted() {
    let (ranges, language) = highlight_file("all:\n\techo hi\n", Path::new("Makefile"));
    assert!(language.is_some());
    assert!(!ranges.is_empty());
}

#[test]
fn typescript_borrows_the_javascript_syntax() {
    let src = "const answer: number = 42; // done\n";
    let (ranges, language) = highlight_file(src, Path::new("main.ts"));
    assert_eq!(language.as_deref(), Some("JavaScript"));
    assert!(!ranges.is_empty());
}

#[test]
fn unknown_plain_files_stay_unhighlighted() {
    let (ranges, language) = highlight_file("hello\n", Path::new("notes.unknownext"));
    assert!(ranges.is_empty());
    assert!(language.is_none());
}
