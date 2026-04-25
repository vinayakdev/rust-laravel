use crate::core::analysis::ProjectAnalysis;
use crate::lsp::overrides::FileOverrides;
use crate::project::LaravelProject;
use crate::types::ConfigReport;

pub fn analyze(project: &LaravelProject) -> Result<ConfigReport, String> {
    analyze_with_context(&ProjectAnalysis::from_project(project))
}

pub fn analyze_with_overrides(
    project: &LaravelProject,
    overrides: &FileOverrides,
) -> Result<ConfigReport, String> {
    let context = ProjectAnalysis::new(project, overrides.clone());
    analyze_with_context(&context)
}

pub fn analyze_with_context(context: &ProjectAnalysis) -> Result<ConfigReport, String> {
    let provider_report = context.providers()?;
    rust_php_configs::analyze(
        context.project(),
        &provider_report.providers,
        context.overrides(),
    )
}
