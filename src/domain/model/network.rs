//! Network location data shared by messages, config and the mount layer.

#[derive(Debug, Clone, Default)]
pub struct NetworkCredentials {
    pub username: Option<String>,
    pub password: Option<String>,
    pub domain: Option<String>,
    pub anonymous: bool,
}

impl NetworkCredentials {
    #[allow(dead_code)]
    pub fn anonymous() -> Self {
        Self {
            anonymous: true,
            ..Default::default()
        }
    }

    #[allow(dead_code)]
    pub fn with_password(username: impl Into<String>, password: impl Into<String>) -> Self {
        Self {
            username: Some(username.into()),
            password: Some(password.into()),
            ..Default::default()
        }
    }
}

bitflags::bitflags! {
    /// Which credential fields a server asked for.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct NetworkAuthFlags: u8 {
        const USERNAME = 0b0001;
        const PASSWORD = 0b0010;
        const DOMAIN   = 0b0100;
        const ANON_OK  = 0b1000;
    }
}

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
