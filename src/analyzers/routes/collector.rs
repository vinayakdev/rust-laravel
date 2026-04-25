use bumpalo::Bump;
use php_parser::ast::{Expr, ExprId};
use php_parser::lexer::Lexer;
use php_parser::parser::Parser;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use crate::analyzers::routes::chain::{ChainOp, flatten_route_chain};
use crate::core::analysis::ProjectAnalysis;
use crate::php::ast::{expr_name, expr_to_path, strip_root};
use crate::php::walk::walk_stmts;
use crate::project::LaravelProject;
use crate::types::{ProviderEntry, RouteRegistration};

#[derive(Clone)]
pub(crate) struct RegisteredRouteFile {
    pub(crate) file: PathBuf,
    pub(crate) registration: RouteRegistration,
}

pub(crate) fn collect_registered_route_files(
    context: &ProjectAnalysis,
) -> Result<Vec<RegisteredRouteFile>, String> {
    let direct_files = collect_direct_route_files(context)?;
    let discovered = discover_provider_route_files(context)?;
    let mut by_file: BTreeMap<PathBuf, Vec<RouteRegistration>> = BTreeMap::new();

    for registered in discovered {
        if registered.file.is_file() {
            by_file
                .entry(registered.file)
                .or_default()
                .push(registered.registration);
        }
    }

    let direct_file_set: BTreeSet<PathBuf> = direct_files.iter().cloned().collect();
    let mut all_files = Vec::new();

    for file in &direct_files {
        if let Some(registrations) = by_file.get(file) {
            for registration in registrations {
                all_files.push(RegisteredRouteFile {
                    file: file.clone(),
                    registration: registration.clone(),
                });
            }
        } else {
            all_files.push(RegisteredRouteFile {
                file: file.clone(),
                registration: default_route_registration(&context.project().root, file),
            });
        }
    }

    for (file, registrations) in by_file {
        if direct_file_set.contains(&file) {
            continue;
        }
        for registration in registrations {
            all_files.push(RegisteredRouteFile {
                file: file.clone(),
                registration,
            });
        }
    }

    all_files.sort_by(|l, r| {
        l.file
            .cmp(&r.file)
            .then(l.registration.declared_in.cmp(&r.registration.declared_in))
            .then(l.registration.line.cmp(&r.registration.line))
    });

    Ok(all_files)
}

fn has_explicit_route_registrations(context: &ProjectAnalysis) -> Result<bool, String> {
    let bootstrap_app = context.project().root.join("bootstrap/app.php");
    if bootstrap_app.is_file()
        && !discover_bootstrap_route_files(context.project(), &bootstrap_app)?.is_empty()
    {
        return Ok(true);
    }

    Ok(!discover_provider_route_files(context)?.is_empty())
}

fn collect_direct_route_files(context: &ProjectAnalysis) -> Result<Vec<PathBuf>, String> {
    let bootstrap_app = context.project().root.join("bootstrap/app.php");
    if bootstrap_app.is_file() {
        let bootstrap_files = discover_bootstrap_route_files(context.project(), &bootstrap_app)?;
        if !bootstrap_files.is_empty() {
            return Ok(bootstrap_files
                .into_iter()
                .map(|registered| registered.file)
                .collect());
        }
    }

    if has_explicit_route_registrations(context)? {
        return Ok(Vec::new());
    }

    let routes_dir = context.project().root.join("routes");
    Ok(collect_php_files(&routes_dir))
}

fn discover_bootstrap_route_files(
    project: &LaravelProject,
    bootstrap_file: &Path,
) -> Result<Vec<RegisteredRouteFile>, String> {
    let source = fs::read(bootstrap_file)
        .map_err(|e| format!("failed to read {}: {e}", bootstrap_file.display()))?;
    let arena = Bump::new();
    let lexer = Lexer::new(&source);
    let mut parser = Parser::new(lexer, &arena);
    let program = parser.parse_program();
    if !program.errors.is_empty() {
        return Ok(Vec::new());
    }

    let mut files = Vec::new();
    walk_stmts(program.statements, false, &mut |expr| {
        visit_bootstrap_expr(expr, &source, project, bootstrap_file, &mut files);
    });

    files.sort_by(|l, r| {
        l.file
            .cmp(&r.file)
            .then(l.registration.line.cmp(&r.registration.line))
            .then(l.registration.column.cmp(&r.registration.column))
    });
    files.dedup_by(|l, r| l.file == r.file && l.registration.line == r.registration.line);

    Ok(files)
}

fn discover_provider_route_files(
    context: &ProjectAnalysis,
) -> Result<Vec<RegisteredRouteFile>, String> {
    let provider_report = context.providers()?;
    let mut files = Vec::new();
    let mut seen_sources = BTreeSet::new();

    for provider in &provider_report.providers {
        let Some(relative_source_file) = provider.source_file.as_ref() else {
            continue;
        };
        if !provider.source_available {
            continue;
        }
        if !seen_sources.insert((
            provider.provider_class.clone(),
            relative_source_file.clone(),
        )) {
            continue;
        }

        let source_file = context.project().root.join(relative_source_file);
        let source = context.read_bytes(&source_file)?;

        files.extend(extract_provider_route_files(
            context.project(),
            &provider,
            &source_file,
            &source,
        ));
    }

    Ok(files)
}

fn extract_provider_route_files(
    project: &LaravelProject,
    provider: &ProviderEntry,
    provider_file: &Path,
    source: &[u8],
) -> Vec<RegisteredRouteFile> {
    let arena = Bump::new();
    let lexer = Lexer::new(source);
    let mut parser = Parser::new(lexer, &arena);
    let program = parser.parse_program();
    if !program.errors.is_empty() {
        return Vec::new();
    }

    let mut files = Vec::new();
    // Providers have loadRoutesFrom inside class methods, so include class members.
    walk_stmts(program.statements, true, &mut |expr| {
        visit_provider_expr(expr, source, project, provider, provider_file, &mut files);
    });
    files
}

fn visit_provider_expr(
    expr: ExprId<'_>,
    source: &[u8],
    project: &LaravelProject,
    provider: &ProviderEntry,
    provider_file: &Path,
    files: &mut Vec<RegisteredRouteFile>,
) {
    match expr {
        Expr::MethodCall { method, args, .. } => {
            let method_name = expr_name(method, source).unwrap_or_default();
            if method_name == "loadRoutesFrom" {
                if let Some(path_expr) = args.first().map(|a| a.value) {
                    if let Some(route_file) =
                        expr_to_path(path_expr, source, &project.root, provider_file)
                    {
                        let line_info = path_expr.span().line_info(source);
                        let line = line_info.map_or(provider.line, |i| i.line);
                        let column = line_info.map_or(provider.column, |i| i.column);
                        files.push(RegisteredRouteFile {
                            file: route_file,
                            registration: RouteRegistration {
                                kind: "provider-loadRoutesFrom".to_string(),
                                declared_in: provider
                                    .source_file
                                    .clone()
                                    .unwrap_or_else(|| strip_root(&project.root, provider_file)),
                                line,
                                column,
                                provider_class: Some(provider.provider_class.clone()),
                            },
                        });
                    }
                }
            } else if method_name == "group" {
                if let Some(route_file) =
                    extract_grouped_route_file(expr, source, project, provider_file)
                {
                    let line_info = expr.span().line_info(source);
                    files.push(RegisteredRouteFile {
                        file: route_file,
                        registration: RouteRegistration {
                            kind: "provider-route-group".to_string(),
                            declared_in: provider
                                .source_file
                                .clone()
                                .unwrap_or_else(|| strip_root(&project.root, provider_file)),
                            line: line_info.map_or(provider.line, |i| i.line),
                            column: line_info.map_or(provider.column, |i| i.column),
                            provider_class: Some(provider.provider_class.clone()),
                        },
                    });
                }
            }

            // Recurse into closure/arrow-function args so provider route
            // registrations nested inside callbacks are found.
            for arg in *args {
                visit_provider_expr(arg.value, source, project, provider, provider_file, files);
            }
        }
        Expr::Closure { body, .. } => {
            // walk_stmts handles stmt-level recursion; closures are expr-level
            // so we have to descend manually here.
            walk_stmts(body, true, &mut |inner| {
                visit_provider_expr(inner, source, project, provider, provider_file, files);
            });
        }
        Expr::ArrowFunction { expr: inner, .. } => {
            visit_provider_expr(*inner, source, project, provider, provider_file, files);
        }
        _ => {}
    }
}

fn visit_bootstrap_expr(
    expr: ExprId<'_>,
    source: &[u8],
    project: &LaravelProject,
    bootstrap_file: &Path,
    files: &mut Vec<RegisteredRouteFile>,
) {
    match expr {
        Expr::MethodCall {
            target,
            method,
            args,
            ..
        } => {
            let method_name = expr_name(method, source).unwrap_or_default();
            if method_name == "withRouting" {
                files.extend(extract_bootstrap_routing_args(
                    args,
                    source,
                    project,
                    bootstrap_file,
                ));
            }

            visit_bootstrap_expr(*target, source, project, bootstrap_file, files);
            for arg in *args {
                visit_bootstrap_expr(arg.value, source, project, bootstrap_file, files);
            }
        }
        Expr::Closure { body, .. } => {
            walk_stmts(body, false, &mut |inner| {
                visit_bootstrap_expr(inner, source, project, bootstrap_file, files);
            });
        }
        Expr::ArrowFunction { expr: inner, .. } => {
            visit_bootstrap_expr(*inner, source, project, bootstrap_file, files);
        }
        _ => {}
    }
}

fn extract_bootstrap_routing_args(
    args: &[php_parser::ast::Arg<'_>],
    source: &[u8],
    project: &LaravelProject,
    bootstrap_file: &Path,
) -> Vec<RegisteredRouteFile> {
    let mut files = Vec::new();

    for arg in args {
        let Some(name) = arg.name else { continue };
        let name = crate::php::ast::span_text(name.span, source);
        if !matches!(name.as_str(), "web" | "api") {
            continue;
        }

        let line_info = arg.span.line_info(source);
        for route_file in expr_to_path_list(arg.value, source, &project.root, bootstrap_file) {
            files.push(RegisteredRouteFile {
                file: route_file,
                registration: RouteRegistration {
                    kind: "bootstrap-withRouting".to_string(),
                    declared_in: strip_root(&project.root, bootstrap_file),
                    line: line_info.map_or(1, |i| i.line),
                    column: line_info.map_or(1, |i| i.column),
                    provider_class: None,
                },
            });
        }
    }

    files
}

fn expr_to_path_list(
    expr: ExprId<'_>,
    source: &[u8],
    project_root: &Path,
    current_file: &Path,
) -> Vec<PathBuf> {
    match expr {
        Expr::Array { items, .. } => items
            .iter()
            .filter_map(|item| expr_to_path(item.value, source, project_root, current_file))
            .collect(),
        _ => expr_to_path(expr, source, project_root, current_file)
            .into_iter()
            .collect(),
    }
}

fn extract_grouped_route_file(
    expr: ExprId<'_>,
    source: &[u8],
    project: &LaravelProject,
    provider_file: &Path,
) -> Option<PathBuf> {
    let chain = flatten_route_chain(expr)?;
    let last = chain.last()?;
    let ChainOp::MethodCall { args, .. } = last else {
        return None;
    };
    let path_expr = args.first()?.value;
    let path = expr_to_path(path_expr, source, &project.root, provider_file)?;
    if !path.is_file() {
        return None;
    }

    let has_route_root = chain.iter().any(|op| match op {
        ChainOp::StaticCall { class, .. } => expr_name(*class, source).as_deref() == Some("Route"),
        ChainOp::MethodCall { .. } => false,
    });

    has_route_root.then_some(path)
}

fn default_route_registration(project_root: &Path, file: &Path) -> RouteRegistration {
    RouteRegistration {
        kind: "route-file".to_string(),
        declared_in: strip_root(project_root, file),
        line: 1,
        column: 1,
        provider_class: None,
    }
}

fn collect_php_files(dir: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                files.extend(collect_php_files(&path));
            } else if path.extension().and_then(|e| e.to_str()) == Some("php") {
                files.push(path);
            }
        }
    }
    files.sort();
    files
}
