use serde::Serialize;
use std::path::PathBuf;

#[derive(Debug, Serialize)]
pub struct ConfigReport {
    pub project_name: String,
    pub project_root: PathBuf,
    pub item_count: usize,
    pub items: Vec<ConfigItem>,
}

#[derive(Debug, Serialize)]
pub struct ConfigItem {
    pub file: PathBuf,
    pub line: usize,
    pub column: usize,
    pub key: String,
    pub env_key: Option<String>,
    pub default_value: Option<String>,
    pub env_value: Option<String>,
    pub source: ConfigSource,
}

#[derive(Debug, Clone, Serialize)]
pub struct ConfigSource {
    pub kind: String,
    pub declared_in: PathBuf,
    pub line: usize,
    pub column: usize,
    pub provider_class: Option<String>,
}
