use bumpalo::Bump;
use php_parser::ast::{ClassMember, Expr, ExprId, Stmt, StmtId};
use php_parser::lexer::Lexer;
use php_parser::parser::Parser;
use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use crate::analyzers::providers;
use crate::project::LaravelProject;
use crate::types::{
    MiddlewareAlias, MiddlewareGroup, MiddlewareReport, MiddlewareSource, RoutePattern,
};

pub fn analyze(project: &LaravelProject) -> Result<MiddlewareReport, String> {
    let provider_report = providers::analyze(project)?;
    let mut aliases = Vec::new();
    let mut groups = Vec::new();
    let mut patterns = Vec::new();
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

        let result = extract_middleware(
            project,
            &provider.provider_class,
            relative_source_file,
            &source,
        );
        aliases.extend(result.aliases);
        groups.extend(result.groups);
        patterns.extend(result.patterns);
    }

    aliases.sort_by(|left, right| left.name.cmp(&right.name));
    groups.sort_by(|left, right| left.name.cmp(&right.name));
    patterns.sort_by(|left, right| left.parameter.cmp(&right.parameter));

    Ok(MiddlewareReport {
        project_name: project.name.clone(),
        project_root: project.root.clone(),
        alias_count: aliases.len(),
        group_count: groups.len(),
        pattern_count: patterns.len(),
        aliases,
        groups,
        patterns,
    })
}

struct ExtractionResult {
    aliases: Vec<MiddlewareAlias>,
    groups: Vec<MiddlewareGroup>,
    patterns: Vec<RoutePattern>,
}

fn extract_middleware(
    project: &LaravelProject,
    provider_class: &str,
    declared_in: &Path,
    source: &[u8],
) -> ExtractionResult {
    let arena = Bump::new();
    let lexer = Lexer::new(source);
    let mut parser = Parser::new(lexer, &arena);
    let program = parser.parse_program();
    if !program.errors.is_empty() {
        return ExtractionResult {
            aliases: Vec::new(),
            groups: Vec::new(),
            patterns: Vec::new(),
        };
    }

    let mut result = ExtractionResult {
        aliases: Vec::new(),
        groups: Vec::new(),
        patterns: Vec::new(),
    };

    for statement in program.statements {
        collect_from_stmt(
            statement,
            source,
            project,
            provider_class,
            declared_in,
            &mut result,
        );
    }

    result
}

fn collect_from_stmt(
    statement: StmtId<'_>,
    source: &[u8],
    project: &LaravelProject,
    provider_class: &str,
    declared_in: &Path,
    result: &mut ExtractionResult,
) {
    match statement {
        Stmt::Expression { expr, .. } => {
            collect_from_expr(expr, source, project, provider_class, declared_in, result);
        }
        Stmt::Block { statements, .. }
        | Stmt::Declare {
            body: statements, ..
        } => {
            for statement in *statements {
                collect_from_stmt(
                    statement,
                    source,
                    project,
                    provider_class,
                    declared_in,
                    result,
                );
            }
        }
        Stmt::Namespace {
            body: Some(body), ..
        } => {
            for statement in *body {
                collect_from_stmt(
                    statement,
                    source,
                    project,
                    provider_class,
                    declared_in,
                    result,
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
                        collect_from_stmt(
                            statement,
                            source,
                            project,
                            provider_class,
                            declared_in,
                            result,
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
                collect_from_stmt(
                    statement,
                    source,
                    project,
                    provider_class,
                    declared_in,
                    result,
                );
            }
            if let Some(else_block) = else_block {
                for statement in *else_block {
                    collect_from_stmt(
                        statement,
                        source,
                        project,
                        provider_class,
                        declared_in,
                        result,
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
                collect_from_stmt(
                    statement,
                    source,
                    project,
                    provider_class,
                    declared_in,
                    result,
                );
            }
        }
        _ => {}
    }
}

fn collect_from_expr(
    expr: ExprId<'_>,
    source: &[u8],
    project: &LaravelProject,
    provider_class: &str,
    declared_in: &Path,
    result: &mut ExtractionResult,
) {
    let Expr::StaticCall {
        class,
        method,
        args,
        ..
    } = expr
    else {
        return;
    };

    if expr_name(class, source).as_deref() != Some("Route") {
        return;
    }
    let Some(method_name) = expr_name(method, source) else {
        return;
    };

    let line_info = expr.span().line_info(source);
    let line = line_info.map_or(1, |info| info.line);
    let column = line_info.map_or(1, |info| info.column);
    let source_ref = MiddlewareSource {
        declared_in: declared_in.to_path_buf(),
        line,
        column,
        provider_class: provider_class.to_string(),
    };

    match method_name.as_str() {
        "aliasMiddleware" if args.len() >= 2 => {
            if let (Some(name), Some(target)) = (
                args.first()
                    .and_then(|arg| expr_to_string(arg.value, source)),
                args.get(1)
                    .and_then(|arg| expr_to_string(arg.value, source)),
            ) {
                result.aliases.push(MiddlewareAlias {
                    name,
                    target,
                    source: source_ref,
                });
            }
        }
        "middlewareGroup" if args.len() >= 2 => {
            if let Some(name) = args
                .first()
                .and_then(|arg| expr_to_string(arg.value, source))
            {
                let members = args
                    .get(1)
                    .map(|arg| expr_to_string_list(arg.value, source))
                    .unwrap_or_default();
                result.groups.push(MiddlewareGroup {
                    name,
                    members,
                    source: source_ref,
                });
            }
        }
        "pattern" if args.len() >= 2 => {
            if let (Some(parameter), Some(pattern)) = (
                args.first()
                    .and_then(|arg| expr_to_string(arg.value, source)),
                args.get(1)
                    .and_then(|arg| expr_to_string(arg.value, source)),
            ) {
                result.patterns.push(RoutePattern {
                    parameter,
                    pattern,
                    source: source_ref,
                });
            }
        }
        _ => {}
    }

    let _ = project;
}

fn expr_name(expr: ExprId<'_>, source: &[u8]) -> Option<String> {
    match expr {
        Expr::Variable { name, .. } => {
            Some(span_text(*name, source).trim_start_matches('$').to_string())
        }
        Expr::String { value, .. } => Some(String::from_utf8_lossy(value).into_owned()),
        _ => Some(span_text(expr.span(), source)),
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
