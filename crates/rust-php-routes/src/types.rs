use rust_php_controllers::types::RouteControllerTarget;
use serde::Serialize;
use std::collections::BTreeMap;
use std::path::PathBuf;

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
    pub controller_target: Option<RouteControllerTarget>,
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
