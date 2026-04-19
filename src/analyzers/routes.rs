use bumpalo::Bump;
use php_parser::ast::{Arg, BinaryOp, ClassMember, Expr, ExprId, MagicConstKind, Stmt, StmtId};
use php_parser::lexer::Lexer;
use php_parser::parser::Parser;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Component, Path, PathBuf};

use crate::analyzers::providers;
use crate::project::LaravelProject;
use crate::types::{RouteEntry, RouteRegistration, RouteReport};

#[derive(Clone, Default)]
struct RouteContext {
    uri_prefix: String,
    name_prefix: String,
    middleware: Vec<String>,
    controller: Option<String>,
}

struct RouteSignature {
    methods: Vec<String>,
    uri_arg_index: usize,
    action_arg_index: usize,
}

struct RouteChunk {
    text: Vec<u8>,
    line: usize,
    complete: bool,
}

#[derive(Clone)]
struct RegisteredRouteFile {
    file: PathBuf,
    registration: RouteRegistration,
}

#[derive(Clone, Copy)]
enum ChainOp<'ast> {
    StaticCall {
        class: ExprId<'ast>,
        method: ExprId<'ast>,
        args: &'ast [Arg<'ast>],
    },
    MethodCall {
        method: ExprId<'ast>,
        args: &'ast [Arg<'ast>],
    },
}

#[derive(Default)]
struct ScanState {
    paren_depth: usize,
    bracket_depth: usize,
    brace_depth: usize,
    in_single_quote: bool,
    in_double_quote: bool,
    in_block_comment: bool,
    in_line_comment: bool,
    escape: bool,
    saw_semicolon: bool,
    statement_complete: bool,
}

pub fn analyze(project: &LaravelProject) -> Result<RouteReport, String> {
    let mut routes = Vec::new();
    let route_files = collect_registered_route_files(project)?;

    for registered in &route_files {
        let file = &registered.file;
        let source = fs::read(file)
            .map_err(|error| format!("failed to read {}: {error}", file.display()))?;
        collect_routes_from_source(
            &source,
            &project.root,
            file,
            &registered.registration,
            1,
            &RouteContext::default(),
            &mut routes,
        );
    }

    routes.sort_by(|left, right| {
        left.file
            .cmp(&right.file)
            .then(left.line.cmp(&right.line))
            .then(left.uri.cmp(&right.uri))
    });

    Ok(RouteReport {
        project_name: project.name.clone(),
        project_root: project.root.clone(),
        route_count: routes.len(),
        routes,
    })
}

fn collect_php_files(dir: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                files.extend(collect_php_files(&path));
            } else if path.extension().and_then(|ext| ext.to_str()) == Some("php") {
                files.push(path);
            }
        }
    }
    files.sort();
    files
}

fn collect_registered_route_files(
    project: &LaravelProject,
) -> Result<Vec<RegisteredRouteFile>, String> {
    let routes_dir = project.root.join("routes");
    let direct_files = collect_php_files(&routes_dir);
    let discovered = discover_provider_route_files(project)?;
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
                registration: default_route_registration(&project.root, file),
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

    all_files.sort_by(|left, right| {
        left.file
            .cmp(&right.file)
            .then(
                left.registration
                    .declared_in
                    .cmp(&right.registration.declared_in),
            )
            .then(left.registration.line.cmp(&right.registration.line))
    });

    Ok(all_files)
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

fn discover_provider_route_files(
    project: &LaravelProject,
) -> Result<Vec<RegisteredRouteFile>, String> {
    let provider_report = providers::analyze(project)?;
    let mut files = Vec::new();
    let mut seen_sources = BTreeSet::new();

    for provider in provider_report.providers {
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

        let source_file = project.root.join(relative_source_file);
        let source = fs::read(&source_file)
            .map_err(|error| format!("failed to read {}: {error}", source_file.display()))?;

        files.extend(extract_provider_route_files(
            project,
            &provider,
            &source_file,
            &source,
        ));
    }

    Ok(files)
}

fn extract_provider_route_files(
    project: &LaravelProject,
    provider: &crate::types::ProviderEntry,
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
    for statement in program.statements {
        collect_provider_route_files_from_stmt(
            statement,
            source,
            project,
            provider,
            provider_file,
            &mut files,
        );
    }
    files
}

fn collect_routes_from_source(
    source: &[u8],
    project_root: &Path,
    file: &Path,
    registration: &RouteRegistration,
    start_line: usize,
    context: &RouteContext,
    routes: &mut Vec<RouteEntry>,
) {
    if start_line == 1
        && source_can_use_full_parse(source)
        && collect_routes_with_full_parse(source, project_root, file, registration, context, routes)
    {
        return;
    }

    for chunk in split_route_chunks(source, start_line) {
        parse_route_chunk(&chunk, project_root, file, registration, context, routes);
    }
}

fn collect_routes_with_full_parse(
    source: &[u8],
    project_root: &Path,
    file: &Path,
    registration: &RouteRegistration,
    context: &RouteContext,
    routes: &mut Vec<RouteEntry>,
) -> bool {
    let arena = Bump::new();
    let lexer = Lexer::new(source);
    let mut parser = Parser::new(lexer, &arena);
    let program = parser.parse_program();
    if !program.errors.is_empty() {
        return false;
    }

    for statement in program.statements {
        collect_routes_from_stmt(
            statement,
            source,
            project_root,
            file,
            registration,
            context,
            routes,
            1,
            None,
        );
    }
    true
}

fn parse_route_chunk(
    chunk: &RouteChunk,
    project_root: &Path,
    file: &Path,
    registration: &RouteRegistration,
    context: &RouteContext,
    routes: &mut Vec<RouteEntry>,
) {
    if !chunk.complete && !chunk.text.windows(5).any(|window| window == b"group") {
        return;
    }

    let sanitized = sanitize_closure_bodies(&chunk.text);
    let mut snippet = b"<?php ".to_vec();
    snippet.extend_from_slice(&sanitized);

    let arena = Bump::new();
    let lexer = Lexer::new(&snippet);
    let mut parser = Parser::new(lexer, &arena);
    let program = parser.parse_program();
    if !program.errors.is_empty() {
        return;
    }

    for statement in program.statements {
        collect_routes_from_stmt(
            statement,
            &snippet,
            project_root,
            file,
            registration,
            context,
            routes,
            chunk.line,
            Some(&chunk.text),
        );
    }
}

fn collect_routes_from_stmt(
    statement: StmtId<'_>,
    source: &[u8],
    project_root: &Path,
    file: &Path,
    registration: &RouteRegistration,
    context: &RouteContext,
    routes: &mut Vec<RouteEntry>,
    line_offset: usize,
    raw_chunk: Option<&[u8]>,
) {
    match statement {
        Stmt::Expression { expr, .. } => {
            analyze_route_expression(
                expr,
                source,
                project_root,
                file,
                registration,
                context,
                routes,
                line_offset,
                raw_chunk,
            );
        }
        Stmt::Block { statements, .. }
        | Stmt::Declare {
            body: statements, ..
        } => {
            for statement in *statements {
                collect_routes_from_stmt(
                    statement,
                    source,
                    project_root,
                    file,
                    registration,
                    context,
                    routes,
                    line_offset,
                    raw_chunk,
                );
            }
        }
        Stmt::Namespace {
            body: Some(body), ..
        } => {
            for statement in *body {
                collect_routes_from_stmt(
                    statement,
                    source,
                    project_root,
                    file,
                    registration,
                    context,
                    routes,
                    line_offset,
                    raw_chunk,
                );
            }
        }
        Stmt::If {
            then_block,
            else_block,
            ..
        } => {
            for statement in *then_block {
                collect_routes_from_stmt(
                    statement,
                    source,
                    project_root,
                    file,
                    registration,
                    context,
                    routes,
                    line_offset,
                    raw_chunk,
                );
            }
            if let Some(else_block) = else_block {
                for statement in *else_block {
                    collect_routes_from_stmt(
                        statement,
                        source,
                        project_root,
                        file,
                        registration,
                        context,
                        routes,
                        line_offset,
                        raw_chunk,
                    );
                }
            }
        }
        Stmt::While { body, .. }
        | Stmt::DoWhile { body, .. }
        | Stmt::For { body, .. }
        | Stmt::Foreach { body, .. }
        | Stmt::Try { body, .. } => {
            for statement in *body {
                collect_routes_from_stmt(
                    statement,
                    source,
                    project_root,
                    file,
                    registration,
                    context,
                    routes,
                    line_offset,
                    raw_chunk,
                );
            }
        }
        _ => {}
    }
}

fn analyze_route_expression(
    expr: ExprId<'_>,
    source: &[u8],
    project_root: &Path,
    file: &Path,
    registration: &RouteRegistration,
    base_context: &RouteContext,
    routes: &mut Vec<RouteEntry>,
    line_offset: usize,
    raw_chunk: Option<&[u8]>,
) {
    let Some(ops) = flatten_route_chain(expr) else {
        return;
    };

    let mut context = base_context.clone();
    let mut current_route = None;

    for op in ops {
        match op {
            ChainOp::StaticCall {
                class,
                method,
                args,
            } => {
                if expr_name(class, source).as_deref() != Some("Route") {
                    return;
                }
                let Some(method_name) = expr_name(method, source) else {
                    return;
                };

                if let Some(signature) = route_signature(&method_name, args, source) {
                    current_route = Some(build_route_entry(
                        &context,
                        project_root,
                        file,
                        registration,
                        route_line(expr, source, line_offset),
                        signature,
                        args,
                        source,
                    ));
                    continue;
                }

                apply_modifier(
                    &mut context,
                    current_route.as_mut(),
                    &method_name,
                    args,
                    source,
                );

                if method_name == "group" {
                    if let Some(raw_chunk) = raw_chunk {
                        if let Some((body, body_line)) = extract_group_body(raw_chunk, line_offset)
                        {
                            collect_routes_from_source(
                                &body,
                                project_root,
                                file,
                                registration,
                                body_line,
                                &context,
                                routes,
                            );
                        }
                    } else if let Some(body) = group_body(args) {
                        for statement in body {
                            collect_routes_from_stmt(
                                statement,
                                source,
                                project_root,
                                file,
                                registration,
                                &context,
                                routes,
                                line_offset,
                                None,
                            );
                        }
                    }
                    return;
                }
            }
            ChainOp::MethodCall { method, args } => {
                let Some(method_name) = expr_name(method, source) else {
                    return;
                };

                if current_route.is_none() {
                    if let Some(signature) = route_signature(&method_name, args, source) {
                        current_route = Some(build_route_entry(
                            &context,
                            project_root,
                            file,
                            registration,
                            route_line(expr, source, line_offset),
                            signature,
                            args,
                            source,
                        ));
                        continue;
                    }
                }

                apply_modifier(
                    &mut context,
                    current_route.as_mut(),
                    &method_name,
                    args,
                    source,
                );

                if method_name == "group" {
                    if let Some(raw_chunk) = raw_chunk {
                        if let Some((body, body_line)) = extract_group_body(raw_chunk, line_offset)
                        {
                            collect_routes_from_source(
                                &body,
                                project_root,
                                file,
                                registration,
                                body_line,
                                &context,
                                routes,
                            );
                        }
                    } else if let Some(body) = group_body(args) {
                        for statement in body {
                            collect_routes_from_stmt(
                                statement,
                                source,
                                project_root,
                                file,
                                registration,
                                &context,
                                routes,
                                line_offset,
                                None,
                            );
                        }
                    }
                    return;
                }
            }
        }
    }

    if let Some(route) = current_route {
        routes.push(route);
    }
}

fn collect_provider_route_files_from_stmt(
    statement: StmtId<'_>,
    source: &[u8],
    project: &LaravelProject,
    provider: &crate::types::ProviderEntry,
    provider_file: &Path,
    files: &mut Vec<RegisteredRouteFile>,
) {
    match statement {
        Stmt::Expression { expr, .. } => {
            collect_provider_route_files_from_expr(
                expr,
                source,
                project,
                provider,
                provider_file,
                files,
            );
        }
        Stmt::Block { statements, .. }
        | Stmt::Declare {
            body: statements, ..
        } => {
            for statement in *statements {
                collect_provider_route_files_from_stmt(
                    statement,
                    source,
                    project,
                    provider,
                    provider_file,
                    files,
                );
            }
        }
        Stmt::Namespace {
            body: Some(body), ..
        } => {
            for statement in *body {
                collect_provider_route_files_from_stmt(
                    statement,
                    source,
                    project,
                    provider,
                    provider_file,
                    files,
                );
            }
        }
        Stmt::Class { members, .. }
        | Stmt::Interface { members, .. }
        | Stmt::Trait { members, .. }
        | Stmt::Enum { members, .. } => {
            for member in *members {
                if let ClassMember::Method { body, .. } = member {
                    for statement in *body {
                        collect_provider_route_files_from_stmt(
                            statement,
                            source,
                            project,
                            provider,
                            provider_file,
                            files,
                        );
                    }
                }
            }
        }
        Stmt::If {
            then_block,
            else_block,
            ..
        } => {
            for statement in *then_block {
                collect_provider_route_files_from_stmt(
                    statement,
                    source,
                    project,
                    provider,
                    provider_file,
                    files,
                );
            }
            if let Some(else_block) = else_block {
                for statement in *else_block {
                    collect_provider_route_files_from_stmt(
                        statement,
                        source,
                        project,
                        provider,
                        provider_file,
                        files,
                    );
                }
            }
        }
        Stmt::While { body, .. }
        | Stmt::DoWhile { body, .. }
        | Stmt::For { body, .. }
        | Stmt::Foreach { body, .. }
        | Stmt::Try { body, .. } => {
            for statement in *body {
                collect_provider_route_files_from_stmt(
                    statement,
                    source,
                    project,
                    provider,
                    provider_file,
                    files,
                );
            }
        }
        _ => {}
    }
}

fn collect_provider_route_files_from_expr(
    expr: ExprId<'_>,
    source: &[u8],
    project: &LaravelProject,
    provider: &crate::types::ProviderEntry,
    provider_file: &Path,
    files: &mut Vec<RegisteredRouteFile>,
) {
    let Expr::MethodCall { method, args, .. } = expr else {
        return;
    };
    let Some(method_name) = expr_name(method, source) else {
        return;
    };
    if method_name != "loadRoutesFrom" {
        return;
    }

    let Some(path_expr) = args.first().map(|arg| arg.value) else {
        return;
    };
    let Some(route_file) = expr_to_path(path_expr, source, project, provider_file) else {
        return;
    };

    let line_info = path_expr.span().line_info(source);
    let line = line_info.map_or(provider.line, |info| info.line);
    let column = line_info.map_or(provider.column, |info| info.column);

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

fn flatten_route_chain<'ast>(expr: ExprId<'ast>) -> Option<Vec<ChainOp<'ast>>> {
    let mut ops = Vec::new();

    fn visit<'ast>(expr: ExprId<'ast>, ops: &mut Vec<ChainOp<'ast>>) -> bool {
        match expr {
            Expr::MethodCall {
                target,
                method,
                args,
                ..
            } => {
                if !visit(target, ops) {
                    return false;
                }
                ops.push(ChainOp::MethodCall { method, args });
                true
            }
            Expr::StaticCall {
                class,
                method,
                args,
                ..
            } => {
                ops.push(ChainOp::StaticCall {
                    class,
                    method,
                    args,
                });
                true
            }
            _ => false,
        }
    }

    if visit(expr, &mut ops) {
        Some(ops)
    } else {
        None
    }
}

fn apply_modifier(
    context: &mut RouteContext,
    route: Option<&mut RouteEntry>,
    method_name: &str,
    args: &[Arg<'_>],
    source: &[u8],
) {
    match method_name {
        "prefix" => {
            if let Some(value) = args
                .first()
                .and_then(|arg| expr_to_string(arg.value, source))
            {
                if let Some(route) = route {
                    route.uri = join_uri(&value, &route.uri);
                } else {
                    context.uri_prefix = join_uri(&context.uri_prefix, &value);
                }
            }
        }
        "name" | "as" => {
            if let Some(value) = args
                .first()
                .and_then(|arg| expr_to_string(arg.value, source))
            {
                if let Some(route) = route {
                    let current = route.name.take().unwrap_or_default();
                    route.name = Some(format!("{current}{value}"));
                } else {
                    context.name_prefix.push_str(&value);
                }
            }
        }
        "middleware" => {
            let values = args_to_string_list(args, source);
            if let Some(route) = route {
                route.middleware.extend(values);
            } else {
                context.middleware.extend(values);
            }
        }
        "controller" => {
            if let Some(value) = args
                .first()
                .and_then(|arg| expr_to_controller(arg.value, source))
            {
                context.controller = Some(value);
            }
        }
        _ => {}
    }
}

fn build_route_entry(
    context: &RouteContext,
    project_root: &Path,
    file: &Path,
    registration: &RouteRegistration,
    line: usize,
    signature: RouteSignature,
    args: &[Arg<'_>],
    source: &[u8],
) -> RouteEntry {
    let (line, column) = route_position(source, args, line);
    let raw_uri = args
        .get(signature.uri_arg_index)
        .and_then(|arg| expr_to_string(arg.value, source))
        .unwrap_or_else(|| "/".to_string());
    let action = args
        .get(signature.action_arg_index)
        .and_then(|arg| expr_to_action(arg.value, context.controller.as_deref(), source));

    RouteEntry {
        file: strip_root(project_root, file),
        line,
        column,
        methods: signature.methods,
        uri: join_uri(&context.uri_prefix, &raw_uri),
        name: (!context.name_prefix.is_empty()).then(|| context.name_prefix.clone()),
        action,
        middleware: context.middleware.clone(),
        registration: registration.clone(),
    }
}

fn route_signature(method_name: &str, args: &[Arg<'_>], source: &[u8]) -> Option<RouteSignature> {
    let static_method = |name: &str| {
        Some(RouteSignature {
            methods: vec![name.to_string()],
            uri_arg_index: 0,
            action_arg_index: 1,
        })
    };

    match method_name {
        "get" => static_method("GET"),
        "post" => static_method("POST"),
        "put" => static_method("PUT"),
        "patch" => static_method("PATCH"),
        "delete" => static_method("DELETE"),
        "options" => static_method("OPTIONS"),
        "any" => static_method("ANY"),
        "match" => args
            .first()
            .map(|arg| {
                expr_to_string_list(arg.value, source)
                    .into_iter()
                    .map(|method| method.to_ascii_uppercase())
                    .collect::<Vec<_>>()
            })
            .filter(|methods| !methods.is_empty())
            .map(|methods| RouteSignature {
                methods,
                uri_arg_index: 1,
                action_arg_index: 2,
            }),
        _ => None,
    }
}

fn expr_name(expr: ExprId<'_>, source: &[u8]) -> Option<String> {
    match expr {
        Expr::Variable { name, .. } => {
            Some(span_text(*name, source).trim_start_matches('$').to_string())
        }
        Expr::String { value, .. } => Some(String::from_utf8_lossy(value).into_owned()),
        _ => None,
    }
}

fn expr_to_string(expr: ExprId<'_>, source: &[u8]) -> Option<String> {
    match expr {
        Expr::String { value, .. } => Some(parse_php_string_literal(value)),
        Expr::Variable { name, .. } => {
            Some(span_text(*name, source).trim_start_matches('$').to_string())
        }
        Expr::ClassConstFetch {
            class, constant, ..
        } => {
            let class_name = expr_name(class, source)?;
            let constant_name = expr_name(constant, source)?;
            Some(format!("{class_name}::{constant_name}"))
        }
        _ => None,
    }
}

fn expr_to_path(
    expr: ExprId<'_>,
    source: &[u8],
    project: &LaravelProject,
    provider_file: &Path,
) -> Option<PathBuf> {
    let raw = expr_to_path_fragment(expr, source, project, provider_file)?;
    let path = PathBuf::from(raw);
    let absolute = if path.is_absolute() {
        path
    } else {
        project.root.join(path)
    };
    Some(normalize_path(&absolute))
}

fn expr_to_path_fragment(
    expr: ExprId<'_>,
    source: &[u8],
    project: &LaravelProject,
    provider_file: &Path,
) -> Option<String> {
    match expr {
        Expr::String { .. } => expr_to_string(expr, source),
        Expr::MagicConst { kind, .. } => {
            if *kind == MagicConstKind::Dir {
                provider_file
                    .parent()
                    .map(|path| path.display().to_string())
            } else {
                None
            }
        }
        Expr::Binary {
            left, op, right, ..
        } if *op == BinaryOp::Concat => {
            let left = expr_to_path_fragment(left, source, project, provider_file)?;
            let right = expr_to_path_fragment(right, source, project, provider_file)?;
            Some(format!("{left}{right}"))
        }
        Expr::Call { func, args, .. } => {
            let function_name = span_text(func.span(), source)
                .trim()
                .trim_start_matches('\\')
                .to_string();
            let arg = args.first().map(|arg| arg.value)?;
            let inner = expr_to_path_fragment(arg, source, project, provider_file)?;
            match function_name.as_str() {
                "base_path" => Some(project.root.join(inner).display().to_string()),
                "app_path" => Some(project.root.join("app").join(inner).display().to_string()),
                "resource_path" => Some(
                    project
                        .root
                        .join("resources")
                        .join(inner)
                        .display()
                        .to_string(),
                ),
                _ => None,
            }
        }
        _ => None,
    }
}

fn expr_to_string_list(expr: ExprId<'_>, source: &[u8]) -> Vec<String> {
    match expr {
        Expr::Array { items, .. } => items
            .iter()
            .filter_map(|item| expr_to_string(item.value, source))
            .collect(),
        _ => expr_to_string(expr, source).into_iter().collect(),
    }
}

fn args_to_string_list(args: &[Arg<'_>], source: &[u8]) -> Vec<String> {
    let mut values = Vec::new();
    for arg in args {
        values.extend(expr_to_string_list(arg.value, source));
    }
    values
}

fn expr_to_controller(expr: ExprId<'_>, source: &[u8]) -> Option<String> {
    match expr {
        Expr::ClassConstFetch {
            class, constant, ..
        } => {
            let class_name = expr_name(class, source)?;
            let constant_name = expr_name(constant, source)?;
            if constant_name == "class" {
                Some(class_name)
            } else {
                Some(format!("{class_name}::{constant_name}"))
            }
        }
        _ => expr_to_string(expr, source),
    }
}

fn expr_to_action(expr: ExprId<'_>, controller: Option<&str>, source: &[u8]) -> Option<String> {
    match expr {
        Expr::Closure { .. } | Expr::ArrowFunction { .. } => Some("closure".to_string()),
        Expr::ClassConstFetch { .. } => expr_to_controller(expr, source),
        Expr::Array { items, .. } if items.len() >= 2 => {
            let controller_expr = items.first()?.value;
            let method_expr = items.get(1)?.value;
            let controller_name = expr_to_controller(controller_expr, source)?;
            let method_name = expr_to_string(method_expr, source)?;
            Some(format!("{controller_name}@{method_name}"))
        }
        _ => {
            let value = expr_to_string(expr, source)?;
            if let Some(controller) = controller {
                if !value.contains('@') && !value.contains("::") {
                    return Some(format!("{controller}@{value}"));
                }
            }
            Some(value)
        }
    }
}

fn join_uri(prefix: &str, path: &str) -> String {
    let prefix = prefix.trim_matches('/');
    let path = path.trim_matches('/');
    match (prefix.is_empty(), path.is_empty()) {
        (true, true) => "/".to_string(),
        (true, false) => format!("/{path}"),
        (false, true) => format!("/{prefix}"),
        (false, false) => format!("/{prefix}/{path}"),
    }
}

fn span_text(span: php_parser::Span, source: &[u8]) -> String {
    String::from_utf8_lossy(span.as_str(source)).into_owned()
}

fn parse_php_string_literal(value: &[u8]) -> String {
    let text = String::from_utf8_lossy(value).into_owned();
    if text.len() >= 2 {
        let first = text.as_bytes()[0];
        let last = text.as_bytes()[text.len() - 1];
        if (first == b'\'' && last == b'\'') || (first == b'"' && last == b'"') {
            return text[1..text.len() - 1].to_string();
        }
    }
    text
}

fn normalize_path(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            other => normalized.push(other.as_os_str()),
        }
    }
    normalized
}

fn route_line(expr: ExprId<'_>, source: &[u8], line_offset: usize) -> usize {
    line_offset + chunk_relative_line(expr, source) - 1
}

fn route_position(source: &[u8], args: &[Arg<'_>], fallback_line: usize) -> (usize, usize) {
    if let Some(arg) = args.first()
        && let Some(info) = arg.value.span().line_info(source)
    {
        return (fallback_line, info.column);
    }

    (fallback_line, 1)
}

fn chunk_relative_line(expr: ExprId<'_>, source: &[u8]) -> usize {
    expr.span().line_info(source).map_or(1, |info| info.line)
}

fn split_route_chunks(source: &[u8], start_line: usize) -> Vec<RouteChunk> {
    let mut chunks = Vec::new();
    let mut line_starts = vec![0usize];
    for (index, byte) in source.iter().enumerate() {
        if *byte == b'\n' && index + 1 < source.len() {
            line_starts.push(index + 1);
        }
    }

    let mut current_start = None;
    let mut current_line = 0usize;
    let mut current_indent = 0usize;
    let mut state = ScanState::default();

    for (line_index, line_start) in line_starts.iter().copied().enumerate() {
        let line_end = source[line_start..]
            .iter()
            .position(|byte| *byte == b'\n')
            .map(|offset| line_start + offset + 1)
            .unwrap_or(source.len());
        let line = &source[line_start..line_end];
        let trimmed = trim_ascii_start(line);
        let indent = line.len().saturating_sub(trimmed.len());
        let starts_route = trimmed.starts_with(b"Route::");

        if let Some(chunk_start) = current_start {
            if starts_route && indent <= current_indent && !state.statement_complete {
                chunks.push(RouteChunk {
                    text: source[chunk_start..line_start].to_vec(),
                    line: current_line,
                    complete: false,
                });
                current_start = Some(line_start);
                current_line = start_line + line_index;
                current_indent = indent;
                state = ScanState::default();
                state.consume(line);
            } else {
                state.consume(line);
                if state.statement_complete {
                    chunks.push(RouteChunk {
                        text: source[chunk_start..line_end].to_vec(),
                        line: current_line,
                        complete: true,
                    });
                    current_start = None;
                    state = ScanState::default();
                }
            }
        } else if starts_route {
            current_start = Some(line_start);
            current_line = start_line + line_index;
            current_indent = indent;
            state = ScanState::default();
            state.consume(line);
            if state.statement_complete {
                chunks.push(RouteChunk {
                    text: source[line_start..line_end].to_vec(),
                    line: current_line,
                    complete: true,
                });
                current_start = None;
                state = ScanState::default();
            }
        }
    }

    if let Some(chunk_start) = current_start {
        chunks.push(RouteChunk {
            text: source[chunk_start..].to_vec(),
            line: current_line,
            complete: false,
        });
    }
    chunks
}

impl ScanState {
    fn consume(&mut self, bytes: &[u8]) {
        let mut index = 0;
        self.in_line_comment = false;

        while index < bytes.len() {
            let byte = bytes[index];
            let next = bytes.get(index + 1).copied();

            if self.in_line_comment {
                index += 1;
                continue;
            }
            if self.in_block_comment {
                if byte == b'*' && next == Some(b'/') {
                    self.in_block_comment = false;
                    index += 2;
                } else {
                    index += 1;
                }
                continue;
            }
            if self.in_single_quote {
                if self.escape {
                    self.escape = false;
                } else if byte == b'\\' {
                    self.escape = true;
                } else if byte == b'\'' {
                    self.in_single_quote = false;
                }
                index += 1;
                continue;
            }
            if self.in_double_quote {
                if self.escape {
                    self.escape = false;
                } else if byte == b'\\' {
                    self.escape = true;
                } else if byte == b'"' {
                    self.in_double_quote = false;
                }
                index += 1;
                continue;
            }

            if byte == b'/' && next == Some(b'/') {
                self.in_line_comment = true;
                index += 2;
                continue;
            }
            if byte == b'#' {
                self.in_line_comment = true;
                index += 1;
                continue;
            }
            if byte == b'/' && next == Some(b'*') {
                self.in_block_comment = true;
                index += 2;
                continue;
            }

            match byte {
                b'\'' => self.in_single_quote = true,
                b'"' => self.in_double_quote = true,
                b'(' => self.paren_depth += 1,
                b')' => self.paren_depth = self.paren_depth.saturating_sub(1),
                b'[' => self.bracket_depth += 1,
                b']' => self.bracket_depth = self.bracket_depth.saturating_sub(1),
                b'{' => self.brace_depth += 1,
                b'}' => self.brace_depth = self.brace_depth.saturating_sub(1),
                b';' => self.saw_semicolon = true,
                _ => {}
            }
            index += 1;
        }

        self.statement_complete = self.saw_semicolon
            && self.paren_depth == 0
            && self.bracket_depth == 0
            && self.brace_depth == 0;
    }

    fn is_balanced(&self) -> bool {
        self.paren_depth == 0
            && self.bracket_depth == 0
            && self.brace_depth == 0
            && !self.in_single_quote
            && !self.in_double_quote
            && !self.in_block_comment
    }
}

fn trim_ascii_start(bytes: &[u8]) -> &[u8] {
    let mut index = 0;
    while let Some(byte) = bytes.get(index) {
        if !byte.is_ascii_whitespace() {
            break;
        }
        index += 1;
    }
    &bytes[index..]
}

fn sanitize_closure_bodies(source: &[u8]) -> Vec<u8> {
    let mut output = Vec::with_capacity(source.len());
    let mut index = 0;

    while index < source.len() {
        if starts_with_keyword(source, index, b"function") {
            let function_start = index;
            if let Some(open_brace) = find_closure_open_brace(source, index + "function".len()) {
                output.extend_from_slice(&source[function_start..=open_brace]);
                output.push(b'}');
                if let Some(close_brace) = find_matching_brace(source, open_brace) {
                    index = close_brace + 1;
                } else {
                    output.extend_from_slice(b");");
                    index = source.len();
                }
                continue;
            }
        }

        output.push(source[index]);
        index += 1;
    }
    output
}

fn starts_with_keyword(source: &[u8], index: usize, keyword: &[u8]) -> bool {
    source
        .get(index..index + keyword.len())
        .is_some_and(|slice| slice == keyword)
        && !source
            .get(index.wrapping_sub(1))
            .is_some_and(|byte| byte.is_ascii_alphanumeric() || *byte == b'_')
        && !source
            .get(index + keyword.len())
            .is_some_and(|byte| byte.is_ascii_alphanumeric() || *byte == b'_')
}

fn find_closure_open_brace(source: &[u8], mut index: usize) -> Option<usize> {
    let mut state = ScanState::default();
    while index < source.len() {
        let byte = source[index];
        let next = source.get(index + 1).copied();

        if state.in_line_comment {
            if byte == b'\n' {
                state.in_line_comment = false;
            }
            index += 1;
            continue;
        }
        if state.in_block_comment {
            if byte == b'*' && next == Some(b'/') {
                state.in_block_comment = false;
                index += 2;
            } else {
                index += 1;
            }
            continue;
        }
        if state.in_single_quote {
            if state.escape {
                state.escape = false;
            } else if byte == b'\\' {
                state.escape = true;
            } else if byte == b'\'' {
                state.in_single_quote = false;
            }
            index += 1;
            continue;
        }
        if state.in_double_quote {
            if state.escape {
                state.escape = false;
            } else if byte == b'\\' {
                state.escape = true;
            } else if byte == b'"' {
                state.in_double_quote = false;
            }
            index += 1;
            continue;
        }

        if byte == b'/' && next == Some(b'/') {
            state.in_line_comment = true;
            index += 2;
            continue;
        }
        if byte == b'#' {
            state.in_line_comment = true;
            index += 1;
            continue;
        }
        if byte == b'/' && next == Some(b'*') {
            state.in_block_comment = true;
            index += 2;
            continue;
        }
        if byte == b'\'' {
            state.in_single_quote = true;
            index += 1;
            continue;
        }
        if byte == b'"' {
            state.in_double_quote = true;
            index += 1;
            continue;
        }
        if byte == b'{' {
            return Some(index);
        }
        index += 1;
    }
    None
}

fn find_matching_brace(source: &[u8], open_brace: usize) -> Option<usize> {
    let mut depth = 0usize;
    let mut state = ScanState::default();
    let mut index = open_brace;

    while index < source.len() {
        let byte = source[index];
        let next = source.get(index + 1).copied();

        if state.in_line_comment {
            if byte == b'\n' {
                state.in_line_comment = false;
            }
            index += 1;
            continue;
        }
        if state.in_block_comment {
            if byte == b'*' && next == Some(b'/') {
                state.in_block_comment = false;
                index += 2;
            } else {
                index += 1;
            }
            continue;
        }
        if state.in_single_quote {
            if state.escape {
                state.escape = false;
            } else if byte == b'\\' {
                state.escape = true;
            } else if byte == b'\'' {
                state.in_single_quote = false;
            }
            index += 1;
            continue;
        }
        if state.in_double_quote {
            if state.escape {
                state.escape = false;
            } else if byte == b'\\' {
                state.escape = true;
            } else if byte == b'"' {
                state.in_double_quote = false;
            }
            index += 1;
            continue;
        }

        if byte == b'/' && next == Some(b'/') {
            state.in_line_comment = true;
            index += 2;
            continue;
        }
        if byte == b'#' {
            state.in_line_comment = true;
            index += 1;
            continue;
        }
        if byte == b'/' && next == Some(b'*') {
            state.in_block_comment = true;
            index += 2;
            continue;
        }
        if byte == b'\'' {
            state.in_single_quote = true;
            index += 1;
            continue;
        }
        if byte == b'"' {
            state.in_double_quote = true;
            index += 1;
            continue;
        }

        if byte == b'{' {
            depth += 1;
        } else if byte == b'}' {
            depth = depth.saturating_sub(1);
            if depth == 0 {
                return Some(index);
            }
        }
        index += 1;
    }
    None
}

fn extract_group_body(source: &[u8], base_line: usize) -> Option<(Vec<u8>, usize)> {
    let group_index = find_bytes(source, b"group")?;
    let function_index = find_bytes(&source[group_index..], b"function")? + group_index;
    let open_brace = find_closure_open_brace(source, function_index + "function".len())?;
    let body_start = open_brace + 1;
    let body_end = find_matching_brace(source, open_brace).unwrap_or(source.len());
    let body_line = base_line + count_newlines(&source[..body_start]);
    Some((source[body_start..body_end].to_vec(), body_line))
}

fn group_body<'ast>(args: &'ast [Arg<'ast>]) -> Option<&'ast [StmtId<'ast>]> {
    let expr = args.first()?.value;
    match expr {
        Expr::Closure { body, .. } => Some(body),
        _ => None,
    }
}

fn find_bytes(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

fn count_newlines(bytes: &[u8]) -> usize {
    bytes.iter().filter(|byte| **byte == b'\n').count()
}

fn source_can_use_full_parse(source: &[u8]) -> bool {
    let mut state = ScanState::default();
    state.consume(source);
    state.is_balanced()
}

fn strip_root(root: &Path, file: &Path) -> PathBuf {
    file.strip_prefix(root).unwrap_or(file).to_path_buf()
}
