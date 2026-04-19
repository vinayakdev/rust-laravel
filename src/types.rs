use serde::Serialize;
use std::path::PathBuf;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum OutputMode {
    Text,
    Json,
}

#[derive(Debug, Serialize)]
pub struct RouteEntry {
    pub file: PathBuf,
    pub line: usize,
    pub column: usize,
    pub methods: Vec<String>,
    pub uri: String,
    pub name: Option<String>,
    pub action: Option<String>,
    pub middleware: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct RouteReport {
    pub project_name: String,
    pub project_root: PathBuf,
    pub route_count: usize,
    pub routes: Vec<RouteEntry>,
}

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
}
