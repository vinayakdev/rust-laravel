use bumpalo::Bump;
use php_parser::ast::{ClassMember, Expr, Stmt};
use php_parser::lexer::Lexer;
use php_parser::parser::Parser;
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use crate::analyzers::providers;
use crate::php::ast::{
    byte_offset_to_line_col, expr_name, expr_to_path, expr_to_string, expr_to_string_list,
    span_text, strip_root,
};
use crate::php::walk::walk_stmts;
use crate::project::LaravelProject;
use crate::types::{ColumnEntry, IndexEntry, MigrationEntry, MigrationReport};

pub fn analyze(project: &LaravelProject) -> Result<MigrationReport, String> {
    let mut files = collect_migration_files(project)?;
    files.sort();
    files.dedup();

    let mut migrations = files
        .into_iter()
        .filter_map(|file| parse_migration_file(project, &file))
        .collect::<Vec<_>>();

    migrations.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));

    Ok(MigrationReport {
        project_name: project.name.clone(),
        project_root: project.root.clone(),
        migration_count: migrations.len(),
        migrations,
    })
}

fn collect_migration_files(project: &LaravelProject) -> Result<Vec<PathBuf>, String> {
    let mut files = Vec::new();

    // 1. Conventional database/migrations/
    scan_migrations_dir(&project.root.join("database/migrations"), &mut files);

    // 2. packages/<vendor>/<name>/database/migrations/
    let packages_root = project.root.join("packages");
    if let Ok(vendors) = fs::read_dir(&packages_root) {
        for vendor in vendors.flatten() {
            if let Ok(pkgs) = fs::read_dir(vendor.path()) {
                for pkg in pkgs.flatten() {
                    scan_migrations_dir(&pkg.path().join("database/migrations"), &mut files);
                }
            }
        }
    }

    // 3. Provider-registered directories via loadMigrationsFrom()
    files.extend(discover_provider_migration_files(project)?);

    Ok(files)
}

/// Walk every registered service provider looking for `loadMigrationsFrom(path)` calls.
/// Same pattern as routes (loadRoutesFrom) and configs (mergeConfigFrom).
fn discover_provider_migration_files(project: &LaravelProject) -> Result<Vec<PathBuf>, String> {
    let provider_report = providers::analyze(project)?;
    let mut files = Vec::new();
    let mut seen_providers = BTreeSet::new();

    for provider in provider_report.providers {
        let Some(relative_source) = provider.source_file.as_ref() else {
            continue;
        };
        if !provider.source_available {
            continue;
        }
        if !seen_providers.insert((provider.provider_class.clone(), relative_source.clone())) {
            continue;
        }

        let provider_file = project.root.join(relative_source);
        let source = fs::read(&provider_file)
            .map_err(|e| format!("failed to read {}: {e}", provider_file.display()))?;

        for dir in extract_migration_dirs_from_provider(project, &provider_file, &source) {
            scan_migrations_dir(&dir, &mut files);
        }
    }

    Ok(files)
}

fn extract_migration_dirs_from_provider(
    project: &LaravelProject,
    provider_file: &Path,
    source: &[u8],
) -> Vec<PathBuf> {
    let arena = Bump::new();
    let lexer = Lexer::new(source);
    let mut parser = Parser::new(lexer, &arena);
    let program = parser.parse_program();
    if !program.errors.is_empty() {
        return Vec::new();
    }

    let mut dirs = Vec::new();
    walk_stmts(program.statements, true, &mut |expr| {
        let Expr::MethodCall { method, args, .. } = expr else {
            return;
        };
        if expr_name(method, source).as_deref() != Some("loadMigrationsFrom") {
            return;
        }
        if let Some(path_arg) = args.first().map(|a| a.value) {
            if let Some(dir) = expr_to_path(path_arg, source, &project.root, provider_file) {
                dirs.push(dir);
            }
        }
    });
    dirs
}

fn scan_migrations_dir(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    let mut batch: Vec<PathBuf> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("php"))
        .collect();
    batch.sort();
    out.extend(batch);
}

fn parse_migration_file(project: &LaravelProject, file: &Path) -> Option<MigrationEntry> {
    let source = fs::read(file).ok()?;
    let arena = Bump::new();
    let lexer = Lexer::new(&source);
    let mut parser = Parser::new(lexer, &arena);
    let program = parser.parse_program();

    let timestamp = file
        .file_stem()
        .and_then(|s| s.to_str())
        .map(|s| s.chars().take(17).collect::<String>())
        .unwrap_or_default();

    let relative = strip_root(&project.root, file);

    for stmt in program.statements.iter() {
        // Named class: class CreateXxx extends Migration { ... }
        if let Stmt::Class { name, members, .. } = stmt {
            let class_name = span_text(name.span, &source);
            if let Some(entry) =
                find_up_method(members, &source, &class_name, &timestamp, &relative)
            {
                return Some(entry);
            }
        }

        // Anonymous class: return new class extends Migration { ... }
        // Parsed as Stmt::Return { expr: Expr::New { class: Expr::AnonymousClass { .. } } }
        if let Stmt::Return {
            expr: Some(expr), ..
        } = stmt
        {
            if let Expr::New { class, .. } = *expr {
                if let Expr::AnonymousClass { members, .. } = *class {
                    let class_name = timestamp.clone();
                    if let Some(entry) =
                        find_up_method(members, &source, &class_name, &timestamp, &relative)
                    {
                        return Some(entry);
                    }
                }
            }
        }
    }
    None
}

fn find_up_method(
    members: &[ClassMember<'_>],
    source: &[u8],
    class_name: &str,
    timestamp: &str,
    relative: &Path,
) -> Option<MigrationEntry> {
    for member in members.iter().copied() {
        let ClassMember::Method {
            name: method_name,
            body,
            ..
        } = member
        else {
            continue;
        };
        if span_text(method_name.span, source) != "up" {
            continue;
        }
        return Some(
            extract_schema_call(body, source, class_name, timestamp, relative).unwrap_or_else(
                || MigrationEntry {
                    file: relative.to_path_buf(),
                    timestamp: timestamp.to_string(),
                    class_name: class_name.to_string(),
                    table: String::new(),
                    operation: "unknown".to_string(),
                    columns: Vec::new(),
                    indexes: Vec::new(),
                    dropped_columns: Vec::new(),
                },
            ),
        );
    }
    None
}

fn extract_schema_call(
    stmts: &[php_parser::ast::StmtId<'_>],
    source: &[u8],
    class_name: &str,
    timestamp: &str,
    relative: &Path,
) -> Option<MigrationEntry> {
    for stmt in stmts {
        match stmt {
            Stmt::Expression { expr, .. } => {
                if let Some(entry) = try_schema_expr(*expr, source, class_name, timestamp, relative)
                {
                    return Some(entry);
                }
            }
            Stmt::Block { statements, .. } => {
                if let Some(entry) =
                    extract_schema_call(statements, source, class_name, timestamp, relative)
                {
                    return Some(entry);
                }
            }
            _ => {}
        }
    }
    None
}

fn try_schema_expr(
    expr: php_parser::ast::ExprId<'_>,
    source: &[u8],
    class_name: &str,
    timestamp: &str,
    relative: &Path,
) -> Option<MigrationEntry> {
    let Expr::StaticCall {
        class,
        method,
        args,
        ..
    } = expr
    else {
        return None;
    };
    let class_text = span_text(class.span(), source);
    if class_text != "Schema" && class_text != "\\Schema" {
        return None;
    }
    let method_text = span_text(method.span(), source);
    let operation = match method_text.as_str() {
        "create" => "create",
        "table" => "alter",
        _ => return None,
    };

    let table_arg = args.first()?;
    let table = expr_to_string(table_arg.value, source)?;

    let closure_arg = args.get(1)?;
    let closure_body = match closure_arg.value {
        Expr::Closure { body, .. } => body,
        _ => return None,
    };

    let mut columns: Vec<ColumnEntry> = Vec::new();
    let mut indexes: Vec<IndexEntry> = Vec::new();
    let mut dropped: Vec<String> = Vec::new();

    for stmt in closure_body.iter() {
        let Stmt::Expression { expr, .. } = stmt else {
            continue;
        };
        process_table_call(*expr, source, &mut columns, &mut indexes, &mut dropped);
    }

    Some(MigrationEntry {
        file: relative.to_path_buf(),
        timestamp: timestamp.to_string(),
        class_name: class_name.to_string(),
        table,
        operation: operation.to_string(),
        columns,
        indexes,
        dropped_columns: dropped,
    })
}

fn process_table_call(
    expr: php_parser::ast::ExprId<'_>,
    source: &[u8],
    columns: &mut Vec<ColumnEntry>,
    indexes: &mut Vec<IndexEntry>,
    dropped: &mut Vec<String>,
) {
    let mut chain: Vec<(String, Vec<php_parser::ast::ExprId<'_>>)> = Vec::new();
    if !flatten_method_chain(expr, source, &mut chain) {
        return;
    }
    if chain.is_empty() {
        return;
    }

    let (first_method, first_args) = &chain[0];

    match first_method.as_str() {
        "dropColumn" | "removeColumn" => {
            for arg in first_args {
                dropped.extend(expr_to_string_list(*arg, source));
            }
            return;
        }
        "index" => {
            if let Some(cols) = first_args.first().map(|a| expr_to_string_list(*a, source)) {
                if !cols.is_empty() {
                    indexes.push(IndexEntry {
                        columns: cols,
                        index_type: "index".to_string(),
                    });
                }
            }
            return;
        }
        "unique" => {
            if let Some(arg) = first_args.first() {
                let cols = expr_to_string_list(*arg, source);
                if !cols.is_empty() {
                    indexes.push(IndexEntry {
                        columns: cols,
                        index_type: "unique".to_string(),
                    });
                    return;
                }
            }
        }
        "primary" => {
            if let Some(arg) = first_args.first() {
                let cols = expr_to_string_list(*arg, source);
                if !cols.is_empty() {
                    indexes.push(IndexEntry {
                        columns: cols,
                        index_type: "primary".to_string(),
                    });
                    return;
                }
            }
        }
        "foreign" => {
            // Standalone ->foreign('col')->references('id')->on('users')
            // Parse the FK chain
            if let Some(col) = first_args.first().and_then(|a| expr_to_string(*a, source)) {
                let references = chain
                    .iter()
                    .find(|(m, _)| m == "references")
                    .and_then(|(_, a)| a.first())
                    .and_then(|a| expr_to_string(*a, source));
                let on_table = chain
                    .iter()
                    .find(|(m, _)| m == "on")
                    .and_then(|(_, a)| a.first())
                    .and_then(|a| expr_to_string(*a, source));
                // Update the existing column if present
                if let Some(existing) = columns.iter_mut().find(|c| c.name == col) {
                    existing.references = references;
                    existing.on_table = on_table;
                }
            }
            return;
        }
        "timestamps" | "nullableTimestamps" => {
            let nullable = first_method == "nullableTimestamps";
            columns.push(ColumnEntry {
                name: "created_at".to_string(),
                column_type: "timestamp".to_string(),
                nullable: true,
                default: None,
                unique: false,
                unsigned: false,
                primary: false,
                enum_values: Vec::new(),
                comment: None,
                references: None,
                on_table: None,
            });
            columns.push(ColumnEntry {
                name: "updated_at".to_string(),
                column_type: "timestamp".to_string(),
                nullable: true,
                default: None,
                unique: false,
                unsigned: false,
                primary: false,
                enum_values: Vec::new(),
                comment: None,
                references: None,
                on_table: None,
            });
            let _ = nullable;
            return;
        }
        "softDeletes" | "softDeletesTz" => {
            columns.push(ColumnEntry {
                name: "deleted_at".to_string(),
                column_type: "timestamp".to_string(),
                nullable: true,
                default: None,
                unique: false,
                unsigned: false,
                primary: false,
                enum_values: Vec::new(),
                comment: None,
                references: None,
                on_table: None,
            });
            return;
        }
        "rememberToken" => {
            columns.push(ColumnEntry {
                name: "remember_token".to_string(),
                column_type: "string".to_string(),
                nullable: true,
                default: None,
                unique: false,
                unsigned: false,
                primary: false,
                enum_values: Vec::new(),
                comment: None,
                references: None,
                on_table: None,
            });
            return;
        }
        "morphs" | "nullableMorphs" | "uuidMorphs" | "nullableUuidMorphs" => {
            let nullable = first_method.contains("nullable");
            let uuid = first_method.contains("uuid") || first_method.contains("Uuid");
            let col_name = first_args
                .first()
                .and_then(|a| expr_to_string(*a, source))
                .unwrap_or_else(|| "morphable".to_string());
            columns.push(ColumnEntry {
                name: format!("{col_name}_type"),
                column_type: "string".to_string(),
                nullable,
                default: None,
                unique: false,
                unsigned: false,
                primary: false,
                enum_values: Vec::new(),
                comment: None,
                references: None,
                on_table: None,
            });
            columns.push(ColumnEntry {
                name: format!("{col_name}_id"),
                column_type: if uuid { "uuid" } else { "unsignedBigInteger" }.to_string(),
                nullable,
                default: None,
                unique: false,
                unsigned: !uuid,
                primary: false,
                enum_values: Vec::new(),
                comment: None,
                references: None,
                on_table: None,
            });
            return;
        }
        _ => {}
    }

    // Regular column definition
    let col_name = first_args.first().and_then(|a| expr_to_string(*a, source));

    let (col_type, col_name, auto_unsigned, auto_primary) = match first_method.as_str() {
        "id" => ("bigIncrements".to_string(), "id".to_string(), true, true),
        "bigIncrements" => {
            let name = col_name.unwrap_or_else(|| "id".to_string());
            ("bigIncrements".to_string(), name, true, true)
        }
        "increments" => {
            let name = col_name.unwrap_or_else(|| "id".to_string());
            ("increments".to_string(), name, true, true)
        }
        "smallIncrements" => (
            "smallIncrements".to_string(),
            col_name.unwrap_or_else(|| "id".to_string()),
            true,
            true,
        ),
        "tinyIncrements" => (
            "tinyIncrements".to_string(),
            col_name.unwrap_or_else(|| "id".to_string()),
            true,
            true,
        ),
        "mediumIncrements" => (
            "mediumIncrements".to_string(),
            col_name.unwrap_or_else(|| "id".to_string()),
            true,
            true,
        ),
        "foreignId" => {
            let name = col_name.unwrap_or_default();
            ("unsignedBigInteger".to_string(), name, true, false)
        }
        "foreignUuid" => {
            let name = col_name.unwrap_or_default();
            ("uuid".to_string(), name, false, false)
        }
        "unsignedBigInteger"
        | "unsignedInteger"
        | "unsignedSmallInteger"
        | "unsignedTinyInteger"
        | "unsignedMediumInteger" => {
            let name = col_name.unwrap_or_default();
            (first_method.clone(), name, true, false)
        }
        _ => {
            let Some(name) = col_name else { return };
            let ct = column_type_str(first_method);
            (ct, name, false, false)
        }
    };

    if col_name.is_empty() {
        return;
    }

    let mut entry = ColumnEntry {
        name: col_name,
        column_type: col_type,
        nullable: false,
        default: None,
        unique: auto_primary,
        unsigned: auto_unsigned,
        primary: auto_primary,
        enum_values: Vec::new(),
        comment: None,
        references: None,
        on_table: None,
    };

    // For enum/set, second arg is the values array
    if (first_method == "enum" || first_method == "set") && first_args.len() >= 2 {
        entry.enum_values = expr_to_string_list(first_args[1], source);
    }

    // Apply modifiers from the rest of the chain
    let mut fk_references: Option<String> = None;
    let mut fk_on_table: Option<String> = None;
    for (modifier, mod_args) in chain.iter().skip(1) {
        match modifier.as_str() {
            "nullable" => entry.nullable = true,
            "unsigned" => entry.unsigned = true,
            "unique" => entry.unique = true,
            "primary" => entry.primary = true,
            "default" => {
                entry.default = mod_args.first().map(|a| {
                    span_text(a.span(), source)
                        .trim_matches('\'')
                        .trim_matches('"')
                        .to_string()
                });
            }
            "comment" => {
                entry.comment = mod_args.first().and_then(|a| expr_to_string(*a, source));
            }
            "references" => {
                fk_references = mod_args.first().and_then(|a| expr_to_string(*a, source));
            }
            "on" => {
                fk_on_table = mod_args.first().and_then(|a| expr_to_string(*a, source));
            }
            _ => {}
        }
    }
    if fk_references.is_some() {
        entry.references = fk_references;
        entry.on_table = fk_on_table;
    }

    columns.push(entry);
}

/// Flattens a method call chain rooted at `$table` variable.
/// Returns true if the chain root is a `$table`-like variable.
/// `out` is filled in call order (innermost first).
fn flatten_method_chain<'a>(
    expr: php_parser::ast::ExprId<'a>,
    source: &[u8],
    out: &mut Vec<(String, Vec<php_parser::ast::ExprId<'a>>)>,
) -> bool {
    match expr {
        Expr::MethodCall {
            target,
            method,
            args,
            ..
        } => {
            let method_name = span_text(method.span(), source);
            let is_root = flatten_method_chain(*target, source, out);
            if is_root {
                out.push((method_name, args.iter().map(|a| a.value).collect()));
            }
            is_root
        }
        Expr::Variable { name, .. } => {
            let var = span_text(*name, source);
            !var.contains("this") && !var.contains("self")
        }
        _ => false,
    }
}

fn column_type_str(method: &str) -> String {
    match method {
        "bigInteger" => "bigInteger",
        "binary" => "binary",
        "boolean" | "bool" => "boolean",
        "char" => "char",
        "date" => "date",
        "dateTime" | "datetime" => "dateTime",
        "dateTimeTz" => "dateTimeTz",
        "decimal" => "decimal",
        "double" => "double",
        "enum" => "enum",
        "float" => "float",
        "integer" | "int" => "integer",
        "ipAddress" => "ipAddress",
        "json" => "json",
        "jsonb" => "jsonb",
        "longText" => "longText",
        "macAddress" => "macAddress",
        "mediumInteger" => "mediumInteger",
        "mediumText" => "mediumText",
        "set" => "set",
        "smallInteger" => "smallInteger",
        "string" | "varchar" => "string",
        "text" => "text",
        "time" => "time",
        "timeTz" => "timeTz",
        "timestamp" => "timestamp",
        "timestampTz" => "timestampTz",
        "tinyInteger" => "tinyInteger",
        "tinyText" => "tinyText",
        "uuid" => "uuid",
        "ulid" => "ulid",
        "year" => "year",
        other => other,
    }
    .to_string()
}

/// Replay migrations for a given table name (sorted by timestamp) to produce
/// the current column state.
pub fn resolve_columns_for_table(table: &str, migrations: &[MigrationEntry]) -> Vec<ColumnEntry> {
    let mut columns: Vec<ColumnEntry> = Vec::new();
    let relevant: Vec<&MigrationEntry> = migrations.iter().filter(|m| m.table == table).collect();

    for migration in &relevant {
        if migration.operation == "create" {
            columns = migration.columns.clone();
        } else {
            for col in &migration.columns {
                if !columns.iter().any(|c| c.name == col.name) {
                    columns.push(col.clone());
                }
            }
            for dropped in &migration.dropped_columns {
                columns.retain(|c| &c.name != dropped);
            }
        }
    }
    columns
}

#[allow(dead_code)]
fn _byte_offset_hint(source: &[u8], offset: usize) -> (usize, usize) {
    byte_offset_to_line_col(source, offset)
}
