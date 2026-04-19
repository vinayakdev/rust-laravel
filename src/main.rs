use std::fs;
use std::path::{Path, PathBuf};

use bumpalo::Bump;
use php_parser::ast::{Arg, Expr, ExprId, Stmt, StmtId};
use php_parser::lexer::Lexer;
use php_parser::parser::Parser;

#[derive(Clone, Default)]
struct RouteContext {
    uri_prefix: String,
    name_prefix: String,
    middleware: Vec<String>,
    controller: Option<String>,
}

#[derive(Debug)]
struct RouteEntry {
    file: PathBuf,
    line: usize,
    methods: Vec<String>,
    uri: String,
    name: Option<String>,
    action: Option<String>,
    middleware: Vec<String>,
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

fn main() {
    let routes_dir = Path::new("routes");
    let files = collect_php_files(routes_dir);
    let mut routes = Vec::new();

    for path in &files {
        let source = match fs::read(path) {
            Ok(source) => source,
            Err(error) => {
                eprintln!("failed to read {}: {error}", path.display());
                continue;
            }
        };

        collect_routes_from_source(&source, path, 1, &RouteContext::default(), &mut routes);
    }

    routes.sort_by(|left, right| {
        left.file
            .cmp(&right.file)
            .then(left.line.cmp(&right.line))
            .then(left.uri.cmp(&right.uri))
    });

    print_routes(&routes);
}

fn print_routes(routes: &[RouteEntry]) {
    if routes.is_empty() {
        println!("No routes found.");
        return;
    }

    let method_width = routes
        .iter()
        .map(|route| route.methods.join("|").len())
        .max()
        .unwrap_or(6)
        .max("METHOD".len());
    let uri_width = routes
        .iter()
        .map(|route| route.uri.len())
        .max()
        .unwrap_or(3)
        .max("URI".len());
    let name_width = routes
        .iter()
        .map(|route| route.name.as_deref().unwrap_or("-").len())
        .max()
        .unwrap_or(4)
        .max("NAME".len());
    let action_width = routes
        .iter()
        .map(|route| route.action.as_deref().unwrap_or("-").len())
        .max()
        .unwrap_or(6)
        .max("ACTION".len());

    let mut current_file: Option<&Path> = None;

    for route in routes {
        let file = route.file.as_path();
        if current_file != Some(file) {
            if current_file.is_some() {
                println!();
            }

            println!("{}", file.display());
            println!(
                "  {line:>4}  {method:<method_width$}  {uri:<uri_width$}  {name:<name_width$}  {action:<action_width$}  {middleware}",
                line = "LINE",
                method = "METHOD",
                uri = "URI",
                name = "NAME",
                action = "ACTION",
                middleware = "MIDDLEWARE",
                method_width = method_width,
                uri_width = uri_width,
                name_width = name_width,
                action_width = action_width,
            );
            println!(
                "  {line:>4}  {method:-<method_width$}  {uri:-<uri_width$}  {name:-<name_width$}  {action:-<action_width$}  ----------",
                line = "",
                method = "",
                uri = "",
                name = "",
                action = "",
                method_width = method_width,
                uri_width = uri_width,
                name_width = name_width,
                action_width = action_width,
            );

            current_file = Some(file);
        }

        let methods = route.methods.join("|");
        let name = route.name.as_deref().unwrap_or("-");
        let action = route.action.as_deref().unwrap_or("-");
        let middleware = if route.middleware.is_empty() {
            "-"
        } else {
            &route.middleware.join(",")
        };

        println!(
            "  {line:>4}  {method:<method_width$}  {uri:<uri_width$}  {name:<name_width$}  {action:<action_width$}  {middleware}",
            line = route.line,
            method = methods,
            uri = route.uri,
            name = name,
            action = action,
            middleware = middleware,
            method_width = method_width,
            uri_width = uri_width,
            name_width = name_width,
            action_width = action_width,
        );
    }
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

fn collect_routes_from_source(
    source: &[u8],
    file: &Path,
    start_line: usize,
    context: &RouteContext,
    routes: &mut Vec<RouteEntry>,
) {
    for chunk in split_route_chunks(source, start_line) {
        parse_route_chunk(&chunk, file, context, routes);
    }
}

fn parse_route_chunk(
    chunk: &RouteChunk,
    file: &Path,
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
            file,
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
    file: &Path,
    context: &RouteContext,
    routes: &mut Vec<RouteEntry>,
    line_offset: usize,
    raw_chunk: Option<&[u8]>,
) {
    match statement {
        Stmt::Expression { expr, .. } => {
            analyze_route_expression(expr, source, file, context, routes, line_offset, raw_chunk);
        }
        Stmt::Block { statements, .. }
        | Stmt::Declare {
            body: statements, ..
        } => {
            for statement in *statements {
                collect_routes_from_stmt(
                    statement,
                    source,
                    file,
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
                    file,
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
                    file,
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
                        file,
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
                    file,
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
    file: &Path,
    base_context: &RouteContext,
    routes: &mut Vec<RouteEntry>,
    line_offset: usize,
    raw_chunk: Option<&[u8]>,
) {
    let Some(ops) = flatten_route_chain(expr) else {
        return;
    };

    let mut context = base_context.clone();
    let mut current_route: Option<RouteEntry> = None;

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
                        file,
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
                    if let Some(raw_chunk) = raw_chunk
                        && let Some((body, body_line)) = extract_group_body(raw_chunk, line_offset)
                    {
                        collect_routes_from_source(&body, file, body_line, &context, routes);
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
                            file,
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
                    if let Some(raw_chunk) = raw_chunk
                        && let Some((body, body_line)) = extract_group_body(raw_chunk, line_offset)
                    {
                        collect_routes_from_source(&body, file, body_line, &context, routes);
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
    file: &Path,
    line: usize,
    signature: RouteSignature,
    args: &[Arg<'_>],
    source: &[u8],
) -> RouteEntry {
    let raw_uri = args
        .get(signature.uri_arg_index)
        .and_then(|arg| expr_to_string(arg.value, source))
        .unwrap_or_else(|| "/".to_string());

    let action = args
        .get(signature.action_arg_index)
        .and_then(|arg| expr_to_action(arg.value, context.controller.as_deref(), source));

    RouteEntry {
        file: file.to_path_buf(),
        line,
        methods: signature.methods,
        uri: join_uri(&context.uri_prefix, &raw_uri),
        name: (!context.name_prefix.is_empty()).then(|| context.name_prefix.clone()),
        action,
        middleware: context.middleware.clone(),
    }
}

fn route_signature(method_name: &str, args: &[Arg<'_>], source: &[u8]) -> Option<RouteSignature> {
    match method_name {
        "get" => Some(RouteSignature {
            methods: vec!["GET".to_string()],
            uri_arg_index: 0,
            action_arg_index: 1,
        }),
        "post" => Some(RouteSignature {
            methods: vec!["POST".to_string()],
            uri_arg_index: 0,
            action_arg_index: 1,
        }),
        "put" => Some(RouteSignature {
            methods: vec!["PUT".to_string()],
            uri_arg_index: 0,
            action_arg_index: 1,
        }),
        "patch" => Some(RouteSignature {
            methods: vec!["PATCH".to_string()],
            uri_arg_index: 0,
            action_arg_index: 1,
        }),
        "delete" => Some(RouteSignature {
            methods: vec!["DELETE".to_string()],
            uri_arg_index: 0,
            action_arg_index: 1,
        }),
        "options" => Some(RouteSignature {
            methods: vec!["OPTIONS".to_string()],
            uri_arg_index: 0,
            action_arg_index: 1,
        }),
        "any" => Some(RouteSignature {
            methods: vec!["ANY".to_string()],
            uri_arg_index: 0,
            action_arg_index: 1,
        }),
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

fn route_line(expr: ExprId<'_>, source: &[u8], line_offset: usize) -> usize {
    line_offset + chunk_relative_line(expr, source) - 1
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

fn find_bytes(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

fn count_newlines(bytes: &[u8]) -> usize {
    bytes.iter().filter(|byte| **byte == b'\n').count()
}
