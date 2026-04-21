use std::collections::BTreeMap;
use std::path::PathBuf;

use crate::analyzers::{configs, controllers, routes};
use crate::php::env::load_env_entries_with;
use crate::project::LaravelProject;
use crate::types::{
    ConfigItem, ConfigReport, ControllerEntry, ControllerMethodEntry, ControllerReport, EnvItem,
    RouteEntry, RouteReport,
};

use super::overrides::FileOverrides;

pub struct ProjectIndex {
    pub project_root: PathBuf,
    config_report: ConfigReport,
    controller_report: ControllerReport,
    route_report: RouteReport,
    env_items: Vec<EnvItem>,
    config_by_key: BTreeMap<String, Vec<usize>>,
    route_by_name: BTreeMap<String, Vec<usize>>,
    env_by_key: BTreeMap<String, Vec<usize>>,
}

impl ProjectIndex {
    pub fn build_with_overrides(
        project: &LaravelProject,
        overrides: &FileOverrides,
    ) -> Result<Self, String> {
        let config_report = configs::analyze_with_overrides(project, overrides)?;
        let controller_report = controllers::analyze_with_overrides(project, overrides)?;
        let route_report = routes::analyze_with_overrides(project, overrides)?;
        let env_items = load_env_entries_with(&project.root, |path| overrides.get_string(path))?;
        let mut config_by_key: BTreeMap<String, Vec<usize>> = BTreeMap::new();
        let mut route_by_name: BTreeMap<String, Vec<usize>> = BTreeMap::new();
        let mut env_by_key: BTreeMap<String, Vec<usize>> = BTreeMap::new();

        for (index, item) in config_report.items.iter().enumerate() {
            config_by_key
                .entry(item.key.clone())
                .or_default()
                .push(index);
        }

        for (index, route) in route_report.routes.iter().enumerate() {
            if let Some(name) = &route.name {
                route_by_name.entry(name.clone()).or_default().push(index);
            }
        }

        for (index, item) in env_items.iter().enumerate() {
            env_by_key.entry(item.key.clone()).or_default().push(index);
        }

        Ok(Self {
            project_root: project.root.clone(),
            config_report,
            controller_report,
            route_report,
            env_items,
            config_by_key,
            route_by_name,
            env_by_key,
        })
    }

    pub fn config_matches<'a>(&'a self, prefix: &str) -> Vec<&'a ConfigItem> {
        self.config_by_key
            .iter()
            .filter(|(key, _)| key.starts_with(prefix))
            .flat_map(|(_, indices)| {
                indices
                    .iter()
                    .map(|index| &self.config_report.items[*index])
            })
            .collect()
    }

    pub fn route_matches<'a>(&'a self, prefix: &str) -> Vec<&'a RouteEntry> {
        self.route_by_name
            .iter()
            .filter(|(key, _)| key.starts_with(prefix))
            .flat_map(|(_, indices)| {
                indices
                    .iter()
                    .map(|index| &self.route_report.routes[*index])
            })
            .collect()
    }

    pub fn env_matches<'a>(&'a self, prefix: &str) -> Vec<&'a EnvItem> {
        self.env_by_key
            .iter()
            .filter(|(key, _)| key.starts_with(prefix))
            .flat_map(|(_, indices)| indices.iter().map(|index| &self.env_items[*index]))
            .collect()
    }

    pub fn config_definitions<'a>(&'a self, key: &str) -> Vec<&'a ConfigItem> {
        self.config_by_key
            .get(key)
            .into_iter()
            .flat_map(|indices| {
                indices
                    .iter()
                    .map(|index| &self.config_report.items[*index])
            })
            .collect()
    }

    pub fn route_definitions<'a>(&'a self, name: &str) -> Vec<&'a RouteEntry> {
        self.route_by_name
            .get(name)
            .into_iter()
            .flat_map(|indices| {
                indices
                    .iter()
                    .map(|index| &self.route_report.routes[*index])
            })
            .collect()
    }

    pub fn env_definitions<'a>(&'a self, key: &str) -> Vec<&'a EnvItem> {
        self.env_by_key
            .get(key)
            .into_iter()
            .flat_map(|indices| indices.iter().map(|index| &self.env_items[*index]))
            .collect()
    }

    pub fn controller_matches<'a>(&'a self, prefix: &str) -> Vec<&'a ControllerEntry> {
        self.controller_report
            .controllers
            .iter()
            .filter(|controller| {
                controller.class_name.starts_with(prefix)
                    || controller.fqn.starts_with(prefix)
                    || controller
                        .fqn
                        .rsplit('\\')
                        .next()
                        .unwrap_or(controller.fqn.as_str())
                        .starts_with(prefix)
            })
            .collect()
    }

    pub fn controller_definitions<'a>(&'a self, controller: &str) -> Vec<&'a ControllerEntry> {
        controller_candidates(&self.controller_report, controller)
    }

    pub fn controller_methods<'a>(
        &'a self,
        controller: &str,
        prefix: &str,
    ) -> Vec<(&'a ControllerEntry, &'a ControllerMethodEntry)> {
        controller_candidates(&self.controller_report, controller)
            .into_iter()
            .flat_map(|controller| {
                controller
                    .methods
                    .iter()
                    .filter(move |method| {
                        method.accessible_from_route && method.name.starts_with(prefix)
                    })
                    .map(move |method| (controller, method))
            })
            .collect()
    }

    pub fn controller_method_definitions<'a>(
        &'a self,
        controller: &str,
        method: &str,
    ) -> Vec<(&'a ControllerEntry, &'a ControllerMethodEntry)> {
        controller_candidates(&self.controller_report, controller)
            .into_iter()
            .flat_map(|controller| {
                controller
                    .methods
                    .iter()
                    .filter(move |item| item.name == method)
                    .map(move |item| (controller, item))
            })
            .collect()
    }
}

fn controller_candidates<'a>(
    report: &'a ControllerReport,
    controller: &str,
) -> Vec<&'a ControllerEntry> {
    let normalized = controller.trim_start_matches('\\');
    let short_name = normalized.rsplit('\\').next().unwrap_or(normalized);

    let exact_fqn = report
        .controllers
        .iter()
        .filter(|entry| entry.fqn == normalized)
        .collect::<Vec<_>>();
    if !exact_fqn.is_empty() {
        return exact_fqn;
    }

    let exact_short = report
        .controllers
        .iter()
        .filter(|entry| entry.class_name == short_name)
        .collect::<Vec<_>>();
    if !exact_short.is_empty() {
        return exact_short;
    }

    report
        .controllers
        .iter()
        .filter(|entry| entry.fqn.ends_with(&format!("\\{normalized}")))
        .collect()
}
