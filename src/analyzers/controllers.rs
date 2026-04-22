use crate::core::analysis::ProjectAnalysis;
use crate::lsp::overrides::FileOverrides;
use crate::project::LaravelProject;
use crate::types::{ControllerReport, RouteControllerTarget};

pub fn analyze(project: &LaravelProject) -> Result<ControllerReport, String> {
    analyze_with_context(&ProjectAnalysis::from_project(project))
}

pub fn analyze_with_overrides(
    project: &LaravelProject,
    overrides: &FileOverrides,
) -> Result<ControllerReport, String> {
    let context = ProjectAnalysis::new(project, overrides.clone());
    analyze_with_context(&context)
}

pub fn analyze_with_context(context: &ProjectAnalysis) -> Result<ControllerReport, String> {
    rust_php_controllers::analyze(
        context.project(),
        context.psr4_mappings()?,
        context.overrides(),
    )
}

pub fn resolve_route_target(
    report: &ControllerReport,
    action: &str,
) -> Option<RouteControllerTarget> {
    rust_php_controllers::resolve_route_target(report, action)
}
