use rust_php_foundation::discovery::providers;
use rust_php_foundation::overrides::FileOverrides;
use rust_php_foundation::php::psr4::collect_psr4_mappings;
use rust_php_foundation::project::LaravelProject;
use rust_php_middleware::analyze as analyze_middleware_raw;
use rust_php_routes::routes::MiddlewareIndex;

use crate::types::{
    ConfigReport, ControllerReport, RouteControllerTarget, RouteReport, ViewReport,
};

pub mod configs {
    use super::*;

    pub fn analyze_with_overrides(
        project: &LaravelProject,
        overrides: &FileOverrides,
    ) -> Result<ConfigReport, String> {
        let mappings = collect_psr4_mappings(&project.root)?;
        let provider_report = providers::analyze(project, &mappings)?;
        rust_php_configs::analyze(project, &provider_report.providers, overrides)
    }
}

pub mod controllers {
    use super::*;

    pub fn analyze_with_overrides(
        project: &LaravelProject,
        overrides: &FileOverrides,
    ) -> Result<ControllerReport, String> {
        let mappings = collect_psr4_mappings(&project.root)?;
        rust_php_controllers::analyze(project, &mappings, overrides)
    }

    pub fn resolve_route_target(
        report: &ControllerReport,
        action: &str,
    ) -> Option<RouteControllerTarget> {
        rust_php_controllers::resolve_route_target(report, action)
    }
}

pub mod routes {
    use super::*;

    pub fn analyze_with_overrides(
        project: &LaravelProject,
        overrides: &FileOverrides,
    ) -> Result<RouteReport, String> {
        let mappings = collect_psr4_mappings(&project.root)?;
        let provider_report = providers::analyze(project, &mappings)?;
        let middleware = analyze_middleware_raw(project, &provider_report.providers, overrides)?;
        let middleware_index = MiddlewareIndex::new()
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

        let mut report = rust_php_routes::analyze_raw(
            project,
            &provider_report.providers,
            overrides,
            &middleware_index,
        )?;

        let controller_report = rust_php_controllers::analyze(project, &mappings, overrides)?;
        for route in &mut report.routes {
            route.controller_target = route.action.as_deref().and_then(|action| {
                rust_php_controllers::resolve_route_target(&controller_report, action)
            });
        }

        Ok(report)
    }

    pub fn reindex_guard_reason(source: &[u8]) -> Option<&'static str> {
        rust_php_routes::reindex_guard_reason(source)
    }

    pub fn collect_registered_route_paths(
        project: &LaravelProject,
        overrides: &FileOverrides,
    ) -> Result<Vec<std::path::PathBuf>, String> {
        let mappings = collect_psr4_mappings(&project.root)?;
        let provider_report = providers::analyze(project, &mappings)?;
        rust_php_routes::collect_registered_route_paths(
            project,
            &provider_report.providers,
            overrides,
        )
    }
}

pub mod models {
    use super::*;
    use crate::types::ModelReport;

    pub fn analyze(project: &LaravelProject) -> Result<ModelReport, String> {
        let mappings = collect_psr4_mappings(&project.root)?;
        let provider_report = providers::analyze(project, &mappings)?;
        let migrations = rust_php_migrations::analyze(
            project,
            &provider_report.providers,
            &FileOverrides::default(),
        )?;
        rust_php_models::analyze(project, &mappings, &migrations.migrations)
    }
}

pub mod views {
    use super::*;

    pub fn analyze(project: &LaravelProject) -> Result<ViewReport, String> {
        analyze_with_overrides(project, &FileOverrides::default())
    }

    pub fn analyze_with_overrides(
        project: &LaravelProject,
        overrides: &FileOverrides,
    ) -> Result<ViewReport, String> {
        let mappings = collect_psr4_mappings(&project.root)?;
        let provider_report = providers::analyze(project, &mappings)?;
        let route_files =
            crate::analyzers::routes::collect_registered_route_paths(project, overrides)?;
        rust_php_views::analyze(
            project,
            &mappings,
            &provider_report.providers,
            &route_files,
            overrides,
        )
    }
}
