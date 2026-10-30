use gtk::gio::prelude::FileExt;
use gtk::{gio, glib};

/// Recursively deletes `f`, descending into directories with
/// `NOFOLLOW_SYMLINKS`. The final `f.delete()` is what actually unlinks the
/// entry - the recursion is only there so a non-empty directory doesn't fail
/// with `ENOTEMPTY`.
pub fn delete_recursive(f: &gio::File) -> Result<(), glib::Error> {
    crate::hit!("delete_recursive");
    let uri = f.uri().to_string();

    if uri.starts_with("trash:///") {
        return f.delete(gio::Cancellable::NONE);
    }

    eprintln!("[delete_recursive] entering uri={:?}", uri);

    let info = f.query_info(
        "standard::type",
        gio::FileQueryInfoFlags::NOFOLLOW_SYMLINKS,
        gio::Cancellable::NONE,
    )?;

    eprintln!(
        "[delete_recursive] file_type={:?} uri={:?}",
        info.file_type(),
        uri
    );

    if info.file_type() == gio::FileType::Directory {
        let enumerator = f.enumerate_children(
            "standard::name",
            gio::FileQueryInfoFlags::NOFOLLOW_SYMLINKS,
            gio::Cancellable::NONE,
        )?;

        let iter = enumerator.into_iter();
        for child_info in iter {
            let child_info = child_info?;
            let child = f.child(child_info.name());
            eprintln!(
                "[delete_recursive] recursing into child={:?}",
                child.uri().to_string()
            );
            delete_recursive(&child)?;
        }
    }

    eprintln!("[delete_recursive] calling f.delete() uri={:?}", uri);
    let res = f.delete(gio::Cancellable::NONE);
    eprintln!(
        "[delete_recursive] f.delete() result={} uri={:?}",
        res.as_ref().map(|_| "ok").unwrap_or("err"),
        uri
    );
    res
}
