mod chain;
mod collector;
mod context;
mod parser;

use collector::collect_registered_route_files;
pub use context::MiddlewareIndex;
use context::RouteContext;
use parser::{
    collect_routes_from_source, has_unsafe_string_adjacency, reset_include_tracking,
    source_can_use_full_parse,
};
use rust_php_foundation::overrides::FileOverrides;
use rust_php_foundation::project::LaravelProject;
use rust_php_foundation::types::ProviderEntry;
use std::collections::BTreeSet;
use std::path::PathBuf;

use crate::types::RouteReport;

pub fn analyze_raw(
    project: &LaravelProject,
    providers: &[ProviderEntry],
    overrides: &FileOverrides,
    middleware_index: &MiddlewareIndex,
) -> Result<RouteReport, String> {
    reset_include_tracking();
    let route_files = collect_registered_route_files(project, providers, overrides)?;
    let mut routes = Vec::new();

    for registered in &route_files {
        let file = &registered.file;
        let source = overrides.get_bytes(file).map_or_else(
            || {
                std::fs::read(file)
                    .map_err(|error| format!("failed to read {}: {error}", file.display()))
            },
            Ok,
        )?;
        collect_routes_from_source(
            &source,
            &project.root,
            file,
            &registered.registration,
            1,
            &RouteContext::default(),
            middleware_index,
            &mut routes,
        );
    }

    routes.sort_by(|l, r| {
        l.file
            .cmp(&r.file)
            .then(l.line.cmp(&r.line))
            .then(l.uri.cmp(&r.uri))
    });

    Ok(RouteReport {
        project_name: project.name.clone(),
        project_root: project.root.clone(),
        route_count: routes.len(),
        routes,
    })
}

pub fn collect_registered_route_paths(
    project: &LaravelProject,
    providers: &[ProviderEntry],
    overrides: &FileOverrides,
) -> Result<Vec<PathBuf>, String> {
    let mut seen = BTreeSet::new();
    let mut files = Vec::new();

    for registered in collect_registered_route_files(project, providers, overrides)? {
        if seen.insert(registered.file.clone()) {
            files.push(registered.file);
        }
    }

    Ok(files)
}

pub fn reindex_guard_reason(source: &[u8]) -> Option<&'static str> {
    if has_unsafe_string_adjacency(source) {
        return Some("unsafe-string-adjacency");
    }

    if !source_can_use_full_parse(source) {
        return Some("unbalanced-route-source");
    }

    None
}
