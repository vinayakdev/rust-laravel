use bumpalo::Bump;
use php_parser::ast::{Expr, ExprId};
use php_parser::lexer::Lexer;
use php_parser::parser::Parser;
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use crate::types::ConfigSource;
use rust_php_foundation::overrides::FileOverrides;
use rust_php_foundation::php::ast::{expr_name, expr_to_path, expr_to_string, strip_root};
use rust_php_foundation::php::walk::walk_stmts;
use rust_php_foundation::project::LaravelProject;
use rust_php_foundation::types::ProviderEntry;

#[derive(Clone)]
pub(crate) struct RegisteredConfigFile {
    pub(crate) file: PathBuf,
    pub(crate) namespace: String,
    pub(crate) source: ConfigSource,
}

pub(crate) fn collect_registered_config_files(
    project: &LaravelProject,
    providers: &[ProviderEntry],
    overrides: &FileOverrides,
) -> Result<Vec<RegisteredConfigFile>, String> {
    let config_dir = project.root.join("config");
    let mut files = Vec::new();
    let mut seen = BTreeSet::new();

    let entries = fs::read_dir(&config_dir)
        .map_err(|e| format!("failed to read {}: {e}", config_dir.display()))?;

    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("php") {
            continue;
        }
        let namespace = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("config")
            .to_string();

        seen.insert((path.clone(), namespace.clone()));
        files.push(RegisteredConfigFile {
            source: ConfigSource {
                kind: "config-file".to_string(),
                declared_in: strip_root(&project.root, &path),
                line: 1,
                column: 1,
                provider_class: None,
            },
            file: path,
            namespace,
        });
    }

    for merged in discover_provider_merged_configs(project, providers, overrides)? {
        if !merged.file.is_file() {
            continue;
        }
        if seen.contains(&(merged.file.clone(), merged.namespace.clone())) {
            continue;
        }
        files.push(RegisteredConfigFile {
            file: merged.file,
            namespace: merged.namespace,
            source: merged.source,
        });
    }

    files.sort_by(|l, r| {
        l.file
            .cmp(&r.file)
            .then(l.namespace.cmp(&r.namespace))
            .then(l.source.declared_in.cmp(&r.source.declared_in))
            .then(l.source.line.cmp(&r.source.line))
    });

    Ok(files)
}

struct MergedConfigFile {
    file: PathBuf,
    namespace: String,
    source: ConfigSource,
}

fn discover_provider_merged_configs(
    project: &LaravelProject,
    providers: &[ProviderEntry],
    overrides: &FileOverrides,
) -> Result<Vec<MergedConfigFile>, String> {
    let mut files = Vec::new();
    let mut seen_sources = BTreeSet::new();

    for provider in providers {
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
        let source = overrides.get_string(&source_file).map_or_else(
            || {
                fs::read_to_string(&source_file)
                    .map_err(|e| format!("failed to read {}: {e}", source_file.display()))
            },
            Ok,
        )?;
        files.extend(extract_merged_configs(
            project,
            &provider.provider_class,
            relative_source_file,
            &source_file,
            source.as_bytes(),
        ));
    }

    Ok(files)
}

fn extract_merged_configs(
    project: &LaravelProject,
    provider_class: &str,
    declared_in: &Path,
    provider_file: &Path,
    source: &[u8],
) -> Vec<MergedConfigFile> {
    let arena = Bump::new();
    let lexer = Lexer::new(source);
    let mut parser = Parser::new(lexer, &arena);
    let program = parser.parse_program();
    if !program.errors.is_empty() {
        return Vec::new();
    }

    let mut files = Vec::new();
    walk_stmts(program.statements, true, &mut |expr| {
        visit_expr(
            expr,
            source,
            project,
            provider_class,
            declared_in,
            provider_file,
            &mut files,
        );
    });
    files
}

fn visit_expr(
    expr: ExprId<'_>,
    source: &[u8],
    project: &LaravelProject,
    provider_class: &str,
    declared_in: &Path,
    provider_file: &Path,
    files: &mut Vec<MergedConfigFile>,
) {
    let Expr::MethodCall { method, args, .. } = expr else {
        return;
    };
    let Some(method_name) = expr_name(method, source) else {
        return;
    };
    if method_name != "mergeConfigFrom" {
        return;
    }

    let Some(path_expr) = args.first().map(|a| a.value) else {
        return;
    };
    let Some(namespace_expr) = args.get(1).map(|a| a.value) else {
        return;
    };

    let Some(config_file) = expr_to_path(path_expr, source, &project.root, provider_file) else {
        return;
    };
    let Some(namespace) = expr_to_string(namespace_expr, source) else {
        return;
    };

    let line_info = path_expr.span().line_info(source);
    files.push(MergedConfigFile {
        file: config_file,
        namespace,
        source: ConfigSource {
            kind: "provider-mergeConfigFrom".to_string(),
            declared_in: declared_in.to_path_buf(),
            line: line_info.map_or(1, |i| i.line),
            column: line_info.map_or(1, |i| i.column),
            provider_class: Some(provider_class.to_string()),
        },
    });
}
