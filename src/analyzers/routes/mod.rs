mod chain;
mod collector;
mod context;
mod parser;

use collector::collect_registered_route_files;
use context::{RouteContext, build_middleware_index};
use parser::{
    collect_routes_from_source, has_unsafe_string_adjacency, reset_include_tracking,
    source_can_use_full_parse,
};

use crate::analyzers::{controllers, middleware};
use crate::lsp::overrides::FileOverrides;
use crate::project::LaravelProject;
use crate::types::RouteReport;
use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;

pub fn analyze(project: &LaravelProject) -> Result<RouteReport, String> {
    analyze_with_overrides(project, &FileOverrides::default())
}

pub fn analyze_with_overrides(
    project: &LaravelProject,
    overrides: &FileOverrides,
) -> Result<RouteReport, String> {
    reset_include_tracking();
    let route_files = collect_registered_route_files(project)?;
    let middleware_index = build_middleware_index(&middleware::analyze(project)?);
    let mut routes = Vec::new();

    for registered in &route_files {
        let file = &registered.file;
        let source = overrides.get_bytes(file).map_or_else(
            || fs::read(file).map_err(|e| format!("failed to read {}: {e}", file.display())),
            Ok,
        )?;
        collect_routes_from_source(
            &source,
            &project.root,
            file,
            &registered.registration,
            1,
            &RouteContext::default(),
            &middleware_index,
            &mut routes,
        );
    }

    routes.sort_by(|l, r| {
        l.file
            .cmp(&r.file)
            .then(l.line.cmp(&r.line))
            .then(l.uri.cmp(&r.uri))
    });

    let controller_report = controllers::analyze(project)?;
    for route in &mut routes {
        route.controller_target = route
            .action
            .as_deref()
            .and_then(|action| controllers::resolve_route_target(&controller_report, action));
    }

    Ok(RouteReport {
        project_name: project.name.clone(),
        project_root: project.root.clone(),
        route_count: routes.len(),
        routes,
    })
}

pub(crate) fn collect_registered_route_paths(
    project: &LaravelProject,
) -> Result<Vec<PathBuf>, String> {
    let mut seen = BTreeSet::new();
    let mut files = Vec::new();

    for registered in collect_registered_route_files(project)? {
        if seen.insert(registered.file.clone()) {
            files.push(registered.file);
        }
    }

    Ok(files)
}

pub(crate) fn reindex_guard_reason(source: &[u8]) -> Option<&'static str> {
    if has_unsafe_string_adjacency(source) {
        return Some("unsafe-string-adjacency");
    }

    if !source_can_use_full_parse(source) {
        return Some("unbalanced-route-source");
    }

    None
}
