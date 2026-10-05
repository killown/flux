use gtk::gio;
use gtk::prelude::*;

use super::credentials::NetworkCredentials;
use super::debug::net_debug;
use super::entry::{themed_icon_name, NetworkEntry};
use super::error::{classify_enum_error, NetworkError};
use super::mount::{build_mount_op, is_already_mounted, mount_enclosing_volume};
use super::protocol::protocol_for_uri;

pub async fn list_network_entries(
    uri: &str,
    credentials: Option<&NetworkCredentials>,
) -> Result<Vec<NetworkEntry>, NetworkError> {
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

    let cancellable = gio::Cancellable::new();
    let file = gio::File::for_uri(uri);
    net_debug!("[network]   file.uri() = {:?}", file.uri());
    net_debug!("[network]   file.path() = {:?}", file.path());
    net_debug!("[network]   file.basename() = {:?}", file.basename());

    let mount_op = build_mount_op(credentials);
    net_debug!("[network]   calling mount_enclosing_volume…");
    match mount_enclosing_volume(&file, &mount_op).await {
        Ok(()) => net_debug!("[network]   mount_enclosing_volume OK"),
        Err(e) if is_already_mounted(&e) => {
            net_debug!("[network]   mount_enclosing_volume: already mounted");
        }
        Err(e) => eprintln!("[network]   mount_enclosing_volume FAILED: {e}"),
    }

    // WARNING: do not add `standard::content-type` here. GVFS's Google Drive
    // backend resolves it per entry with a full metadata fetch, which turns a
    // listing into an N-round-trip operation. `standard::type` is enough to
    // distinguish files from directories on every mainstream backend.
    let attributes =
        "standard::name,standard::display-name,standard::type,standard::size,time::modified,standard::icon";
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
        let has_icon = info.icon().is_some();

        net_debug!(
            "[network]   raw#{raw_count} name={name:?} type={file_type:?} size={size} \
             has_icon={has_icon}"
        );

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

    net_debug!(
        "[network] list_network_entries EXIT uri = {uri:?} ({} entries)",
        entries.len()
    );
    net_debug!("[network] ────────────────────────────────────────────────");

    Ok(entries)
}

pub async fn create_network_directory(
    uri: &str,
    credentials: Option<&NetworkCredentials>,
) -> Result<(), NetworkError> {
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
