use serde::Serialize;
use std::path::PathBuf;

#[derive(Debug, Serialize)]
pub struct ViewReport {
    pub project_name: String,
    pub project_root: PathBuf,
    pub view_count: usize,
    pub blade_component_count: usize,
    pub livewire_component_count: usize,
    pub missing_view_count: usize,
    pub views: Vec<ViewEntry>,
    pub blade_components: Vec<BladeComponentEntry>,
    pub livewire_components: Vec<LivewireComponentEntry>,
    pub missing_views: Vec<MissingViewEntry>,
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
    pub actions: Vec<LivewireActionEntry>,
    pub source: ViewSource,
}

#[derive(Debug, Clone, Serialize)]
pub struct LivewireActionEntry {
    pub name: String,
    pub line: usize,
    pub column: usize,
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
pub struct MissingViewEntry {
    pub name: String,
    pub expected_file: PathBuf,
    pub usages: Vec<ViewUsage>,
}
