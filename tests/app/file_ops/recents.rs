//! `FluxApp::remove_recents` against a sandboxed `recently-used.xbel`.
//!
//! GLib writes `href` XML-escaped (`&` as `&amp,`, `'` as `&apos,`), so the
//! fixtures below hold the escaped form exactly as GLib stores it.

use flux::app::state::FluxApp;
use std::ffi::OsString;
use std::path::PathBuf;
use std::sync::Mutex;

/// `remove_recents` finds the file through `XDG_DATA_HOME`, which is
/// process-global, so every test here runs under this lock.
static ENV_LOCK: Mutex<()> = Mutex::new(());

const AMP_QUOTE_PATH: &str = r#"/home/me/Tom & Jerry's "q".txt"#;
const AMP_QUOTE_HREF: &str = "file:///home/me/Tom%20&amp;%20Jerry&apos;s%20%22q%22.txt";

const APOSTROPHE_PATH: &str = "/home/me/Don't Stop.mp3";
const APOSTROPHE_HREF: &str = "file:///home/me/Don&apos;t%20Stop.mp3";

const FOLDER_PATH: &str = "/home/me/R&D";
const FOLDER_HREF: &str = "file:///home/me/R&amp;D";

const INSIDE_FOLDER_HREF: &str = "file:///home/me/R&amp;D/notes.txt";

const PLAIN_PATH: &str = "/home/me/plain.txt";
const PLAIN_HREF: &str = "file:///home/me/plain.txt";

/// Points `XDG_DATA_HOME` at a temp dir and restores it on drop.
struct Sandbox {
    _tmp: tempfile::TempDir,
    xbel: PathBuf,
    previous: Option<OsString>,
}

impl Sandbox {
    fn new() -> Self {
        let tmp = tempfile::tempdir().unwrap();
        let previous = std::env::var_os("XDG_DATA_HOME");
        std::env::set_var("XDG_DATA_HOME", tmp.path());
        let xbel = tmp.path().join("recently-used.xbel");
        Sandbox {
            _tmp: tmp,
            xbel,
            previous,
        }
    }

    fn write(&self, hrefs: &[&str]) {
        let mut xml = String::from(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
             <xbel version=\"1.0\"\n      \
             xmlns:bookmark=\"http://www.freedesktop.org/standard/desktop-bookmarks\"\n      \
             xmlns:mime=\"http://www.freedesktop.org/standard/shared-mime-info\"\n>\n",
        );
        for href in hrefs {
            xml.push_str(&bookmark(href));
        }
        xml.push_str("</xbel>\n");
        std::fs::write(&self.xbel, xml).unwrap();
    }

    fn read(&self) -> String {
        std::fs::read_to_string(&self.xbel).unwrap()
    }

    fn bookmark_count(&self) -> usize {
        self.read().matches("<bookmark href=").count()
    }
}

impl Drop for Sandbox {
    fn drop(&mut self) {
        match &self.previous {
            Some(v) => std::env::set_var("XDG_DATA_HOME", v),
            None => std::env::remove_var("XDG_DATA_HOME"),
        }
    }
}

/// One multi-line `<bookmark>` element in the layout GLib writes.
fn bookmark(href: &str) -> String {
    format!(
        "  <bookmark href=\"{href}\" added=\"2024-01-01T00:00:00Z\" \
         modified=\"2024-01-01T00:00:00Z\" visited=\"2024-01-01T00:00:00Z\">\n    \
         <info>\n      <metadata owner=\"http://freedesktop.org\">\n        \
         <mime:mime-type type=\"text/plain\"/>\n        \
         <bookmark:applications>\n          \
         <bookmark:application name=\"flux\" exec=\"&apos;flux %u&apos;\" \
         modified=\"2024-01-01T00:00:00Z\" count=\"1\"/>\n        \
         </bookmark:applications>\n      </metadata>\n    </info>\n  </bookmark>\n"
    )
}

fn remove(paths: &[&str]) {
    let paths = paths.iter().map(PathBuf::from).collect();
    FluxApp::remove_recents(Some(paths)).unwrap();
}

#[test]
fn removes_a_path_containing_ampersand_apostrophe_and_quotes() {
    let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let sb = Sandbox::new();
    sb.write(&[PLAIN_HREF, AMP_QUOTE_HREF, FOLDER_HREF]);

    remove(&[AMP_QUOTE_PATH]);

    let after = sb.read();
    assert!(!after.contains("Tom"), "entry was not removed:\n{after}");
    assert_eq!(sb.bookmark_count(), 2);
    assert_eq!(after.matches("</bookmark>").count(), 2, "orphaned end tag");
    assert!(after.contains(PLAIN_HREF));
    assert!(after.contains(FOLDER_HREF));
}

#[test]
fn removes_a_path_containing_an_apostrophe() {
    let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let sb = Sandbox::new();
    sb.write(&[APOSTROPHE_HREF, PLAIN_HREF]);

    remove(&[APOSTROPHE_PATH]);

    assert!(!sb.read().contains("Stop"));
    assert_eq!(sb.bookmark_count(), 1);
}

#[test]
fn removes_a_folder_with_an_ampersand() {
    let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let sb = Sandbox::new();
    sb.write(&[FOLDER_HREF, PLAIN_HREF]);

    remove(&[FOLDER_PATH]);

    assert!(!sb.read().contains("R&amp;D"));
    assert_eq!(sb.bookmark_count(), 1);
}

#[test]
fn removing_a_folder_keeps_the_entries_inside_it() {
    let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let sb = Sandbox::new();
    sb.write(&[FOLDER_HREF, INSIDE_FOLDER_HREF]);

    remove(&[FOLDER_PATH]);

    let after = sb.read();
    assert!(
        after.contains(INSIDE_FOLDER_HREF),
        "matched by prefix:\n{after}"
    );
    assert_eq!(sb.bookmark_count(), 1);
}

#[test]
fn removes_several_paths_in_one_call() {
    let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let sb = Sandbox::new();
    sb.write(&[AMP_QUOTE_HREF, PLAIN_HREF, APOSTROPHE_HREF, FOLDER_HREF]);

    remove(&[AMP_QUOTE_PATH, APOSTROPHE_PATH]);

    let after = sb.read();
    assert_eq!(sb.bookmark_count(), 2);
    assert!(after.contains(PLAIN_HREF));
    assert!(after.contains(FOLDER_HREF));
}

#[test]
fn still_removes_a_plain_path() {
    let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let sb = Sandbox::new();
    sb.write(&[PLAIN_HREF, AMP_QUOTE_HREF]);

    remove(&[PLAIN_PATH]);

    assert!(!sb.read().contains(PLAIN_HREF));
    assert_eq!(sb.bookmark_count(), 1);
}

#[test]
fn a_path_that_is_not_in_recents_changes_nothing() {
    let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let sb = Sandbox::new();
    sb.write(&[PLAIN_HREF, AMP_QUOTE_HREF]);

    remove(&["/home/me/other & more.txt"]);

    assert_eq!(sb.bookmark_count(), 2);
}

#[test]
fn clearing_everything_removes_every_bookmark() {
    let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let sb = Sandbox::new();
    sb.write(&[PLAIN_HREF, AMP_QUOTE_HREF, FOLDER_HREF]);

    FluxApp::remove_recents(None).unwrap();

    let after = sb.read();
    assert_eq!(sb.bookmark_count(), 0);
    assert!(after.contains("<xbel"), "document wrapper must survive");
    assert!(after.contains("</xbel>"));
}

#[test]
fn a_missing_xbel_file_is_not_an_error() {
    let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let sb = Sandbox::new();
    assert!(!sb.xbel.exists());

    assert!(FluxApp::remove_recents(Some(vec![PathBuf::from(PLAIN_PATH)])).is_ok());
    assert!(FluxApp::remove_recents(None).is_ok());
}
