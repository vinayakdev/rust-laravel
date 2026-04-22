use crate::analyzers::controllers;
use crate::core::analysis::ProjectAnalysis;
use crate::lsp::overrides::FileOverrides;
use crate::project::LaravelProject;
use crate::types::RouteReport;

pub fn analyze(project: &LaravelProject) -> Result<RouteReport, String> {
    analyze_with_context(&ProjectAnalysis::from_project(project))
}

pub fn analyze_with_overrides(
    project: &LaravelProject,
    overrides: &FileOverrides,
) -> Result<RouteReport, String> {
    let context = ProjectAnalysis::new(project, overrides.clone());
    analyze_with_context(&context)
}

pub fn analyze_with_context(context: &ProjectAnalysis) -> Result<RouteReport, String> {
    let middleware = context.middleware()?;
    let middleware_index = rust_php_routes::routes::MiddlewareIndex::new()
        .with_aliases(
            middleware
                .aliases
                .iter()
                .map(|alias| (alias.name.clone(), alias.target.clone())),
        )
        .with_groups(
            middleware
                .groups
                .iter()
                .map(|group| (group.name.clone(), group.members.clone())),
        )
        .with_patterns(
            middleware
                .patterns
                .iter()
                .map(|pattern| (pattern.parameter.clone(), pattern.pattern.clone())),
        );

    let provider_report = context.providers()?;
    let mut report = rust_php_routes::analyze_raw(
        context.project(),
        &provider_report.providers,
        context.overrides(),
        &middleware_index,
    )?;

    let controller_report = context.controllers()?;
    for route in &mut report.routes {
        route.controller_target = route
            .action
            .as_deref()
            .and_then(|action| controllers::resolve_route_target(controller_report, action));
    }

    Ok(report)
}

pub(crate) fn collect_registered_route_paths(
    context: &ProjectAnalysis,
) -> Result<Vec<std::path::PathBuf>, String> {
    let provider_report = context.providers()?;
    rust_php_routes::collect_registered_route_paths(
        context.project(),
        &provider_report.providers,
        context.overrides(),
    )
}

pub(crate) fn reindex_guard_reason(source: &[u8]) -> Option<&'static str> {
    rust_php_routes::reindex_guard_reason(source)
}
