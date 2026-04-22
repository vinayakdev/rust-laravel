use crate::core::analysis::ProjectAnalysis;
use crate::project::LaravelProject;
use crate::types::ProviderReport;

pub fn analyze(project: &LaravelProject) -> Result<ProviderReport, String> {
    analyze_with_context(&ProjectAnalysis::from_project(project))
}

pub fn analyze_with_context(context: &ProjectAnalysis) -> Result<ProviderReport, String> {
    rust_php_foundation::discovery::providers::analyze(context.project(), context.psr4_mappings()?)
}
