use crate::core::analysis::ProjectAnalysis;
use crate::project::LaravelProject;
use crate::types::ModelReport;

pub fn analyze(project: &LaravelProject) -> Result<ModelReport, String> {
    analyze_with_context(&ProjectAnalysis::from_project(project))
}

pub fn analyze_with_context(context: &ProjectAnalysis) -> Result<ModelReport, String> {
    let mappings = context.psr4_mappings()?;
    let migration_report = context.migrations()?;
    rust_php_models::analyze(context.project(), mappings, &migration_report.migrations)
}
