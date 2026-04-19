use bumpalo::Bump;
use php_parser::ast::{Expr, Stmt, UseKind};
use php_parser::lexer::Lexer;
use php_parser::parser::Parser;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

use crate::php::ast::{byte_offset_to_line_col, span_text, strip_root};
use crate::php::psr4::{
    collect_psr4_mappings, laravel_providers, package_name_for_source, read_json,
    resolve_class_file, Psr4Mapping,
};
use crate::project::LaravelProject;
use crate::types::{ProviderEntry, ProviderReport};

pub fn analyze(project: &LaravelProject) -> Result<ProviderReport, String> {
    let mappings = collect_psr4_mappings(&project.root)?;
    let mut providers = Vec::new();

    providers.extend(read_bootstrap_providers(project, &mappings)?);
    providers.extend(read_root_composer_providers(project, &mappings)?);
    providers.extend(read_local_package_providers(project, &mappings)?);

    providers.sort_by(|l, r| {
        l.declared_in
            .cmp(&r.declared_in)
            .then(l.line.cmp(&r.line))
            .then(l.provider_class.cmp(&r.provider_class))
    });

    Ok(ProviderReport {
        project_name: project.name.clone(),
        project_root: project.root.clone(),
        provider_count: providers.len(),
        providers,
    })
}

#[derive(Clone)]
struct ClassReference {
    class: String,
    line: usize,
    column: usize,
}

fn read_bootstrap_providers(
    project: &LaravelProject,
    mappings: &[Psr4Mapping],
) -> Result<Vec<ProviderEntry>, String> {
    let path = project.root.join("bootstrap/providers.php");
    if !path.is_file() {
        return Ok(Vec::new());
    }
    let source = fs::read_to_string(&path)
        .map_err(|e| format!("failed to read {}: {e}", path.display()))?;
    Ok(extract_class_references(&source)
        .into_iter()
        .map(|r| build_provider_entry(project, &path, "bootstrap", None, r, mappings))
        .collect())
}

fn read_root_composer_providers(
    project: &LaravelProject,
    mappings: &[Psr4Mapping],
) -> Result<Vec<ProviderEntry>, String> {
    let path = project.root.join("composer.json");
    let composer = read_json(&path)?;
    let mut providers = Vec::new();

    if let Some(classes) = laravel_providers(&composer) {
        for class in classes {
            let (line, column) = find_json_string_position(&path, &class)?;
            providers.push(build_provider_entry(
                project,
                &path,
                "composer-discovered",
                None,
                ClassReference { class, line, column },
                mappings,
            ));
        }
    }
    Ok(providers)
}

fn read_local_package_providers(
    project: &LaravelProject,
    mappings: &[Psr4Mapping],
) -> Result<Vec<ProviderEntry>, String> {
    let mut providers = Vec::new();
    let packages_root = project.root.join("packages");

    if let Ok(vendors) = fs::read_dir(&packages_root) {
        for vendor in vendors.flatten() {
            let vendor_path = vendor.path();
            if !vendor_path.is_dir() {
                continue;
            }
            if let Ok(packages) = fs::read_dir(&vendor_path) {
                for package in packages.flatten() {
                    let package_path = package.path();
                    let composer_path = package_path.join("composer.json");
                    if !composer_path.is_file() {
                        continue;
                    }
                    let composer = read_json(&composer_path)?;
                    let package_name = composer
                        .get("name")
                        .and_then(serde_json::Value::as_str)
                        .map(ToString::to_string);

                    if let Some(classes) = laravel_providers(&composer) {
                        for class in classes {
                            let (line, column) =
                                find_json_string_position(&composer_path, &class)?;
                            providers.push(build_provider_entry(
                                project,
                                &composer_path,
                                "local-package-composer",
                                package_name.clone(),
                                ClassReference { class, line, column },
                                mappings,
                            ));
                        }
                    }
                }
            }
        }
    }
    Ok(providers)
}

fn extract_class_references(source: &str) -> Vec<ClassReference> {
    let bytes = source.as_bytes();
    let arena = Bump::new();
    let lexer = Lexer::new(bytes);
    let mut parser = Parser::new(lexer, &arena);
    let program = parser.parse_program();

    // Build import map: short name / alias → FQN.
    // Handles plain, aliased, and grouped use imports.
    let mut imports: HashMap<String, String> = HashMap::new();
    for stmt in program.statements.iter() {
        let Stmt::Use { uses, kind, .. } = stmt else {
            continue;
        };
        if *kind != UseKind::Normal {
            continue;
        }
        for item in *uses {
            let fqn = item
                .name
                .parts
                .iter()
                .map(|t| span_text(t.span, bytes))
                .collect::<String>()
                .trim_start_matches('\\')
                .to_string();
            let key = if let Some(alias_token) = item.alias {
                span_text(alias_token.span, bytes)
            } else {
                fqn.rsplit('\\').next().unwrap_or(&fqn).to_string()
            };
            imports.insert(key, fqn);
        }
    }

    let mut refs = Vec::new();
    collect_class_const_fetches(program.statements, bytes, &imports, &mut refs);
    refs
}

fn collect_class_const_fetches(
    stmts: &[php_parser::ast::StmtId<'_>],
    source: &[u8],
    imports: &HashMap<String, String>,
    out: &mut Vec<ClassReference>,
) {
    for stmt in stmts {
        match stmt {
            Stmt::Return { expr: Some(expr), .. } => {
                collect_from_expr(*expr, source, imports, out);
            }
            Stmt::Expression { expr, .. } => {
                collect_from_expr(*expr, source, imports, out);
            }
            _ => {}
        }
    }
}

fn collect_from_expr(
    expr: php_parser::ast::ExprId<'_>,
    source: &[u8],
    imports: &HashMap<String, String>,
    out: &mut Vec<ClassReference>,
) {
    match expr {
        Expr::Array { items, .. } => {
            for item in *items {
                collect_from_expr(item.value, source, imports, out);
            }
        }
        Expr::ClassConstFetch { class, constant, span } => {
            let constant_text = span_text(constant.span(), source);
            if constant_text.eq_ignore_ascii_case("class") {
                let raw = span_text(class.span(), source)
                    .trim_start_matches('\\')
                    .to_string();
                let resolved = if raw.contains('\\') {
                    raw
                } else {
                    imports.get(&raw).cloned().unwrap_or(raw)
                };
                let (line, column) = byte_offset_to_line_col(source, span.start);
                out.push(ClassReference { class: resolved, line, column });
            }
        }
        _ => {}
    }
}

fn build_provider_entry(
    project: &LaravelProject,
    declared_in: &Path,
    registration_kind: &str,
    package_name: Option<String>,
    reference: ClassReference,
    mappings: &[Psr4Mapping],
) -> ProviderEntry {
    let source_file = resolve_class_file(&reference.class, mappings);
    let package_name =
        package_name.or_else(|| package_name_for_source(&source_file, mappings));
    let source_available = source_file.is_some();
    let status = if source_available { "static_exact" } else { "source_missing" }.to_string();

    ProviderEntry {
        provider_class: reference.class,
        line: reference.line,
        column: reference.column,
        registration_kind: registration_kind.to_string(),
        declared_in: strip_root(&project.root, declared_in),
        package_name,
        source_file: source_file
            .as_ref()
            .map(|p| strip_root(&project.root, p)),
        source_available,
        status,
    }
}

fn find_json_string_position(path: &Path, needle: &str) -> Result<(usize, usize), String> {
    let text = fs::read_to_string(path)
        .map_err(|e| format!("failed to read {}: {e}", path.display()))?;
    let quoted = serde_json::to_string(needle)
        .map_err(|e| format!("failed to encode JSON string: {e}"))?;
    let index = text
        .find(&quoted)
        .ok_or_else(|| format!("failed to locate {needle} in {}", path.display()))?;
    let line = 1 + text[..index].bytes().filter(|&b| b == b'\n').count();
    let line_start = text[..index].rfind('\n').map_or(0, |o| o + 1);
    let column = index - line_start + 1;
    Ok((line, column))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_fully_qualified_class() {
        let src = r#"<?php
return [
    App\Modules\Blog\BlogServiceProvider::class,
];"#;
        let refs = extract_class_references(src);
        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0].class, "App\\Modules\\Blog\\BlogServiceProvider");
    }

    #[test]
    fn resolves_short_name_via_use_import() {
        let src = r#"<?php
use App\Modules\Blog\BlogServiceProvider;
use App\Providers\AppServiceProvider;

return [
    BlogServiceProvider::class,
    AppServiceProvider::class,
];"#;
        let refs = extract_class_references(src);
        assert_eq!(refs.len(), 2);
        assert_eq!(refs[0].class, "App\\Modules\\Blog\\BlogServiceProvider");
        assert_eq!(refs[1].class, "App\\Providers\\AppServiceProvider");
    }

    #[test]
    fn resolves_aliased_use_import() {
        let src = r#"<?php
use App\Modules\Blog\BlogServiceProvider as BlogSP;

return [
    BlogSP::class,
];"#;
        let refs = extract_class_references(src);
        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0].class, "App\\Modules\\Blog\\BlogServiceProvider");
    }

    #[test]
    fn resolves_grouped_use_import() {
        let src = r#"<?php
use App\Providers\{AppServiceProvider, CoreServiceProvider};

return [
    AppServiceProvider::class,
    CoreServiceProvider::class,
];"#;
        let refs = extract_class_references(src);
        assert_eq!(refs.len(), 2);
        assert_eq!(refs[0].class, "App\\Providers\\AppServiceProvider");
        assert_eq!(refs[1].class, "App\\Providers\\CoreServiceProvider");
    }

    #[test]
    fn unresolvable_short_name_kept_as_is() {
        let src = r#"<?php
return [
    SomeProvider::class,
];"#;
        let refs = extract_class_references(src);
        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0].class, "SomeProvider");
    }
}
