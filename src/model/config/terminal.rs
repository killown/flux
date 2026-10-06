use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct TerminalConfig {
    pub height: i32,
    pub fg_color: String,
    pub bg_color: String,
    pub font: String,
    pub shell: Option<String>,
}

impl Default for TerminalConfig {
    fn default() -> Self {
        Self {
            height: 30,
            fg_color: "#E5E5E5".to_string(),
            bg_color: "#1A1A1A".to_string(),
            font: "JetBrains Mono 13".to_string(),
            shell: None,
        }
    }
}
