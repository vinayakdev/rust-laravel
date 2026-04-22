use serde::Serialize;
use std::path::PathBuf;

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
