use super::protocol::protocol_for_uri;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct NetworkBookmark {
    pub name: String,
    pub uri: String,
    #[serde(default = "default_network_icon")]
    pub icon: String,
}

fn default_network_icon() -> String {
    "folder-remote-symbolic".to_owned()
}

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
