use std::cmp::Reverse;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::analyzers::{configs, controllers, models, routes, views};
use crate::php::env::load_env_entries_with;
use crate::project::LaravelProject;
use crate::types::{
    BladeComponentEntry, ColumnEntry, ConfigItem, ConfigReport, ControllerEntry,
    ControllerMethodEntry, ControllerReport, EnvItem, LivewireActionEntry, LivewireComponentEntry,
    ModelEntry, ModelReport, RouteEntry, RouteReport, ViewEntry, ViewReport, ViewVariable,
};
use rust_php_foundation::vendor as foundation_vendor;
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
    model_report: ModelReport,
    vendor_index: Arc<foundation_vendor::VendorClassIndex>,
    config_by_key: BTreeMap<String, Vec<usize>>,
    route_by_name: BTreeMap<String, Vec<usize>>,
    env_by_key: BTreeMap<String, Vec<usize>>,
    view_by_name: BTreeMap<String, Vec<usize>>,
    blade_component_by_name: BTreeMap<String, Vec<usize>>,
    livewire_component_by_name: BTreeMap<String, Vec<usize>>,
    public_asset_by_path: BTreeMap<String, Vec<usize>>,
    model_by_class: BTreeMap<String, Vec<usize>>,
}

impl ProjectIndex {
    pub fn build_with_overrides(
        project: &LaravelProject,
        overrides: &FileOverrides,
    ) -> Result<Self, String> {
        Self::build_with_overrides_and_vendor_index(
            project,
            overrides,
            Arc::new(foundation_vendor::VendorClassIndex::load(&project.root)),
        )
    }

    pub fn build_with_overrides_and_vendor_index(
        project: &LaravelProject,
        overrides: &FileOverrides,
        vendor_index: Arc<foundation_vendor::VendorClassIndex>,
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
        let model_report = models::analyze(project).unwrap_or_else(|_| ModelReport {
            project_name: project.name.clone(),
            project_root: project.root.clone(),
            model_count: 0,
            models: Vec::new(),
        });
        let mut config_by_key: BTreeMap<String, Vec<usize>> = BTreeMap::new();
        let mut route_by_name: BTreeMap<String, Vec<usize>> = BTreeMap::new();
        let mut env_by_key: BTreeMap<String, Vec<usize>> = BTreeMap::new();
        let mut view_by_name: BTreeMap<String, Vec<usize>> = BTreeMap::new();
        let mut public_asset_by_path: BTreeMap<String, Vec<usize>> = BTreeMap::new();
        let mut model_by_class: BTreeMap<String, Vec<usize>> = BTreeMap::new();

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
        let mut livewire_component_by_name: BTreeMap<String, Vec<usize>> = BTreeMap::new();
        for (index, component) in view_report.livewire_components.iter().enumerate() {
            livewire_component_by_name
                .entry(component.component.clone())
                .or_default()
                .push(index);
        }
        for indices in livewire_component_by_name.values_mut() {
            indices
                .sort_by_key(|&idx| livewire_component_rank(&view_report.livewire_components[idx]));
        }

        for (index, model) in model_report.models.iter().enumerate() {
            model_by_class
                .entry(model.class_name.clone())
                .or_default()
                .push(index);
        }

        Ok(Self {
            project_root: project.root.clone(),
            config_report,
            controller_report,
            route_report,
            env_items,
            view_report,
            public_asset_report,
            model_report,
            vendor_index,
            config_by_key,
            route_by_name,
            env_by_key,
            view_by_name,
            blade_component_by_name,
            livewire_component_by_name,
            public_asset_by_path,
            model_by_class,
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

    pub fn livewire_component_matches<'a>(
        &'a self,
        prefix: &str,
    ) -> Vec<&'a LivewireComponentEntry> {
        ranked_index_matches(self.livewire_component_by_name.iter(), prefix, |index| {
            &self.view_report.livewire_components[index]
        })
    }

    pub fn livewire_component_definitions<'a>(
        &'a self,
        name: &str,
    ) -> Vec<&'a LivewireComponentEntry> {
        self.livewire_component_by_name
            .get(name)
            .into_iter()
            .flat_map(|indices| {
                indices
                    .iter()
                    .map(|index| &self.view_report.livewire_components[*index])
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

    pub fn blade_variable_class_for_file(
        &self,
        file: &std::path::Path,
        var_name: &str,
    ) -> Option<String> {
        self.view_report
            .views
            .iter()
            .filter(|view| view.file == file)
            .flat_map(|view| view.props.iter().chain(view.variables.iter()))
            .chain(
                self.view_report
                    .livewire_components
                    .iter()
                    .filter(|c| c.view_file.as_deref() == Some(file))
                    .flat_map(|c| c.state.iter()),
            )
            .find(|v| v.name == var_name)
            .and_then(|v| v.class_name.clone())
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
            .chain(
                self.view_report
                    .livewire_components
                    .iter()
                    .filter(|component| component.view_file.as_deref() == Some(file))
                    .flat_map(|component| component.state.iter()),
            )
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

    pub fn livewire_state_for_file<'a>(
        &'a self,
        file: &std::path::Path,
        prefix: &str,
    ) -> Vec<&'a ViewVariable> {
        ranked_livewire_file_items(
            self.view_report
                .livewire_components
                .iter()
                .filter(|component| component.view_file.as_deref() == Some(file))
                .flat_map(|component| component.state.iter()),
            prefix,
            |variable| variable.name.as_str(),
        )
    }

    pub fn livewire_actions_for_file<'a>(
        &'a self,
        file: &std::path::Path,
        prefix: &str,
    ) -> Vec<&'a LivewireActionEntry> {
        ranked_livewire_file_items(
            self.view_report
                .livewire_components
                .iter()
                .filter(|component| component.view_file.as_deref() == Some(file))
                .flat_map(|component| component.actions.iter()),
            prefix,
            |action| action.name.as_str(),
        )
    }

    pub fn livewire_component_for_view_file<'a>(
        &'a self,
        file: &std::path::Path,
    ) -> Option<&'a LivewireComponentEntry> {
        self.view_report
            .livewire_components
            .iter()
            .find(|component| component.view_file.as_deref() == Some(file))
    }

    pub fn livewire_component_for_view_name<'a>(
        &'a self,
        view_name: &str,
    ) -> Option<&'a LivewireComponentEntry> {
        self.view_report
            .livewire_components
            .iter()
            .find(|component| component.view_name.as_deref() == Some(view_name))
    }

    pub fn routes_for_file<'a>(&'a self, file: &std::path::Path) -> Vec<&'a RouteEntry> {
        self.route_report
            .routes
            .iter()
            .filter(|route| route.file == file)
            .collect()
    }

    pub fn vendor_class_path(&self, fqn: &str) -> Option<&Path> {
        self.vendor_index.class_path(fqn)
    }

    pub fn vendor_chainable_methods(&self, fqn: &str) -> Vec<String> {
        self.vendor_index.collect_chainable_methods(fqn)
    }

    pub fn vendor_class_properties(&self, fqn: &str) -> Vec<String> {
        self.vendor_index.collect_class_properties(fqn)
    }

    pub fn vendor_class_properties_with_source(&self, fqn: &str) -> Vec<(String, String)> {
        self.vendor_index.collect_class_properties_with_source(fqn)
    }

    pub fn vendor_class_property_stubs(&self, fqn: &str) -> Vec<(String, String, String)> {
        self.vendor_index
            .collect_class_property_stubs_with_source(fqn)
    }

    pub fn model_columns_for_class<'a>(&'a self, class_name: &str) -> Vec<&'a ColumnEntry> {
        let normalized = class_name.trim_start_matches('\\');
        let short = normalized.rsplit('\\').next().unwrap_or(normalized);

        let indices = self
            .model_by_class
            .get(normalized)
            .or_else(|| self.model_by_class.get(short));

        indices
            .into_iter()
            .flat_map(|idxs| {
                idxs.iter()
                    .flat_map(|&i| self.model_report.models[i].columns.iter())
            })
            .collect()
    }

    pub fn model_for_class<'a>(&'a self, class_name: &str) -> Option<&'a ModelEntry> {
        let normalized = class_name.trim_start_matches('\\');
        let short = normalized.rsplit('\\').next().unwrap_or(normalized);

        let indices = self
            .model_by_class
            .get(normalized)
            .or_else(|| self.model_by_class.get(short))?;

        indices.first().map(|&i| &self.model_report.models[i])
    }
}

fn livewire_component_rank(component: &LivewireComponentEntry) -> usize {
    if component.kind.contains("class") {
        0
    } else if component.kind.contains("multi-file") {
        1
    } else {
        2
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

fn ranked_livewire_file_items<'a, T>(
    items: impl Iterator<Item = &'a T>,
    query: &str,
    label: impl Fn(&T) -> &str,
) -> Vec<&'a T> {
    let mut matches = items
        .filter_map(|item| {
            let item_label = label(item);
            let score = fuzzy_score(item_label, query)?;
            Some((score, item_label.len(), item_label.to_string(), item))
        })
        .collect::<Vec<_>>();

    matches.sort_by_key(|(score, len, item_label, _)| (Reverse(*score), *len, item_label.clone()));
    matches.dedup_by(|left, right| left.2 == right.2);
    matches.into_iter().map(|(_, _, _, item)| item).collect()
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
