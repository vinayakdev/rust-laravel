use std::collections::BTreeMap;
use std::path::PathBuf;

use crate::analyzers::{configs, routes};
use crate::project::LaravelProject;
use crate::types::{ConfigItem, ConfigReport, RouteEntry, RouteReport};

use super::overrides::FileOverrides;

pub struct ProjectIndex {
    pub project_root: PathBuf,
    config_report: ConfigReport,
    route_report: RouteReport,
    config_by_key: BTreeMap<String, Vec<usize>>,
    route_by_name: BTreeMap<String, Vec<usize>>,
}

impl ProjectIndex {
    pub fn build_with_overrides(
        project: &LaravelProject,
        overrides: &FileOverrides,
    ) -> Result<Self, String> {
        let config_report = configs::analyze_with_overrides(project, overrides)?;
        let route_report = routes::analyze_with_overrides(project, overrides)?;
        let mut config_by_key: BTreeMap<String, Vec<usize>> = BTreeMap::new();
        let mut route_by_name: BTreeMap<String, Vec<usize>> = BTreeMap::new();

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

        Ok(Self {
            project_root: project.root.clone(),
            config_report,
            route_report,
            config_by_key,
            route_by_name,
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
}
