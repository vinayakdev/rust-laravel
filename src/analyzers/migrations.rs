use crate::core::analysis::ProjectAnalysis;
use crate::project::LaravelProject;
use crate::types::{ColumnEntry, MigrationEntry, MigrationReport};

pub fn analyze(project: &LaravelProject) -> Result<MigrationReport, String> {
    analyze_with_context(&ProjectAnalysis::from_project(project))
}

pub fn analyze_with_context(context: &ProjectAnalysis) -> Result<MigrationReport, String> {
    let provider_report = context.providers()?;
    rust_php_migrations::analyze(
        context.project(),
        &provider_report.providers,
        context.overrides(),
    )
}

pub fn resolve_columns_for_table(table: &str, migrations: &[MigrationEntry]) -> Vec<ColumnEntry> {
    rust_php_migrations::resolve_columns_for_table(table, migrations)
}
