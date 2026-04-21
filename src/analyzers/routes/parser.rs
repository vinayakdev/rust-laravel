use bumpalo::Bump;
use php_parser::ast::{Expr, ExprId, StmtId};
use php_parser::lexer::Lexer;
use php_parser::parser::Parser;
use std::cell::RefCell;
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use super::chain::{
    ChainOp, apply_modifier, build_route_entry, flatten_route_chain, join_uri, resource_routes,
    route_line, route_signature,
};
use super::context::{MiddlewareIndex, RouteContext};
use crate::php::ast::{expr_name, expr_to_path, expr_to_string, expr_to_string_list, strip_root};
use crate::php::walk::walk_stmts;
use crate::types::{RouteEntry, RouteRegistration};

thread_local! {
    static VISITED_INCLUDES: RefCell<BTreeSet<PathBuf>> = RefCell::new(BTreeSet::new());
}

pub(crate) fn reset_include_tracking() {
    VISITED_INCLUDES.with(|v| v.borrow_mut().clear());
}

pub(crate) struct RouteChunk {
    pub(crate) text: Vec<u8>,
    pub(crate) line: usize,
    pub(crate) complete: bool,
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

// Entry point: tries a full parse first; falls back to chunk-based parsing for
// malformed PHP (common in route files that mix closures and raw statements).
pub(crate) fn collect_routes_from_source(
    source: &[u8],
    project_root: &Path,
    file: &Path,
    registration: &RouteRegistration,
    start_line: usize,
    context: &RouteContext,
    middleware_index: &MiddlewareIndex,
    routes: &mut Vec<RouteEntry>,
) {
    if start_line == 1
        && source_can_use_full_parse(source)
        && try_full_parse(
            source,
            project_root,
            file,
            registration,
            context,
            middleware_index,
            routes,
        )
    {
        return;
    }

    for chunk in split_route_chunks(source, start_line) {
        parse_chunk(
            &chunk,
            project_root,
            file,
            registration,
            context,
            middleware_index,
            routes,
        );
    }
}

fn try_full_parse(
    source: &[u8],
    project_root: &Path,
    file: &Path,
    registration: &RouteRegistration,
    context: &RouteContext,
    middleware_index: &MiddlewareIndex,
    routes: &mut Vec<RouteEntry>,
) -> bool {
    let arena = Bump::new();
    let lexer = Lexer::new(source);
    let mut parser = Parser::new(lexer, &arena);
    let program = parser.parse_program();
    if !program.errors.is_empty() {
        return false;
    }

    walk_stmts(program.statements, false, &mut |expr| {
        analyze_expr(
            expr,
            source,
            project_root,
            file,
            registration,
            context,
            middleware_index,
            routes,
            1,
            None,
        );
    });
    true
}

fn parse_chunk(
    chunk: &RouteChunk,
    project_root: &Path,
    file: &Path,
    registration: &RouteRegistration,
    context: &RouteContext,
    middleware_index: &MiddlewareIndex,
    routes: &mut Vec<RouteEntry>,
) {
    if !chunk.complete && !chunk.text.windows(5).any(|w| w == b"group") {
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

    let raw = chunk.text.as_slice();
    walk_stmts(program.statements, false, &mut |expr| {
        analyze_expr(
            expr,
            &snippet,
            project_root,
            file,
            registration,
            context,
            middleware_index,
            routes,
            chunk.line,
            Some(raw),
        );
    });
}

// Analyses a single route chain expression, handling group nesting.
// Takes `raw_chunk` so group bodies can be re-parsed with correct line numbers.
pub(crate) fn analyze_expr(
    expr: ExprId<'_>,
    source: &[u8],
    project_root: &Path,
    file: &Path,
    registration: &RouteRegistration,
    base_context: &RouteContext,
    middleware_index: &MiddlewareIndex,
    routes: &mut Vec<RouteEntry>,
    line_offset: usize,
    raw_chunk: Option<&[u8]>,
) {
    if let Expr::Include {
        expr: path_expr, ..
    } = expr
    {
        if let Some(included) = expr_to_path(path_expr, source, project_root, file) {
            let is_new = VISITED_INCLUDES.with(|v| v.borrow_mut().insert(included.clone()));
            if is_new {
                if let Ok(included_source) = fs::read(&included) {
                    collect_routes_from_source(
                        &included_source,
                        project_root,
                        &included,
                        registration,
                        1,
                        base_context,
                        middleware_index,
                        routes,
                    );
                }
            }
        }
        return;
    }

    let Some(ops) = flatten_route_chain(expr) else {
        return;
    };

    let mut context = base_context.clone();
    let mut current_routes: Vec<RouteEntry> = Vec::new();

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
                if current_routes.is_empty()
                    && let Some(routes) = build_special_routes(
                        &method_name,
                        args,
                        &context,
                        source,
                        project_root,
                        file,
                        registration,
                        route_line(expr, source, line_offset),
                        middleware_index,
                    )
                {
                    current_routes = routes;
                    continue;
                }
                if let Some(sig) = route_signature(&method_name, args, source) {
                    current_routes.push(build_route_entry(
                        &context,
                        project_root,
                        file,
                        registration,
                        route_line(expr, source, line_offset),
                        sig,
                        args,
                        source,
                        middleware_index,
                    ));
                    continue;
                }
                apply_modifier_to_routes(
                    &mut context,
                    &mut current_routes,
                    &method_name,
                    args,
                    source,
                );
                if handle_group(
                    &method_name,
                    raw_chunk,
                    args,
                    source,
                    project_root,
                    file,
                    registration,
                    line_offset,
                    &context,
                    middleware_index,
                    routes,
                ) {
                    return;
                }
            }
            ChainOp::MethodCall { method, args } => {
                let Some(method_name) = expr_name(method, source) else {
                    return;
                };
                if current_routes.is_empty() {
                    if let Some(routes) = build_special_routes(
                        &method_name,
                        args,
                        &context,
                        source,
                        project_root,
                        file,
                        registration,
                        route_line(expr, source, line_offset),
                        middleware_index,
                    ) {
                        current_routes = routes;
                        continue;
                    }
                    if let Some(sig) = route_signature(&method_name, args, source) {
                        current_routes.push(build_route_entry(
                            &context,
                            project_root,
                            file,
                            registration,
                            route_line(expr, source, line_offset),
                            sig,
                            args,
                            source,
                            middleware_index,
                        ));
                        continue;
                    }
                }
                apply_modifier_to_routes(
                    &mut context,
                    &mut current_routes,
                    &method_name,
                    args,
                    source,
                );
                if handle_group(
                    &method_name,
                    raw_chunk,
                    args,
                    source,
                    project_root,
                    file,
                    registration,
                    line_offset,
                    &context,
                    middleware_index,
                    routes,
                ) {
                    return;
                }
            }
        }
    }

    if !current_routes.is_empty() {
        routes.extend(current_routes);
    }
}

fn apply_modifier_to_routes(
    context: &mut RouteContext,
    routes: &mut Vec<RouteEntry>,
    method_name: &str,
    args: &[php_parser::ast::Arg<'_>],
    source: &[u8],
) {
    match method_name {
        "only" if !routes.is_empty() => {
            let Some(arg) = args.first() else { return };
            let allowed = expr_to_string_list(arg.value, source);
            routes.retain(|route| {
                route
                    .name
                    .as_deref()
                    .and_then(|name| name.rsplit('.').next())
                    .map(|leaf| allowed.iter().any(|item| item == leaf))
                    .unwrap_or(true)
            });
        }
        "except" if !routes.is_empty() => {
            let Some(arg) = args.first() else { return };
            let blocked = expr_to_string_list(arg.value, source);
            routes.retain(|route| {
                route
                    .name
                    .as_deref()
                    .and_then(|name| name.rsplit('.').next())
                    .map(|leaf| blocked.iter().all(|item| item != leaf))
                    .unwrap_or(true)
            });
        }
        _ if routes.is_empty() => apply_modifier(context, None, method_name, args, source),
        _ => {
            for route in routes.iter_mut() {
                apply_modifier(context, Some(route), method_name, args, source);
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn build_special_routes(
    method_name: &str,
    args: &[php_parser::ast::Arg<'_>],
    context: &RouteContext,
    source: &[u8],
    project_root: &Path,
    file: &Path,
    registration: &RouteRegistration,
    line: usize,
    middleware_index: &MiddlewareIndex,
) -> Option<Vec<RouteEntry>> {
    match method_name {
        "resource" | "apiResource" | "singleton" => {
            let resource = args.first().and_then(|a| expr_to_string(a.value, source))?;
            let controller = args.get(1).and_then(|a| controller_name(a.value, source))?;
            Some(expand_resource_routes(
                &resource,
                &controller,
                method_name == "apiResource",
                method_name == "singleton",
                context,
                project_root,
                file,
                registration,
                line,
                middleware_index,
            ))
        }
        "resources" | "apiResources" => {
            let php_parser::ast::Expr::Array { items, .. } = args.first()?.value else {
                return None;
            };
            let mut routes = Vec::new();
            for item in items.iter() {
                let resource = item.key.and_then(|key| expr_to_string(key, source))?;
                let controller = controller_name(item.value, source)?;
                routes.extend(expand_resource_routes(
                    &resource,
                    &controller,
                    method_name == "apiResources",
                    false,
                    context,
                    project_root,
                    file,
                    registration,
                    line,
                    middleware_index,
                ));
            }
            Some(routes)
        }
        _ => None,
    }
}

fn controller_name(expr: ExprId<'_>, source: &[u8]) -> Option<String> {
    match expr {
        Expr::ClassConstFetch {
            class, constant, ..
        } => {
            if expr_name(constant, source).as_deref() == Some("class") {
                expr_name(class, source)
            } else {
                expr_to_string(expr, source)
            }
        }
        _ => expr_to_string(expr, source),
    }
}

#[allow(clippy::too_many_arguments)]
fn expand_resource_routes(
    resource: &str,
    controller: &str,
    api: bool,
    singleton: bool,
    context: &RouteContext,
    project_root: &Path,
    file: &Path,
    registration: &RouteRegistration,
    line: usize,
    middleware_index: &MiddlewareIndex,
) -> Vec<RouteEntry> {
    resource_routes(resource, controller, api, singleton)
        .into_iter()
        .map(|spec| RouteEntry {
            file: strip_root(project_root, file),
            line,
            column: 1,
            methods: spec.methods,
            uri: join_uri(
                &context.uri_prefix,
                &format!("{}{}", resource.trim_matches('/'), spec.suffix),
            ),
            name: Some(format!("{}{}", context.name_prefix, spec.name_suffix)),
            action: Some(spec.action),
            middleware: context.middleware.clone(),
            resolved_middleware: super::context::resolve_middleware(
                &context.middleware,
                middleware_index,
            ),
            parameter_patterns: super::context::collect_parameter_patterns(
                &join_uri(
                    &context.uri_prefix,
                    &format!("{}{}", resource.trim_matches('/'), spec.suffix),
                ),
                middleware_index,
            ),
            registration: registration.clone(),
        })
        .collect()
}

// Returns true when a `group` call was handled (caller should stop processing the chain).
fn handle_group(
    method_name: &str,
    raw_chunk: Option<&[u8]>,
    args: &[php_parser::ast::Arg<'_>],
    source: &[u8],
    project_root: &Path,
    file: &Path,
    registration: &RouteRegistration,
    line_offset: usize,
    context: &RouteContext,
    middleware_index: &MiddlewareIndex,
    routes: &mut Vec<RouteEntry>,
) -> bool {
    if method_name != "group" {
        return false;
    }

    if let Some(raw_chunk) = raw_chunk {
        if let Some((body, body_line)) = extract_group_body(raw_chunk, line_offset) {
            collect_routes_from_source(
                &body,
                project_root,
                file,
                registration,
                body_line,
                context,
                middleware_index,
                routes,
            );
        }
    } else if let Some(body) = group_body_stmts(args) {
        walk_stmts(body, false, &mut |expr| {
            analyze_expr(
                expr,
                source,
                project_root,
                file,
                registration,
                context,
                middleware_index,
                routes,
                line_offset,
                None,
            );
        });
    }
    true
}

fn group_body_stmts<'ast>(
    args: &'ast [php_parser::ast::Arg<'ast>],
) -> Option<&'ast [StmtId<'ast>]> {
    let expr = args.first()?.value;
    match expr {
        Expr::Closure { body, .. } => Some(body),
        _ => None,
    }
}

pub(crate) fn split_route_chunks(source: &[u8], start_line: usize) -> Vec<RouteChunk> {
    let mut chunks = Vec::new();
    let mut line_starts = vec![0usize];
    for (i, &byte) in source.iter().enumerate() {
        if byte == b'\n' && i + 1 < source.len() {
            line_starts.push(i + 1);
        }
    }

    let mut current_start = None;
    let mut current_line = 0usize;
    let mut current_indent = 0usize;
    let mut state = ScanState::default();

    for (line_index, line_start) in line_starts.iter().copied().enumerate() {
        let line_end = source[line_start..]
            .iter()
            .position(|&b| b == b'\n')
            .map(|o| line_start + o + 1)
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

pub(crate) fn source_can_use_full_parse(source: &[u8]) -> bool {
    let mut state = ScanState::default();
    state.consume(source);
    state.is_balanced()
}

pub(crate) fn sanitize_closure_bodies(source: &[u8]) -> Vec<u8> {
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

fn extract_group_body(source: &[u8], base_line: usize) -> Option<(Vec<u8>, usize)> {
    let group_index = find_bytes(source, b"group")?;
    let function_index = find_bytes(&source[group_index..], b"function")? + group_index;
    let open_brace = find_closure_open_brace(source, function_index + "function".len())?;
    let body_start = open_brace + 1;
    let body_end = find_matching_brace(source, open_brace).unwrap_or(source.len());
    let body_line = base_line + count_newlines(&source[..body_start]);
    Some((source[body_start..body_end].to_vec(), body_line))
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

fn starts_with_keyword(source: &[u8], index: usize, keyword: &[u8]) -> bool {
    source
        .get(index..index + keyword.len())
        .is_some_and(|s| s == keyword)
        && !source
            .get(index.wrapping_sub(1))
            .is_some_and(|b| b.is_ascii_alphanumeric() || *b == b'_')
        && !source
            .get(index + keyword.len())
            .is_some_and(|b| b.is_ascii_alphanumeric() || *b == b'_')
}

fn trim_ascii_start(bytes: &[u8]) -> &[u8] {
    let mut i = 0;
    while let Some(b) = bytes.get(i) {
        if !b.is_ascii_whitespace() {
            break;
        }
        i += 1;
    }
    &bytes[i..]
}

fn find_bytes(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|w| w == needle)
}

fn count_newlines(bytes: &[u8]) -> usize {
    bytes.iter().filter(|&&b| b == b'\n').count()
}
