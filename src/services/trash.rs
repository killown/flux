use crate::model::AppMsg;
use crate::model::FluxApp;
use adw::gio::prelude::*;
use gtk::{gio, glib};
use relm4::prelude::*;
use std::path::PathBuf;

pub fn delete_items(
    selection: Vec<PathBuf>,
    active_item_path: Option<PathBuf>,
    sender: AsyncComponentSender<FluxApp>,
) {
    let mut selection = selection;
    if selection.is_empty() {
        if let Some(active) = active_item_path {
            selection.push(active);
        }
    }

    if selection.is_empty() {
        eprintln!("[Delete] no selection and no active_item_path, bailing");
        return;
    }

    eprintln!("[Delete] {} item(s) selected", selection.len());

    let sender_clone = sender.clone();
    for path in selection {
        let path_str = path.to_string_lossy().into_owned();
        let is_network = crate::services::network::is_network_uri(&path);

        eprintln!(
            "[Delete] path={:?} is_network={} contains_scheme={}",
            path_str,
            is_network,
            path_str.contains("://")
        );

        let file = if path_str.contains("://") {
            gio::File::for_uri(&path_str)
        } else {
            gio::File::for_path(&path)
        };

        let s = sender_clone.clone();

        if is_network {
            eprintln!(
                "[Delete] network path → skipping trash, using GLib-context delete_recursive"
            );
            let file_clone = file.clone();
            glib::MainContext::default().spawn_local(async move {
                fn delete_recursive(f: &gio::File) -> Result<(), glib::Error> {
                    let uri = f.uri().to_string();
                    eprintln!("[delete_recursive] entering uri={:?}", uri);

                    let info = f.query_info(
                        "standard::type",
                        gio::FileQueryInfoFlags::NOFOLLOW_SYMLINKS,
                        gio::Cancellable::NONE,
                    );

                    match info {
                        Err(ref e) => {
                            eprintln!(
                                "[delete_recursive] query_info failed uri={:?} err={:?}",
                                uri, e.message()
                            );
                        }
                        Ok(ref info) => {
                            eprintln!(
                                "[delete_recursive] file_type={:?} uri={:?}",
                                info.file_type(), uri
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
                                            uri, e.message()
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

                eprintln!("[Delete] spawned GLib task for uri={:?}", file_clone.uri().to_string());
                match delete_recursive(&file_clone) {
                    Ok(()) => {
                        eprintln!("[Delete] delete_recursive succeeded, sending Refresh");
                        s.input(AppMsg::Refresh);
                    }
                    Err(e) => {
                        eprintln!(
                            "[Delete] delete_recursive failed uri={:?} gio_kind={:?} msg={:?}",
                            file_clone.uri().to_string(),
                            e.kind::<gio::IOErrorEnum>(),
                            e.message()
                        );
                        if e.message().contains("Permission denied")
                            || e.message().contains("Operation not permitted")
                        {
                            s.input(AppMsg::ShowToast(
                                "Permission denied: Cannot delete item.".into(),
                            ));
                        } else {
                            s.input(AppMsg::ShowToast(format!("Deletion error: {e}")));
                        }
                    }
                }
            });
        } else {
            eprintln!("[Delete] local path → attempting trash_async");
            let file_for_fallback = file.clone();
            file.trash_async(
                glib::Priority::DEFAULT,
                gio::Cancellable::NONE,
                move |res| {
                    match res {
                        Ok(_) => {
                            eprintln!("[Delete] trash_async succeeded, sending Refresh");
                            s.input(AppMsg::Refresh);
                        }
                        Err(trash_err) => {
                            eprintln!(
                                "[Delete] trash_async failed gio_kind={:?} msg={:?}, falling back to delete_recursive",
                                trash_err.kind::<gio::IOErrorEnum>(),
                                trash_err.message()
                            );
                            let s_inner = s.clone();
                            relm4::spawn_blocking(move || {
                                fn delete_recursive(f: &gio::File) -> Result<(), glib::Error> {
                                    let uri = f.uri().to_string();
                                    eprintln!("[delete_recursive/local] entering uri={:?}", uri);
                                    if let Ok(info) = f.query_info(
                                        "standard::type",
                                        gio::FileQueryInfoFlags::NOFOLLOW_SYMLINKS,
                                        gio::Cancellable::NONE,
                                    ) {
                                        eprintln!(
                                            "[delete_recursive/local] file_type={:?} uri={:?}",
                                            info.file_type(), uri
                                        );
                                        if info.file_type() == gio::FileType::Directory {
                                            if let Ok(enumerator) = f.enumerate_children(
                                                "standard::name",
                                                gio::FileQueryInfoFlags::NOFOLLOW_SYMLINKS,
                                                gio::Cancellable::NONE,
                                            ) {
                                                for child_info in enumerator.flatten() {
                                                    let child = f.child(child_info.name());
                                                    delete_recursive(&child)?;
                                                }
                                            }
                                        }
                                    }
                                    eprintln!("[delete_recursive/local] calling f.delete() uri={:?}", uri);
                                    f.delete(gio::Cancellable::NONE)?;
                                    Ok(())
                                }

                                match delete_recursive(&file_for_fallback) {
                                    Ok(()) => {
                                        eprintln!("[Delete] fallback delete_recursive succeeded");
                                        s_inner.input(AppMsg::Refresh);
                                    }
                                    Err(e) => {
                                        eprintln!(
                                            "[Delete] fallback delete_recursive failed gio_kind={:?} msg={:?}",
                                            e.kind::<gio::IOErrorEnum>(),
                                            e.message()
                                        );
                                        if e.message().contains("Permission denied")
                                            || e.message().contains("Operation not permitted")
                                        {
                                            s_inner.input(AppMsg::ShowToast(
                                                "Permission denied: Cannot delete item.".into(),
                                            ));
                                        } else {
                                            s_inner.input(AppMsg::ShowToast(format!(
                                                "Deletion error: {}",
                                                e
                                            )));
                                        }
                                    }
                                }
                            });
                            let _ = trash_err;
                        }
                    }
                },
            );
        }
    }
}
