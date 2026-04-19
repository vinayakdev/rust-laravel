mod chain;
mod collector;
mod context;
mod parser;

use collector::collect_registered_route_files;
use context::{build_middleware_index, RouteContext};
use parser::collect_routes_from_source;

use crate::analyzers::middleware;
use crate::project::LaravelProject;
use crate::types::RouteReport;
use std::fs;

pub fn analyze(project: &LaravelProject) -> Result<RouteReport, String> {
    let route_files = collect_registered_route_files(project)?;
    let middleware_index = build_middleware_index(&middleware::analyze(project)?);
    let mut routes = Vec::new();

    for registered in &route_files {
        let file = &registered.file;
        let source = fs::read(file)
            .map_err(|e| format!("failed to read {}: {e}", file.display()))?;
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

    Ok(RouteReport {
        project_name: project.name.clone(),
        project_root: project.root.clone(),
        route_count: routes.len(),
        routes,
    })
}
