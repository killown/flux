use gtk::gio::prelude::FileExt;
use gtk::{gio, glib};

/// Recursively deletes `f`, descending into directories with
/// `NOFOLLOW_SYMLINKS`. The final `f.delete()` is what actually unlinks the
/// entry - the recursion is only there so a non-empty directory doesn't fail
/// with `ENOTEMPTY`.
pub(super) fn delete_recursive(f: &gio::File) -> Result<(), glib::Error> {
    let uri = f.uri().to_string();
    eprintln!("[delete_recursive] entering uri={:?}", uri);

    match f.query_info(
        "standard::type",
        gio::FileQueryInfoFlags::NOFOLLOW_SYMLINKS,
        gio::Cancellable::NONE,
    ) {
        Err(ref e) => {
            eprintln!(
                "[delete_recursive] query_info failed uri={:?} err={:?}",
                uri,
                e.message()
            );
        }
        Ok(ref info) => {
            eprintln!(
                "[delete_recursive] file_type={:?} uri={:?}",
                info.file_type(),
                uri
            );
            if info.file_type() == gio::FileType::Directory {
                match f.enumerate_children(
                    "standard::name",
                    gio::FileQueryInfoFlags::NOFOLLOW_SYMLINKS,
                    gio::Cancellable::NONE,
                ) {
                    Err(ref e) => {
                        eprintln!(
                            "[delete_recursive] enumerate_children failed uri={:?} err={:?}",
                            uri,
                            e.message()
                        );
                    }
                    Ok(enumerator) => {
                        for child_info in enumerator.flatten() {
                            let child = f.child(child_info.name());
                            eprintln!(
                                "[delete_recursive] recursing into child={:?}",
                                child.uri().to_string()
                            );
                            delete_recursive(&child)?;
                        }
                    }
                }
            }
        }
    }

    eprintln!("[delete_recursive] calling f.delete() uri={:?}", uri);
    let res = f.delete(gio::Cancellable::NONE);
    eprintln!(
        "[delete_recursive] f.delete() result={} uri={:?}",
        res.as_ref().map(|_| "ok").unwrap_or("err"),
        uri
    );
    res?;
    Ok(())
}
