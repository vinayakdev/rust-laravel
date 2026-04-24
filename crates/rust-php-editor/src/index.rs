use std::cmp::Reverse;
use std::collections::BTreeMap;
use std::path::PathBuf;

use crate::analyzers::{configs, controllers, routes, views};
use crate::php::env::load_env_entries_with;
use crate::project::LaravelProject;
use crate::types::{
    BladeComponentEntry, ConfigItem, ConfigReport, ControllerEntry, ControllerMethodEntry,
    ControllerReport, EnvItem, RouteEntry, RouteReport, ViewEntry, ViewReport, ViewVariable,
};
use rust_php_public::types::{PublicAssetEntry, PublicAssetReport};

use super::overrides::FileOverrides;

pub struct ProjectIndex {
    pub project_root: PathBuf,
    config_report: ConfigReport,
    controller_report: ControllerReport,
    route_report: RouteReport,
    env_items: Vec<EnvItem>,
    view_report: ViewReport,
    public_asset_report: PublicAssetReport,
    config_by_key: BTreeMap<String, Vec<usize>>,
    route_by_name: BTreeMap<String, Vec<usize>>,
    env_by_key: BTreeMap<String, Vec<usize>>,
    view_by_name: BTreeMap<String, Vec<usize>>,
    blade_component_by_name: BTreeMap<String, Vec<usize>>,
    public_asset_by_path: BTreeMap<String, Vec<usize>>,
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
        let view_report = views::analyze(project).unwrap_or_else(|_| ViewReport {
            project_name: project.name.clone(),
            project_root: project.root.clone(),
            view_count: 0,
            blade_component_count: 0,
            livewire_component_count: 0,
            missing_view_count: 0,
            views: Vec::new(),
            blade_components: Vec::new(),
            livewire_components: Vec::new(),
            missing_views: Vec::new(),
        });
        let public_asset_report =
            rust_php_public::analyze(project).unwrap_or_else(|_| PublicAssetReport {
                project_name: project.name.clone(),
                project_root: project.root.clone(),
                file_count: 0,
                usage_count: 0,
                assets: Vec::new(),
            });
        let mut config_by_key: BTreeMap<String, Vec<usize>> = BTreeMap::new();
        let mut route_by_name: BTreeMap<String, Vec<usize>> = BTreeMap::new();
        let mut env_by_key: BTreeMap<String, Vec<usize>> = BTreeMap::new();
        let mut view_by_name: BTreeMap<String, Vec<usize>> = BTreeMap::new();
        let mut public_asset_by_path: BTreeMap<String, Vec<usize>> = BTreeMap::new();

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

        for (index, view) in view_report.views.iter().enumerate() {
            view_by_name
                .entry(view.name.clone())
                .or_default()
                .push(index);
        }
        for (index, asset) in public_asset_report.assets.iter().enumerate() {
            public_asset_by_path
                .entry(asset.asset_path.clone())
                .or_default()
                .push(index);
        }

        let mut blade_component_by_name: BTreeMap<String, Vec<usize>> = BTreeMap::new();
        for (index, component) in view_report.blade_components.iter().enumerate() {
            blade_component_by_name
                .entry(component.component.clone())
                .or_default()
                .push(index);
        }
        for indices in blade_component_by_name.values_mut() {
            indices.sort_by_key(|&idx| {
                if view_report.blade_components[idx].kind.contains("class") {
                    0usize
                } else {
                    1usize
                }
            });
        }

        Ok(Self {
            project_root: project.root.clone(),
            config_report,
            controller_report,
            route_report,
            env_items,
            view_report,
            public_asset_report,
            config_by_key,
            route_by_name,
            env_by_key,
            view_by_name,
            blade_component_by_name,
            public_asset_by_path,
        })
    }

    pub fn config_matches<'a>(&'a self, prefix: &str) -> Vec<&'a ConfigItem> {
        ranked_index_matches(self.config_by_key.iter(), prefix, |index| {
            &self.config_report.items[index]
        })
    }

    pub fn route_matches<'a>(&'a self, prefix: &str) -> Vec<&'a RouteEntry> {
        ranked_index_matches(self.route_by_name.iter(), prefix, |index| {
            &self.route_report.routes[index]
        })
    }

    pub fn env_matches<'a>(&'a self, prefix: &str) -> Vec<&'a EnvItem> {
        ranked_index_matches(self.env_by_key.iter(), prefix, |index| {
            &self.env_items[index]
        })
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
        let mut matches = self
            .controller_report
            .controllers
            .iter()
            .filter_map(|controller| {
                let short_name = controller
                    .fqn
                    .rsplit('\\')
                    .next()
                    .unwrap_or(controller.fqn.as_str());
                let score = fuzzy_score(&controller.class_name, prefix)
                    .max(fuzzy_score(&controller.fqn, prefix))
                    .max(fuzzy_score(short_name, prefix))?;
                Some((
                    score,
                    controller.class_name.len(),
                    controller.class_name.as_str(),
                    controller,
                ))
            })
            .collect::<Vec<_>>();

        matches.sort_by_key(|(score, len, label, _)| (Reverse(*score), *len, *label));
        matches
            .into_iter()
            .map(|(_, _, _, controller)| controller)
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
        let mut matches = controller_candidates(&self.controller_report, controller)
            .into_iter()
            .flat_map(|controller| {
                controller
                    .methods
                    .iter()
                    .filter(move |method| method.accessible_from_route)
                    .filter_map(move |method| {
                        let score = fuzzy_score(&method.name, prefix)?;
                        Some((
                            score,
                            method.name.len(),
                            method.name.as_str(),
                            controller,
                            method,
                        ))
                    })
            })
            .collect::<Vec<_>>();

        matches.sort_by_key(|(score, len, label, _, _)| (Reverse(*score), *len, *label));
        matches
            .into_iter()
            .map(|(_, _, _, controller, method)| (controller, method))
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

    pub fn view_matches<'a>(&'a self, prefix: &str) -> Vec<&'a ViewEntry> {
        ranked_index_matches(self.view_by_name.iter(), prefix, |index| {
            &self.view_report.views[index]
        })
    }

    pub fn view_definitions<'a>(&'a self, name: &str) -> Vec<&'a ViewEntry> {
        self.view_by_name
            .get(name)
            .into_iter()
            .flat_map(|indices| indices.iter().map(|index| &self.view_report.views[*index]))
            .collect()
    }

    pub fn blade_component_matches<'a>(&'a self, prefix: &str) -> Vec<&'a BladeComponentEntry> {
        ranked_index_matches(self.blade_component_by_name.iter(), prefix, |index| {
            &self.view_report.blade_components[index]
        })
    }

    pub fn blade_component_definitions<'a>(&'a self, name: &str) -> Vec<&'a BladeComponentEntry> {
        self.blade_component_by_name
            .get(name)
            .into_iter()
            .flat_map(|indices| {
                indices
                    .iter()
                    .map(|index| &self.view_report.blade_components[*index])
            })
            .collect()
    }

    pub fn public_asset_matches<'a>(&'a self, prefix: &str) -> Vec<&'a PublicAssetEntry> {
        ranked_index_matches(self.public_asset_by_path.iter(), prefix, |index| {
            &self.public_asset_report.assets[index]
        })
    }

    pub fn public_asset_definitions<'a>(&'a self, asset_path: &str) -> Vec<&'a PublicAssetEntry> {
        self.public_asset_by_path
            .get(asset_path)
            .into_iter()
            .flat_map(|indices| {
                indices
                    .iter()
                    .map(|index| &self.public_asset_report.assets[*index])
            })
            .collect()
    }

    pub fn blade_variables_for_file<'a>(
        &'a self,
        file: &std::path::Path,
        prefix: &str,
    ) -> Vec<&'a ViewVariable> {
        let mut matches = self
            .view_report
            .views
            .iter()
            .filter(|view| view.file == file)
            .flat_map(|view| view.props.iter().chain(view.variables.iter()))
            .filter_map(|variable| {
                let score = fuzzy_score(&variable.name, prefix)?;
                Some((score, variable.name.len(), variable.name.as_str(), variable))
            })
            .collect::<Vec<_>>();

        matches.sort_by_key(|(score, len, label, _)| (Reverse(*score), *len, *label));
        matches.dedup_by(|left, right| left.2 == right.2);
        matches
            .into_iter()
            .map(|(_, _, _, variable)| variable)
            .collect()
    }

    pub fn routes_for_file<'a>(&'a self, file: &std::path::Path) -> Vec<&'a RouteEntry> {
        self.route_report
            .routes
            .iter()
            .filter(|route| route.file == file)
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

fn ranked_index_matches<'a, T, F>(
    entries: impl Iterator<Item = (&'a String, &'a Vec<usize>)>,
    query: &str,
    resolve: F,
) -> Vec<&'a T>
where
    F: Fn(usize) -> &'a T,
{
    let mut matches = entries
        .filter_map(|(key, indices)| {
            let score = fuzzy_score(key, query)?;
            Some((score, key.len(), key.as_str(), indices))
        })
        .collect::<Vec<_>>();

    matches.sort_by_key(|(score, len, label, _)| (Reverse(*score), *len, *label));

    matches
        .into_iter()
        .flat_map(|(_, _, _, indices)| indices.iter().copied())
        .map(resolve)
        .collect()
}

pub(crate) fn fuzzy_score(candidate: &str, query: &str) -> Option<u32> {
    if query.is_empty() {
        return Some(1);
    }

    if candidate == query {
        return Some(10_000);
    }

    if candidate.starts_with(query) {
        return Some(9_000u32.saturating_sub(candidate.len() as u32));
    }

    if candidate.contains(query) {
        return Some(7_000u32.saturating_sub(candidate.len() as u32));
    }

    let candidate_chars = candidate.chars().collect::<Vec<_>>();
    let query_chars = query.chars().collect::<Vec<_>>();
    let mut query_index = 0usize;
    let mut score = 0u32;
    let mut last_match = None::<usize>;

    for (index, ch) in candidate_chars.iter().enumerate() {
        if query_index >= query_chars.len() {
            break;
        }

        if !ch.eq_ignore_ascii_case(&query_chars[query_index]) {
            continue;
        }

        score += 10;

        if index == 0 {
            score += 20;
        } else {
            let previous = candidate_chars[index - 1];
            if matches!(previous, '.' | '_' | '-' | '\\' | '/' | ':' | '@') {
                score += 18;
            }
        }

        if let Some(previous_match) = last_match {
            if index == previous_match + 1 {
                score += 14;
            }
        }

        last_match = Some(index);
        query_index += 1;
    }

    if query_index != query_chars.len() {
        return None;
    }

    Some(score.saturating_sub(candidate.len() as u32))
}
