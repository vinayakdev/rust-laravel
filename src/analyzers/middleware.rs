use crate::core::analysis::ProjectAnalysis;
use crate::project::LaravelProject;
use crate::types::MiddlewareReport;

pub fn analyze(project: &LaravelProject) -> Result<MiddlewareReport, String> {
    analyze_with_context(&ProjectAnalysis::from_project(project))
}

pub fn analyze_with_context(context: &ProjectAnalysis) -> Result<MiddlewareReport, String> {
    let provider_report = context.providers()?;
    rust_php_middleware::analyze(
        context.project(),
        &provider_report.providers,
        context.overrides(),
    )
}
