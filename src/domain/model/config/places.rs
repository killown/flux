use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq, Eq)]
pub struct DeviceRename {
    pub name: String,
    pub icon: Option<String>,
}

/// A user-defined location entry for the sidebar bookmarks.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct CustomPlace {
    pub name: String,
    #[serde(default)]
    pub kind: Option<String>,
    pub icon: String,
    pub path: String,
}
