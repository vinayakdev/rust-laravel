use bumpalo::Bump;
use php_parser::ast::{Expr, ExprId};
use php_parser::lexer::Lexer;
use php_parser::parser::Parser;
use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use crate::analyzers::providers;
use crate::php::ast::{expr_name, expr_to_string, expr_to_string_list};
use crate::php::walk::walk_stmts;
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
            .map_err(|e| format!("failed to read {}: {e}", source_file.display()))?;

        let result = extract_middleware(&provider.provider_class, relative_source_file, &source);
        aliases.extend(result.aliases);
        groups.extend(result.groups);
        patterns.extend(result.patterns);
    }

    aliases.sort_by(|l, r| l.name.cmp(&r.name));
    groups.sort_by(|l, r| l.name.cmp(&r.name));
    patterns.sort_by(|l, r| l.parameter.cmp(&r.parameter));

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

fn extract_middleware(provider_class: &str, declared_in: &Path, source: &[u8]) -> ExtractionResult {
    let arena = Bump::new();
    let lexer = Lexer::new(source);
    let mut parser = Parser::new(lexer, &arena);
    let program = parser.parse_program();

    let mut result = ExtractionResult {
        aliases: Vec::new(),
        groups: Vec::new(),
        patterns: Vec::new(),
    };

    if program.errors.is_empty() {
        walk_stmts(program.statements, true, &mut |expr| {
            visit_expr(expr, source, provider_class, declared_in, &mut result);
        });
    }

    result
}

fn visit_expr(
    expr: ExprId<'_>,
    source: &[u8],
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
    let source_ref = MiddlewareSource {
        declared_in: declared_in.to_path_buf(),
        line: line_info.map_or(1, |i| i.line),
        column: line_info.map_or(1, |i| i.column),
        provider_class: provider_class.to_string(),
    };

    match method_name.as_str() {
        "aliasMiddleware" if args.len() >= 2 => {
            if let (Some(name), Some(target)) = (
                args.first().and_then(|a| expr_to_string(a.value, source)),
                args.get(1).and_then(|a| expr_to_string(a.value, source)),
            ) {
                result.aliases.push(MiddlewareAlias {
                    name,
                    target,
                    source: source_ref,
                });
            }
        }
        "middlewareGroup" if args.len() >= 2 => {
            if let Some(name) = args.first().and_then(|a| expr_to_string(a.value, source)) {
                let members = args
                    .get(1)
                    .map(|a| expr_to_string_list(a.value, source))
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
                args.first().and_then(|a| expr_to_string(a.value, source)),
                args.get(1).and_then(|a| expr_to_string(a.value, source)),
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
}
