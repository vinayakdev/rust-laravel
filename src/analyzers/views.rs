use crate::analyzers::routes;
use crate::core::analysis::ProjectAnalysis;
use crate::project::LaravelProject;
use crate::types::ViewReport;

pub fn analyze(project: &LaravelProject) -> Result<ViewReport, String> {
    analyze_with_context(&ProjectAnalysis::from_project(project))
}

pub fn analyze_with_context(context: &ProjectAnalysis) -> Result<ViewReport, String> {
    let mappings = context.psr4_mappings()?;
    let provider_report = context.providers()?;
    let route_files = routes::collect_registered_route_paths(context)?;

    rust_php_views::analyze(
        context.project(),
        mappings,
        &provider_report.providers,
        &route_files,
        context.overrides(),
    )
}
