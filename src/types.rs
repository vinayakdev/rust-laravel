use serde::Serialize;
use std::collections::BTreeMap;
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
    pub resolved_middleware: Vec<String>,
    pub parameter_patterns: BTreeMap<String, String>,
    pub registration: RouteRegistration,
}

#[derive(Debug, Serialize)]
pub struct RouteReport {
    pub project_name: String,
    pub project_root: PathBuf,
    pub route_count: usize,
    pub routes: Vec<RouteEntry>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RouteRegistration {
    pub kind: String,
    pub declared_in: PathBuf,
    pub line: usize,
    pub column: usize,
    pub provider_class: Option<String>,
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

#[derive(Debug, Serialize)]
pub struct ProviderReport {
    pub project_name: String,
    pub project_root: PathBuf,
    pub provider_count: usize,
    pub providers: Vec<ProviderEntry>,
}

#[derive(Debug, Serialize)]
pub struct ProviderEntry {
    pub provider_class: String,
    pub line: usize,
    pub column: usize,
    pub registration_kind: String,
    pub declared_in: PathBuf,
    pub package_name: Option<String>,
    pub source_file: Option<PathBuf>,
    pub source_available: bool,
    pub status: String,
}

#[derive(Debug, Serialize)]
pub struct MiddlewareReport {
    pub project_name: String,
    pub project_root: PathBuf,
    pub alias_count: usize,
    pub group_count: usize,
    pub pattern_count: usize,
    pub aliases: Vec<MiddlewareAlias>,
    pub groups: Vec<MiddlewareGroup>,
    pub patterns: Vec<RoutePattern>,
}

#[derive(Debug, Clone, Serialize)]
pub struct MiddlewareAlias {
    pub name: String,
    pub target: String,
    pub source: MiddlewareSource,
}

#[derive(Debug, Clone, Serialize)]
pub struct MiddlewareGroup {
    pub name: String,
    pub members: Vec<String>,
    pub source: MiddlewareSource,
}

#[derive(Debug, Clone, Serialize)]
pub struct RoutePattern {
    pub parameter: String,
    pub pattern: String,
    pub source: MiddlewareSource,
}

#[derive(Debug, Clone, Serialize)]
pub struct MiddlewareSource {
    pub declared_in: PathBuf,
    pub line: usize,
    pub column: usize,
    pub provider_class: String,
}
