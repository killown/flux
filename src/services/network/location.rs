use gtk::gio;
use gtk::prelude::*;

use super::debug::net_debug;
use super::entry::themed_icon_name;
use super::protocol::protocol_for_uri;

#[derive(Debug, Clone, Default)]
pub struct ConnectToServerParams {
    pub protocol: String,
    pub host: String,
    pub port: Option<u16>,
    pub path: Option<String>,
    pub username: Option<String>,
}

impl ConnectToServerParams {
    pub fn build_uri(&self) -> Option<String> {
        if self.host.is_empty() {
            return None;
        }
        let scheme = match self.protocol.as_str() {
            "smb" => "smb",
            "sftp" | "ssh" => "sftp",
            "dav" => "dav",
            "davs" => "davs",
            "nfs" => "nfs",
            "ftp" => "ftp",
            "ftps" => "ftps",
            "afp" => "afp",
            "mtp" => "mtp",
            _ => return None,
        };

        let port_part = self.port.map(|p| format!(":{p}")).unwrap_or_default();

        let user_part = self
            .username
            .as_deref()
            .filter(|u| !u.is_empty())
            .map(|u| format!("{u}@"))
            .unwrap_or_default();

        let path_part = self
            .path
            .as_deref()
            .map(|p| {
                if p.starts_with('/') {
                    p.to_owned()
                } else {
                    format!("/{p}")
                }
            })
            .unwrap_or_else(|| "/".to_owned());

        let uri = format!(
            "{scheme}://{user_part}{host}{port_part}{path_part}",
            host = self.host,
        );
        net_debug!("[network] ConnectToServerParams::build_uri → {uri:?}");
        Some(uri)
    }
}

pub fn describe_network_location(uri: &str) -> (String, String) {
    net_debug!("[network] describe_network_location uri = {uri:?}");
    if uri.is_empty() || uri.contains('\0') {
        return (String::new(), "folder-remote-symbolic".to_owned());
    }

    let file = gio::File::for_uri(uri);
    let display = file
        .query_info(
            "standard::display-name,standard::icon",
            gio::FileQueryInfoFlags::NONE,
            gio::Cancellable::NONE,
        )
        .ok();

    let name = display
        .as_ref()
        .map(|i| i.display_name().to_string())
        .unwrap_or_else(|| uri_display_name(uri));

    let icon = display
        .and_then(|i| themed_icon_name(i.icon()))
        .unwrap_or_else(|| {
            protocol_for_uri(uri)
                .map(|p| p.icon_name().to_owned())
                .unwrap_or_else(|| "folder-remote-symbolic".to_owned())
        });

    net_debug!("[network]   → name={name:?} icon={icon:?}");
    (name, icon)
}

fn uri_display_name(uri: &str) -> String {
    let without_scheme = uri.split("://").nth(1).unwrap_or(uri).trim_end_matches('/');
    if without_scheme.is_empty() {
        return "".to_owned();
    }
    without_scheme.to_owned()
}
