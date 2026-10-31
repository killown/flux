use super::protocol::protocol_for_uri;
pub use crate::domain::model::NetworkBookmark;

impl NetworkBookmark {
    pub fn new(name: impl Into<String>, uri: impl Into<String>) -> Self {
        let uri = uri.into();
        let icon = protocol_for_uri(&uri)
            .map(|p| p.icon_name().to_owned())
            .unwrap_or_else(|| "folder-remote-symbolic".to_owned());
        Self {
            name: name.into(),
            uri,
            icon,
        }
    }
}
