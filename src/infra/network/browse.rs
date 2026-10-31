use gtk::gio;
use gtk::prelude::*;

use super::debug::net_debug;
use super::entry::{entries_to_load_contexts, themed_icon_name, NetworkEntry};
use super::error::{classify_enum_error, NetworkError};
use super::mount::{build_mount_op, is_already_mounted, mount_enclosing_volume};
use super::protocol::protocol_for_uri;
use crate::domain::model::{FileLoadContext, NetworkCredentials};

async fn off_thread<T: Send + 'static>(
    job: impl FnOnce() -> Result<T, NetworkError> + Send + 'static,
) -> Result<T, NetworkError> {
    match relm4::spawn_blocking(job).await {
        Ok(result) => result,
        Err(e) => Err(NetworkError::Other(format!(
            "network listing task failed: {e}"
        ))),
    }
}

/// Mounts the volume enclosing `uri` unless GVFS already has it mounted.
async fn ensure_mounted(uri: &str, credentials: Option<&NetworkCredentials>) {
    let uri_owned = uri.to_string();
    let mounted = off_thread(move || {
        Ok(gio::File::for_uri(&uri_owned)
            .find_enclosing_mount(gio::Cancellable::NONE)
            .is_ok())
    })
    .await
    .unwrap_or(false);

    if mounted {
        net_debug!("[network]   enclosing mount found, skipping mount_enclosing_volume");
        return;
    }

    let file = gio::File::for_uri(uri);
    let mount_op = build_mount_op(credentials);
    net_debug!("[network]   calling mount_enclosing_volume…");
    match mount_enclosing_volume(&file, &mount_op).await {
        Ok(()) => net_debug!("[network]   mount_enclosing_volume OK"),
        Err(e) if is_already_mounted(&e) => {
            net_debug!("[network]   mount_enclosing_volume: already mounted");
        }
        Err(e) => eprintln!("[network]   mount_enclosing_volume FAILED: {e}"),
    }
}

/// Blocking: enumerates `uri` and returns the sorted entries. Must not run on
/// the GTK thread, call it through `off_thread`.
fn enumerate_entries(uri: &str) -> Result<Vec<NetworkEntry>, NetworkError> {
    let cancellable = gio::Cancellable::new();
    let file = gio::File::for_uri(uri);

    // WARNING: do not add `standard::content-type` or `standard::icon` here.
    // GVFS's Google Drive backend resolves them per entry with a full metadata
    // fetch, which turns a listing into an N-round-trip operation.
    // `standard::type` is enough to distinguish files from directories on every
    // mainstream backend, and file icons are resolved locally from the name.
    let attributes =
        "standard::name,standard::display-name,standard::type,standard::size,time::modified";
    net_debug!("[network]   attributes = {attributes:?}");

    net_debug!("[network]   calling enumerate_children…");
    let enumerator = match file.enumerate_children(
        attributes,
        gio::FileQueryInfoFlags::NONE,
        Some(&cancellable),
    ) {
        Ok(e) => {
            net_debug!("[network]   enumerate_children OK");
            e
        }
        Err(e) => {
            eprintln!("[network]   enumerate_children FAILED: {e}");
            eprintln!("[network]   error kind: {:?}", e.kind::<gio::IOErrorEnum>());
            return Err(classify_enum_error(e, uri));
        }
    };

    let mut entries = Vec::new();
    let mut raw_count = 0usize;

    for info in enumerator.flatten() {
        raw_count += 1;
        let name = info.name();
        let file_type = info.file_type();
        let size = info.size();

        net_debug!("[network]   raw#{raw_count} name={name:?} type={file_type:?} size={size}");

        let display_name = info.display_name().to_string();
        let child_file = file.child(info.name());
        let child_uri = child_file.uri().to_string();

        let is_dir = match file_type {
            gio::FileType::Directory => true,
            gio::FileType::Regular => false,
            _ => info.name().to_string_lossy().ends_with('/'),
        };

        let size = info.size().max(0) as u64;
        let mtime = info
            .modification_date_time()
            .map(|dt| dt.to_unix())
            .unwrap_or(0);

        let mut icon_name = themed_icon_name(info.icon());

        if !is_dir {
            if let Some(ref name) = icon_name {
                if name.contains("folder") || name.contains("directory") || name.contains("server")
                {
                    icon_name = None;
                }
            }
        }

        if is_dir && icon_name.is_none() {
            icon_name = Some("inode-directory".to_owned());
        }

        let protocol = protocol_for_uri(&child_uri);

        net_debug!(
            "[network]     → push display_name={display_name:?} child_uri={child_uri:?} \
             is_dir={is_dir} size={size} mtime={mtime} icon_name={icon_name:?} proto={protocol:?}"
        );

        entries.push(NetworkEntry {
            display_name,
            uri: child_uri,
            is_dir,
            size,
            mtime,
            icon_name,
            protocol,
        });
    }

    net_debug!(
        "[network]   enumeration done: raw_count={raw_count} entries.len()={}",
        entries.len()
    );

    entries.sort_unstable_by(|a, b| {
        b.is_dir.cmp(&a.is_dir).then_with(|| {
            a.display_name
                .to_lowercase()
                .cmp(&b.display_name.to_lowercase())
        })
    });

    Ok(entries)
}

pub async fn list_network_entries(
    uri: &str,
    credentials: Option<&NetworkCredentials>,
) -> Result<Vec<NetworkEntry>, NetworkError> {
    crate::hit!("list_network_entries");
    net_debug!("[network] ────────────────────────────────────────────────");
    net_debug!("[network] list_network_entries ENTER uri = {uri:?}");
    net_debug!(
        "[network]   credentials = {:?}",
        credentials.map(|c| {
            format!(
                "user={:?} anon={} domain={:?}",
                c.username, c.anonymous, c.domain
            )
        })
    );

    let file = gio::File::for_uri(uri);
    net_debug!("[network]   file.uri() = {:?}", file.uri());
    net_debug!("[network]   file.path() = {:?}", file.path());
    net_debug!("[network]   file.basename() = {:?}", file.basename());

    ensure_mounted(uri, credentials).await;

    let uri_owned = uri.to_string();
    let entries = off_thread(move || enumerate_entries(&uri_owned)).await?;

    net_debug!(
        "[network] list_network_entries EXIT uri = {uri:?} ({} entries)",
        entries.len()
    );
    net_debug!("[network] ────────────────────────────────────────────────");

    Ok(entries)
}

/// Lists `uri` and returns ready-to-display load contexts.
pub async fn list_network_contexts(
    uri: &str,
    credentials: Option<&NetworkCredentials>,
    expand_labels: bool,
) -> Result<Vec<FileLoadContext>, NetworkError> {
    crate::hit!("list_network_contexts");
    net_debug!("[network] list_network_contexts ENTER uri = {uri:?}");

    ensure_mounted(uri, credentials).await;

    let uri_owned = uri.to_string();
    let contexts = off_thread(move || {
        let entries = enumerate_entries(&uri_owned)?;
        let contexts = entries_to_load_contexts(&entries, expand_labels);

        // WARNING: do not remove these lines.
        //
        // Same reasoning as `loader/directory.rs`: these getters are lazy, and
        // whatever is still empty when the item reaches the UI is computed on the
        // GTK thread, one item at a time. `with_stats` already seeds size, mtime
        // and the owner flag for network entries, so in practice `labels()` (date
        // and size formatting) is the one doing work here, the rest stay so this
        // keeps working if the seeding ever changes.
        let uid = unsafe { libc::geteuid() };
        for item in &contexts {
            let _ = item.size();
            let _ = item.mtime();
            let _ = item.is_foreign_owner(uid);
            let _ = item.labels();
        }
        Ok(contexts)
    })
    .await?;

    net_debug!(
        "[network] list_network_contexts EXIT uri = {uri:?} ({} contexts)",
        contexts.len()
    );
    Ok(contexts)
}

pub async fn create_network_directory(
    uri: &str,
    credentials: Option<&NetworkCredentials>,
) -> Result<(), NetworkError> {
    crate::hit!("create_network_directory");
    net_debug!("[network] create_network_directory uri = {uri:?}");
    let file = gio::File::for_uri(uri);
    let mount_op = build_mount_op(credentials);
    match mount_enclosing_volume(&file, &mount_op).await {
        Ok(()) => net_debug!("[network]   mount_enclosing_volume OK"),
        Err(e) if is_already_mounted(&e) => {
            net_debug!("[network]   mount_enclosing_volume: already mounted");
        }
        Err(e) => eprintln!("[network]   mount_enclosing_volume FAILED: {e}"),
    }
    match file.make_directory(None::<&gio::Cancellable>) {
        Ok(()) => {
            net_debug!("[network]   make_directory OK");
            Ok(())
        }
        Err(e) => {
            eprintln!("[network]   make_directory FAILED: {e}");
            Err(NetworkError::from(e))
        }
    }
}
