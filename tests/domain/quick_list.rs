use flux::model::quick_list::{self as ql, QuickKey, Step};
use gtk::gdk::Key;
use std::path::{Path, PathBuf};

fn p(s: &str) -> PathBuf {
    PathBuf::from(s)
}

fn items(names: &[&str]) -> Vec<PathBuf> {
    names.iter().map(|s| p(s)).collect()
}

fn q(l: &[PathBuf], i: Option<usize>) {
    assert!(ql::invariant_holds(l, i), "Q broken: {l:?} {i:?}");
}

#[test]
fn empty_list_every_key_is_silent() {
    let l: Vec<PathBuf> = vec![];
    let mut i = None;
    assert_eq!(ql::step(&l, &mut i, Step::Next), None);
    assert_eq!(ql::step(&l, &mut i, Step::Prev), None);
    assert_eq!(ql::tab(&l, &mut i, Path::new("/x")), None);
    for n in 1..=9 {
        assert_eq!(ql::jump(&l, &mut i, n), None);
    }
    let mut l2 = l.clone();
    assert!(!ql::clear(&mut l2, &mut i));
    assert!(l2.is_empty() && i.is_none());
}

#[test]
fn one_item_every_key_lands_in_it_same_or_different_cwd() {
    let l = items(&["/a"]);
    for cwd in ["/a", "/elsewhere"] {
        let mut i = Some(0);
        assert_eq!(ql::tab(&l, &mut i, Path::new(cwd)), Some(p("/a")));
        assert_eq!(ql::step(&l, &mut i, Step::Next), Some(p("/a")));
        assert_eq!(ql::step(&l, &mut i, Step::Prev), Some(p("/a")));
        for n in 1..=9 {
            assert_eq!(ql::jump(&l, &mut i, n), Some(p("/a")));
            assert_eq!(i, Some(0));
        }
        q(&l, i);
    }
}

#[test]
fn two_items_wrap_forward_and_backward() {
    let l = items(&["/a", "/b"]);
    let mut i = Some(0);
    assert_eq!(ql::step(&l, &mut i, Step::Next), Some(p("/b")));
    assert_eq!(ql::step(&l, &mut i, Step::Next), Some(p("/a")));
    assert_eq!(ql::step(&l, &mut i, Step::Prev), Some(p("/b")));
    assert_eq!(ql::step(&l, &mut i, Step::Prev), Some(p("/a")));
    q(&l, i);
}

#[test]
fn jump_with_fewer_than_nine_items() {
    let l = items(&["/a", "/b", "/c"]);
    let mut i = Some(0);
    assert_eq!(ql::jump(&l, &mut i, 1), Some(p("/a")));
    assert_eq!(ql::jump(&l, &mut i, 3), Some(p("/c")));
    assert_eq!(i, Some(2));
    for n in 4..=9 {
        assert_eq!(ql::jump(&l, &mut i, n), None);
        assert_eq!(i, Some(2), "a missing slot must not move the cursor");
    }
    q(&l, i);
}

#[test]
fn insert_puts_newest_first_and_keeps_cursor_on_same_folder() {
    let mut l = vec![];
    let mut i = None;
    assert_eq!(ql::add_front(&mut l, &mut i, [p("/a")]), 1);
    assert_eq!((l.clone(), i), (items(&["/a"]), Some(0)));
    assert_eq!(ql::add_front(&mut l, &mut i, [p("/b")]), 1);
    assert_eq!(l, items(&["/b", "/a"]));
    assert_eq!(l[i.unwrap()], p("/a"));
    assert_eq!(ql::add_front(&mut l, &mut i, [p("/a")]), 0, "dedupe");
    assert_eq!(ql::add_front(&mut l, &mut i, [p("/x"), p("/y")]), 2);
    assert_eq!(l, items(&["/x", "/y", "/b", "/a"]));
    ql::sync_to(&l, &mut i, Path::new("/x"));
    assert_eq!(i, Some(0));
    q(&l, i);
}

#[test]
fn clear_empty_and_non_empty() {
    let mut l = items(&["/a", "/b"]);
    let mut i = Some(1);
    assert!(ql::clear(&mut l, &mut i));
    assert!(l.is_empty() && i.is_none());
    assert!(!ql::clear(&mut l, &mut i));
    q(&l, i);
}

#[test]
fn tab_goes_to_current_slot_then_advances_from_it() {
    let l = items(&["/a", "/b", "/c"]);
    let mut i = Some(2);
    assert_eq!(ql::tab(&l, &mut i, Path::new("/home")), Some(p("/c")));
    assert_eq!(i, Some(2));
    assert_eq!(ql::tab(&l, &mut i, Path::new("/c")), Some(p("/a")));
    assert_eq!(i, Some(0));
}

#[test]
fn restore_clamps() {
    let l = items(&["/a", "/b"]);
    assert_eq!(ql::restore(&l, Some(1)), Some(1));
    assert_eq!(ql::restore(&l, Some(9)), Some(0));
    assert_eq!(ql::restore(&l, None), Some(0));
    assert_eq!(ql::restore(&[], Some(1)), None);
}

#[test]
fn classify_table() {
    use QuickKey::*;
    let c = ql::classify_key;
    assert_eq!(c(Key::Insert, false, false, false), Some(AddCurrent));
    assert_eq!(c(Key::End, true, false, false), Some(Clear));
    assert_eq!(
        c(Key::End, true, true, false),
        None,
        "Ctrl+Shift+End selects"
    );
    assert_eq!(c(Key::End, false, false, false), None);
    assert_eq!(c(Key::Page_Up, true, false, false), Some(Prev));
    assert_eq!(c(Key::Page_Down, true, false, false), Some(Next));
    assert_eq!(c(Key::Tab, false, false, false), Some(Tab));
    assert_eq!(
        c(Key::Tab, true, false, false),
        None,
        "Ctrl+Tab switches tabs"
    );
    assert_eq!(c(Key::Insert, true, false, false), None, "Navigate mode");
    assert_eq!(c(Key::_1, true, false, false), Some(Jump(1)));
    assert_eq!(c(Key::_9, true, false, false), Some(Jump(9)));
    assert_eq!(c(Key::_1, true, true, false), None);
    assert_eq!(c(Key::_1, true, false, true), None);
    assert_eq!(c(Key::_1, false, false, false), None);
    assert_eq!(c(Key::_0, true, false, false), None);
    assert_eq!(c(Key::Q, true, true, false), Some(TogglePanel));
    assert_eq!(c(Key::q, true, true, false), Some(TogglePanel));
    assert_eq!(c(Key::q, true, false, false), None, "Ctrl+Q quits");
    assert_eq!(c(Key::Q, true, true, true), None);
}

#[test]
fn terminal_focus_yields_every_quick_list_key() {
    let keys = [
        (Key::Insert, false),
        (Key::End, true),
        (Key::Page_Up, true),
        (Key::Page_Down, true),
        (Key::Tab, false),
        (Key::_1, true),
        (Key::_5, true),
        (Key::_9, true),
    ];
    for (k, ctrl) in keys {
        assert!(
            ql::dispatch_key(k, ctrl, false, false, false).is_some(),
            "{k:?}"
        );
        assert_eq!(ql::dispatch_key(k, ctrl, false, false, true), None, "{k:?}");
    }
}
