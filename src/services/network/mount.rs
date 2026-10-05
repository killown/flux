use std::collections::HashMap;
use std::path::PathBuf;

use gtk::gio;
use gtk::prelude::*;

use super::credentials::NetworkCredentials;
use super::debug::net_debug;
use super::error::NetworkError;
use super::protocol::is_network_uri;

/// True when the error is `G_IO_ERROR_ALREADY_MOUNTED`. That is not a real
/// failure: the volume is attached and the subsequent `enumerate_children`
/// or `make_directory` call will proceed against the existing mount.
#[inline]
pub(super) fn is_already_mounted(e: &glib::Error) -> bool {
    e.kind::<gio::IOErrorEnum>() == Some(gio::IOErrorEnum::AlreadyMounted)
        || e.matches(gio::IOErrorEnum::AlreadyMounted)
}

pub(super) fn build_mount_op(credentials: Option<&NetworkCredentials>) -> gio::MountOperation {
    let op = gio::MountOperation::new();

    if let Some(creds) = credentials {
        if creds.anonymous {
            net_debug!("[network] build_mount_op: anonymous=true");
            op.set_anonymous(true);
            return op;
        }
        if let Some(ref user) = creds.username {
            op.set_username(Some(user.as_str()));
        }
        if let Some(ref pwd) = creds.password {
            op.set_password(Some(pwd.as_str()));
        }
        if let Some(ref domain) = creds.domain {
            op.set_domain(Some(domain.as_str()));
        }
        op.set_password_save(gio::PasswordSave::ForSession);
    }

    op
}

pub(super) async fn mount_enclosing_volume(
    file: &gio::File,
    mount_op: &gio::MountOperation,
) -> Result<(), glib::Error> {
    file.mount_enclosing_volume_future(gio::MountMountFlags::NONE, Some(mount_op))
        .await
}

async fn unmount_with_operation(
    mount: &gio::Mount,
    mount_op: &gio::MountOperation,
) -> Result<(), glib::Error> {
    mount
        .unmount_with_operation_future(gio::MountUnmountFlags::NONE, Some(mount_op))
        .await
}

pub async fn unmount_network_location(uri: &str) -> Result<(), NetworkError> {
    net_debug!("[network] unmount_network_location uri = {uri:?}");
    let file = gio::File::for_uri(uri);
    let mount = file
        .find_enclosing_mount(gio::Cancellable::NONE)
        .map_err(NetworkError::from)?;

    net_debug!("[network]   found mount: {:?}", mount.name());
    let mount_op = gio::MountOperation::new();
    match unmount_with_operation(&mount, &mount_op).await {
        Ok(()) => {
            net_debug!("[network]   unmount OK");
            Ok(())
        }
        Err(e) => {
            eprintln!("[network]   unmount FAILED: {e}");
            Err(NetworkError::from(e))
        }
    }
}

pub fn active_mounts() -> Vec<(String, String, String)> {
    let result: Vec<(String, String, String)> = gio::VolumeMonitor::get()
        .mounts()
        .into_iter()
        .filter_map(|mount| {
            let root = mount.root();
            let uri = root.uri().to_string();

            if !is_network_uri(&PathBuf::from(&uri)) {
                return None;
            }

            let name = mount.name().to_string();
            let icon = mount
                .icon()
                .downcast::<gio::ThemedIcon>()
                .ok()
                .and_then(|themed| themed.names().first().map(|s| s.to_string()))
                .unwrap_or_else(|| "folder-remote-symbolic".to_owned());

            Some((uri, name, icon))
        })
        .collect();
    net_debug!("[network] active_mounts → {} mounts", result.len());
    for (uri, name, _) in &result {
        net_debug!("[network]   mount: {name:?} → {uri:?}");
    }
    result
}

pub fn check_gvfs_deps() -> HashMap<&'static str, bool> {
    let mut results = HashMap::new();
    for binary in ["gvfsd", "gvfsd-smb", "gvfsd-sftp", "gvfsd-ftp", "gvfsd-nfs"] {
        let found = std::process::Command::new("which")
            .arg(binary)
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);
        results.insert(binary, found);
    }
    net_debug!("[network] check_gvfs_deps → {results:?}");
    results
}
