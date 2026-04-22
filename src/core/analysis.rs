use std::path::Path;
use std::sync::OnceLock;

use crate::analyzers::{
    configs, controllers, middleware, migrations, models, providers, routes, views,
};
use crate::php::psr4::{Psr4Mapping, collect_psr4_mappings};
use crate::project::LaravelProject;
use crate::types::{
    ConfigReport, ControllerReport, MiddlewareReport, MigrationReport, ModelReport, ProviderReport,
    RouteReport, ViewReport,
};

use super::overrides::FileOverrides;

pub struct ProjectAnalysis {
    project: LaravelProject,
    overrides: FileOverrides,
    psr4_mappings: OnceLock<Result<Vec<Psr4Mapping>, String>>,
    provider_report: OnceLock<Result<ProviderReport, String>>,
    config_report: OnceLock<Result<ConfigReport, String>>,
    controller_report: OnceLock<Result<ControllerReport, String>>,
    route_report: OnceLock<Result<RouteReport, String>>,
    middleware_report: OnceLock<Result<MiddlewareReport, String>>,
    migration_report: OnceLock<Result<MigrationReport, String>>,
    model_report: OnceLock<Result<ModelReport, String>>,
    view_report: OnceLock<Result<ViewReport, String>>,
}

impl ProjectAnalysis {
    pub fn from_project(project: &LaravelProject) -> Self {
        Self::new(project, FileOverrides::default())
    }

    pub fn new(project: &LaravelProject, overrides: FileOverrides) -> Self {
        Self {
            project: project.clone(),
            overrides,
            psr4_mappings: OnceLock::new(),
            provider_report: OnceLock::new(),
            config_report: OnceLock::new(),
            controller_report: OnceLock::new(),
            route_report: OnceLock::new(),
            middleware_report: OnceLock::new(),
            migration_report: OnceLock::new(),
            model_report: OnceLock::new(),
            view_report: OnceLock::new(),
        }
    }

    pub fn project(&self) -> &LaravelProject {
        &self.project
    }

    pub fn overrides(&self) -> &FileOverrides {
        &self.overrides
    }

    pub fn read_string(&self, path: &Path) -> Result<String, String> {
        self.overrides.get_string(path).map_or_else(
            || {
                std::fs::read_to_string(path)
                    .map_err(|error| format!("failed to read {}: {error}", path.display()))
            },
            Ok,
        )
    }

    pub fn read_bytes(&self, path: &Path) -> Result<Vec<u8>, String> {
        self.overrides.get_bytes(path).map_or_else(
            || {
                std::fs::read(path)
                    .map_err(|error| format!("failed to read {}: {error}", path.display()))
            },
            Ok,
        )
    }

    pub fn psr4_mappings(&self) -> Result<&[Psr4Mapping], String> {
        cached(&self.psr4_mappings, || {
            collect_psr4_mappings(&self.project.root)
        })
        .map(Vec::as_slice)
    }

    pub fn providers(&self) -> Result<&ProviderReport, String> {
        cached(&self.provider_report, || {
            providers::analyze_with_context(self)
        })
    }

    pub fn configs(&self) -> Result<&ConfigReport, String> {
        cached(&self.config_report, || configs::analyze_with_context(self))
    }

    pub fn controllers(&self) -> Result<&ControllerReport, String> {
        cached(&self.controller_report, || {
            controllers::analyze_with_context(self)
        })
    }

    pub fn routes(&self) -> Result<&RouteReport, String> {
        cached(&self.route_report, || routes::analyze_with_context(self))
    }

    pub fn middleware(&self) -> Result<&MiddlewareReport, String> {
        cached(&self.middleware_report, || {
            middleware::analyze_with_context(self)
        })
    }

    pub fn migrations(&self) -> Result<&MigrationReport, String> {
        cached(&self.migration_report, || {
            migrations::analyze_with_context(self)
        })
    }

    pub fn models(&self) -> Result<&ModelReport, String> {
        cached(&self.model_report, || models::analyze_with_context(self))
    }

    pub fn views(&self) -> Result<&ViewReport, String> {
        cached(&self.view_report, || views::analyze_with_context(self))
    }
}

fn cached<T>(
    slot: &OnceLock<Result<T, String>>,
    init: impl FnOnce() -> Result<T, String>,
) -> Result<&T, String> {
    let result = slot.get_or_init(init);
    result.as_ref().map_err(|error| error.clone())
}
