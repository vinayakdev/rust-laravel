use rust_php_foundation::discovery::providers as provider_discovery;
use rust_php_foundation::overrides::FileOverrides;
use rust_php_foundation::php::psr4::collect_psr4_mappings;
use rust_php_foundation::project::LaravelProject;
use rust_php_middleware::analyze as analyze_middleware_raw;
use rust_php_routes::routes::MiddlewareIndex;

pub mod providers {
    use super::*;

    pub fn analyze(
        project: &LaravelProject,
    ) -> Result<rust_php_foundation::types::ProviderReport, String> {
        let mappings = collect_psr4_mappings(&project.root)?;
        provider_discovery::analyze(project, &mappings)
    }
}

pub mod configs {
    use super::*;

    pub fn analyze(
        project: &LaravelProject,
    ) -> Result<rust_php_configs::types::ConfigReport, String> {
        let mappings = collect_psr4_mappings(&project.root)?;
        let provider_report = provider_discovery::analyze(project, &mappings)?;
        rust_php_configs::analyze(
            project,
            &provider_report.providers,
            &FileOverrides::default(),
        )
    }
}

pub mod controllers {
    use super::*;

    pub fn analyze(
        project: &LaravelProject,
    ) -> Result<rust_php_controllers::types::ControllerReport, String> {
        let mappings = collect_psr4_mappings(&project.root)?;
        rust_php_controllers::analyze(project, &mappings, &FileOverrides::default())
    }
}

pub mod middleware {
    use super::*;

    pub fn analyze(
        project: &LaravelProject,
    ) -> Result<rust_php_middleware::types::MiddlewareReport, String> {
        let mappings = collect_psr4_mappings(&project.root)?;
        let provider_report = provider_discovery::analyze(project, &mappings)?;
        analyze_middleware_raw(
            project,
            &provider_report.providers,
            &FileOverrides::default(),
        )
    }
}

pub mod routes {
    use super::*;

    pub fn analyze(
        project: &LaravelProject,
    ) -> Result<rust_php_routes::types::RouteReport, String> {
        let mappings = collect_psr4_mappings(&project.root)?;
        let provider_report = provider_discovery::analyze(project, &mappings)?;
        let middleware = analyze_middleware_raw(
            project,
            &provider_report.providers,
            &FileOverrides::default(),
        )?;
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
            &FileOverrides::default(),
            &middleware_index,
        )?;

        let controller_report =
            rust_php_controllers::analyze(project, &mappings, &FileOverrides::default())?;
        for route in &mut report.routes {
            route.controller_target = route.action.as_deref().and_then(|action| {
                rust_php_controllers::resolve_route_target(&controller_report, action)
            });
        }

        Ok(report)
    }
}

pub mod views {
    use super::*;

    pub fn analyze(project: &LaravelProject) -> Result<rust_php_views::types::ViewReport, String> {
        let mappings = collect_psr4_mappings(&project.root)?;
        let provider_report = provider_discovery::analyze(project, &mappings)?;
        let route_files = rust_php_routes::collect_registered_route_paths(
            project,
            &provider_report.providers,
            &FileOverrides::default(),
        )?;
        rust_php_views::analyze(
            project,
            &mappings,
            &provider_report.providers,
            &route_files,
            &FileOverrides::default(),
        )
    }
}

pub mod migrations {
    use super::*;

    pub fn analyze(
        project: &LaravelProject,
    ) -> Result<rust_php_migrations::types::MigrationReport, String> {
        let mappings = collect_psr4_mappings(&project.root)?;
        let provider_report = provider_discovery::analyze(project, &mappings)?;
        rust_php_migrations::analyze(
            project,
            &provider_report.providers,
            &FileOverrides::default(),
        )
    }
}

pub mod models {
    use super::*;

    pub fn analyze(
        project: &LaravelProject,
    ) -> Result<rust_php_models::types::ModelReport, String> {
        let mappings = collect_psr4_mappings(&project.root)?;
        let provider_report = provider_discovery::analyze(project, &mappings)?;
        let migration_report = rust_php_migrations::analyze(
            project,
            &provider_report.providers,
            &FileOverrides::default(),
        )?;
        rust_php_models::analyze(project, &mappings, &migration_report.migrations)
    }
}
