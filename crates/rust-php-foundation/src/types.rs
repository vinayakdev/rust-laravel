use serde::Serialize;
use std::path::PathBuf;

#[derive(Debug, Serialize)]
pub struct EnvItem {
    pub file: PathBuf,
    pub line: usize,
    pub column: usize,
    pub key: String,
    pub value: String,
}

#[derive(Debug, Serialize)]
pub struct ProviderReport {
    pub project_name: String,
    pub project_root: PathBuf,
    pub provider_count: usize,
    pub providers: Vec<ProviderEntry>,
}

#[derive(Debug, Clone, Serialize)]
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

#[derive(Debug, Clone, Serialize)]
pub struct ControllerVariableEntry {
    pub name: String,
    pub source_kind: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ControllerMethodEntry {
    pub name: String,
    pub declared_in: PathBuf,
    pub line: usize,
    pub visibility: String,
    pub is_static: bool,
    pub source_kind: String,
    pub source_name: String,
    pub accessible_from_route: bool,
    pub accessibility: String,
    pub variables: Vec<ControllerVariableEntry>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ControllerEntry {
    pub file: PathBuf,
    pub line: usize,
    pub class_end_line: usize,
    pub class_name: String,
    pub namespace: String,
    pub fqn: String,
    pub extends: Option<String>,
    pub traits: Vec<String>,
    pub method_count: usize,
    pub callable_method_count: usize,
    pub methods: Vec<ControllerMethodEntry>,
}

#[derive(Debug, Serialize)]
pub struct ControllerReport {
    pub project_name: String,
    pub project_root: PathBuf,
    pub controller_count: usize,
    pub controllers: Vec<ControllerEntry>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RouteControllerTarget {
    pub controller: String,
    pub method: String,
    pub declared_in: Option<PathBuf>,
    pub method_declared_in: Option<PathBuf>,
    pub method_line: Option<usize>,
    pub accessible_from_route: bool,
    pub status: String,
    pub source_kind: Option<String>,
    pub notes: Vec<String>,
}
