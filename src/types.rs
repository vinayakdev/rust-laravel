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

#[derive(Debug, Serialize)]
pub struct ViewReport {
    pub project_name: String,
    pub project_root: PathBuf,
    pub view_count: usize,
    pub blade_component_count: usize,
    pub livewire_component_count: usize,
    pub views: Vec<ViewEntry>,
    pub blade_components: Vec<BladeComponentEntry>,
    pub livewire_components: Vec<LivewireComponentEntry>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ViewEntry {
    pub name: String,
    pub file: PathBuf,
    pub kind: String,
    pub props: Vec<ViewVariable>,
    pub variables: Vec<ViewVariable>,
    pub usages: Vec<ViewUsage>,
    pub source: ViewSource,
}

#[derive(Debug, Clone, Serialize)]
pub struct ViewSource {
    pub declared_in: PathBuf,
    pub line: usize,
    pub column: usize,
    pub provider_class: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct BladeComponentEntry {
    pub component: String,
    pub kind: String,
    pub class_name: Option<String>,
    pub class_file: Option<PathBuf>,
    pub view_name: Option<String>,
    pub view_file: Option<PathBuf>,
    pub props: Vec<ViewVariable>,
    pub source: ViewSource,
}

#[derive(Debug, Clone, Serialize)]
pub struct LivewireComponentEntry {
    pub component: String,
    pub kind: String,
    pub class_name: Option<String>,
    pub class_file: Option<PathBuf>,
    pub view_name: Option<String>,
    pub view_file: Option<PathBuf>,
    pub state: Vec<ViewVariable>,
    pub source: ViewSource,
}

#[derive(Debug, Clone, Serialize)]
pub struct ViewVariable {
    pub name: String,
    pub default_value: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ViewUsage {
    pub kind: String,
    pub source: ViewSource,
    pub variables: Vec<ViewVariable>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RelationEntry {
    pub method: String,
    pub relation_type: String,
    pub related_model: String,
    pub related_model_file: Option<PathBuf>,
    pub foreign_key: Option<String>,
    pub local_key: Option<String>,
    pub pivot_table: Option<String>,
    pub line: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct ColumnEntry {
    pub name: String,
    pub column_type: String,
    pub nullable: bool,
    pub default: Option<String>,
    pub unique: bool,
    pub unsigned: bool,
    pub primary: bool,
    pub enum_values: Vec<String>,
    pub comment: Option<String>,
    pub references: Option<String>,
    pub on_table: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct IndexEntry {
    pub columns: Vec<String>,
    pub index_type: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ModelEntry {
    pub file: PathBuf,
    pub line: usize,
    pub class_name: String,
    pub namespace: String,
    pub table: String,
    pub table_inferred: bool,
    pub primary_key: String,
    pub key_type: String,
    pub incrementing: bool,
    pub timestamps: bool,
    pub soft_deletes: bool,
    pub connection: Option<String>,
    pub fillable: Vec<String>,
    pub guarded: Vec<String>,
    pub hidden: Vec<String>,
    pub casts: BTreeMap<String, String>,
    pub appends: Vec<String>,
    pub with: Vec<String>,
    pub traits: Vec<String>,
    pub relations: Vec<RelationEntry>,
    pub scopes: Vec<String>,
    pub accessors: Vec<String>,
    pub mutators: Vec<String>,
    pub columns: Vec<ColumnEntry>,
}

#[derive(Debug, Serialize)]
pub struct ModelReport {
    pub project_name: String,
    pub project_root: PathBuf,
    pub model_count: usize,
    pub models: Vec<ModelEntry>,
}

#[derive(Debug, Clone, Serialize)]
pub struct MigrationEntry {
    pub file: PathBuf,
    pub timestamp: String,
    pub class_name: String,
    pub table: String,
    pub operation: String,
    pub columns: Vec<ColumnEntry>,
    pub indexes: Vec<IndexEntry>,
    pub dropped_columns: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct MigrationReport {
    pub project_name: String,
    pub project_root: PathBuf,
    pub migration_count: usize,
    pub migrations: Vec<MigrationEntry>,
}
