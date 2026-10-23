#[derive(Debug, Clone, Default)]
pub struct NetworkCredentials {
    pub username: Option<String>,
    pub password: Option<String>,
    pub domain: Option<String>,
    pub anonymous: bool,
}

impl NetworkCredentials {
    pub fn anonymous() -> Self {
        Self {
            anonymous: true,
            ..Default::default()
        }
    }

    pub fn with_password(username: impl Into<String>, password: impl Into<String>) -> Self {
        Self {
            username: Some(username.into()),
            password: Some(password.into()),
            ..Default::default()
        }
    }
}
