pub mod types;

use bumpalo::Bump;
use php_parser::ast::{ClassMember, Expr, Stmt};
use php_parser::lexer::Lexer;
use php_parser::parser::Parser;
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use rust_php_foundation::overrides::FileOverrides;
use rust_php_foundation::php::ast::{
    byte_offset_to_line_col, expr_name, expr_to_path, expr_to_string, expr_to_string_list,
    span_text, strip_root,
};
use rust_php_foundation::php::walk::walk_stmts;
use rust_php_foundation::project::LaravelProject;
use rust_php_foundation::types::ProviderEntry;

use crate::types::{ColumnEntry, IndexEntry, MigrationEntry, MigrationReport};

pub fn analyze(
    project: &LaravelProject,
    providers: &[ProviderEntry],
    overrides: &FileOverrides,
) -> Result<MigrationReport, String> {
    let mut files = collect_migration_files(project, providers, overrides)?;
    files.sort();
    files.dedup();

    let mut migrations = files
        .into_iter()
        .flat_map(|file| parse_migration_file(project, &file))
        .collect::<Vec<_>>();

    migrations.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));

    Ok(MigrationReport {
        project_name: project.name.clone(),
        project_root: project.root.clone(),
        migration_count: migrations.len(),
        migrations,
    })
}

fn collect_migration_files(
    project: &LaravelProject,
    providers: &[ProviderEntry],
    overrides: &FileOverrides,
) -> Result<Vec<PathBuf>, String> {
    let mut files = Vec::new();

    scan_migrations_dir(&project.root.join("database/migrations"), &mut files);

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

    files.extend(discover_provider_migration_files(
        project, providers, overrides,
    )?);

    Ok(files)
}

fn discover_provider_migration_files(
    project: &LaravelProject,
    providers: &[ProviderEntry],
    overrides: &FileOverrides,
) -> Result<Vec<PathBuf>, String> {
    let mut files = Vec::new();
    let mut seen_providers = BTreeSet::new();

    for provider in providers {
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
        let source = overrides.get_bytes(&provider_file).map_or_else(
            || {
                fs::read(&provider_file)
                    .map_err(|error| format!("failed to read {}: {error}", provider_file.display()))
            },
            Ok,
        )?;

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
        if let Some(path_arg) = args.first().map(|arg| arg.value) {
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
        .map(|entry| entry.path())
        .filter(|path| path.extension().and_then(|ext| ext.to_str()) == Some("php"))
        .collect();
    batch.sort();
    out.extend(batch);
}

fn parse_migration_file(project: &LaravelProject, file: &Path) -> Vec<MigrationEntry> {
    let Ok(source) = fs::read(file) else {
        return Vec::new();
    };
    let arena = Bump::new();
    let lexer = Lexer::new(&source);
    let mut parser = Parser::new(lexer, &arena);
    let program = parser.parse_program();

    let timestamp = file
        .file_stem()
        .and_then(|stem| stem.to_str())
        .map(|stem| stem.chars().take(17).collect::<String>())
        .unwrap_or_default();

    let relative = strip_root(&project.root, file);
    let mut entries = Vec::new();

    for stmt in program.statements.iter() {
        if let Stmt::Class { name, members, .. } = stmt {
            let class_name = span_text(name.span, &source);
            entries.extend(find_up_method(
                members,
                &source,
                &class_name,
                &timestamp,
                &relative,
            ));
        }

        if let Stmt::Return {
            expr: Some(expr), ..
        } = stmt
        {
            if let Expr::New { class, .. } = *expr {
                if let Expr::AnonymousClass { members, .. } = *class {
                    let class_name = timestamp.clone();
                    entries.extend(find_up_method(
                        members,
                        &source,
                        &class_name,
                        &timestamp,
                        &relative,
                    ));
                }
            }
        }
    }

    entries
}

fn find_up_method(
    members: &[ClassMember<'_>],
    source: &[u8],
    class_name: &str,
    timestamp: &str,
    relative: &Path,
) -> Vec<MigrationEntry> {
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
        let entries = extract_schema_calls(body, source, class_name, timestamp, relative);
        if entries.is_empty() {
            return vec![MigrationEntry {
                file: relative.to_path_buf(),
                timestamp: timestamp.to_string(),
                class_name: class_name.to_string(),
                table: String::new(),
                operation: "unknown".to_string(),
                columns: Vec::new(),
                indexes: Vec::new(),
                dropped_columns: Vec::new(),
            }];
        }
        return entries;
    }
    Vec::new()
}

fn extract_schema_calls(
    stmts: &[php_parser::ast::StmtId<'_>],
    source: &[u8],
    class_name: &str,
    timestamp: &str,
    relative: &Path,
) -> Vec<MigrationEntry> {
    let mut entries = Vec::new();
    for stmt in stmts {
        match stmt {
            Stmt::Expression { expr, .. } => {
                if let Some(entry) = try_schema_expr(*expr, source, class_name, timestamp, relative)
                {
                    entries.push(entry);
                }
            }
            Stmt::Block { statements, .. } => {
                entries.extend(extract_schema_calls(
                    statements, source, class_name, timestamp, relative,
                ));
            }
            _ => {}
        }
    }
    entries
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
    let method_lc = method_text.to_ascii_lowercase();
    let operation = match method_lc.as_str() {
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
    if !flatten_method_chain(expr, source, &mut chain) || chain.is_empty() {
        return;
    }

    let (first_method, first_args) = &chain[0];
    let first_method_lc = first_method.to_ascii_lowercase();

    match first_method_lc.as_str() {
        "dropcolumn" | "removecolumn" => {
            for arg in first_args {
                dropped.extend(expr_to_string_list(*arg, source));
            }
            return;
        }
        "dropsoftdeletes" | "dropsoftdeletestz" => {
            dropped.push("deleted_at".to_string());
            return;
        }
        "droptimestamps" | "droptimestampstz" => {
            dropped.push("created_at".to_string());
            dropped.push("updated_at".to_string());
            return;
        }
        "index" => {
            if let Some(cols) = first_args
                .first()
                .map(|arg| expr_to_string_list(*arg, source))
            {
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
            if let Some(col) = first_args
                .first()
                .and_then(|arg| expr_to_string(*arg, source))
            {
                let references = chain
                    .iter()
                    .find(|(method, _)| method.eq_ignore_ascii_case("references"))
                    .and_then(|(_, args)| args.first())
                    .and_then(|arg| expr_to_string(*arg, source));
                let on_table = chain
                    .iter()
                    .find(|(method, _)| method.eq_ignore_ascii_case("on"))
                    .and_then(|(_, args)| args.first())
                    .and_then(|arg| expr_to_string(*arg, source));
                if let Some(existing) = columns.iter_mut().find(|entry| entry.name == col) {
                    existing.references = references;
                    existing.on_table = on_table;
                }
            }
            return;
        }
        "timestamps" | "nullabletimestamps" => {
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
            return;
        }
        "softdeletes" | "softdeletestz" => {
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
        "remembertoken" => {
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
        "morphs" | "nullablemorphs" | "uuidmorphs" | "nullableuuidmorphs" => {
            let nullable = first_method_lc.contains("nullable");
            let uuid = first_method_lc.contains("uuid");
            let col_name = first_args
                .first()
                .and_then(|arg| expr_to_string(*arg, source))
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

    let col_name = first_args
        .first()
        .and_then(|arg| expr_to_string(*arg, source));

    let (col_type, col_name, auto_unsigned, auto_primary) = match first_method_lc.as_str() {
        "id" => ("bigIncrements".to_string(), "id".to_string(), true, true),
        "bigincrements" => (
            "bigIncrements".to_string(),
            col_name.unwrap_or_else(|| "id".to_string()),
            true,
            true,
        ),
        "increments" => (
            "increments".to_string(),
            col_name.unwrap_or_else(|| "id".to_string()),
            true,
            true,
        ),
        "smallincrements" => (
            "smallIncrements".to_string(),
            col_name.unwrap_or_else(|| "id".to_string()),
            true,
            true,
        ),
        "tinyincrements" => (
            "tinyIncrements".to_string(),
            col_name.unwrap_or_else(|| "id".to_string()),
            true,
            true,
        ),
        "mediumincrements" => (
            "mediumIncrements".to_string(),
            col_name.unwrap_or_else(|| "id".to_string()),
            true,
            true,
        ),
        "foreignid" => (
            "unsignedBigInteger".to_string(),
            col_name.unwrap_or_default(),
            true,
            false,
        ),
        "foreignuuid" => (
            "uuid".to_string(),
            col_name.unwrap_or_default(),
            false,
            false,
        ),
        "unsignedbiginteger"
        | "unsignedinteger"
        | "unsignedsmallinteger"
        | "unsignedtinyinteger"
        | "unsignedmediuminteger" => (
            first_method.clone(),
            col_name.unwrap_or_default(),
            true,
            false,
        ),
        _ => {
            let Some(name) = col_name else {
                return;
            };
            (column_type_str(&first_method_lc), name, false, false)
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

    if (first_method_lc == "enum" || first_method_lc == "set") && first_args.len() >= 2 {
        entry.enum_values = expr_to_string_list(first_args[1], source);
    }

    let mut fk_references: Option<String> = None;
    let mut fk_on_table: Option<String> = None;
    for (modifier, mod_args) in chain.iter().skip(1) {
        match modifier.to_ascii_lowercase().as_str() {
            "nullable" => entry.nullable = true,
            "unsigned" => entry.unsigned = true,
            "unique" => entry.unique = true,
            "primary" => entry.primary = true,
            "default" => {
                entry.default = mod_args.first().map(|arg| {
                    span_text(arg.span(), source)
                        .trim_matches('\'')
                        .trim_matches('"')
                        .to_string()
                });
            }
            "comment" => {
                entry.comment = mod_args
                    .first()
                    .and_then(|arg| expr_to_string(*arg, source));
            }
            "references" => {
                fk_references = mod_args
                    .first()
                    .and_then(|arg| expr_to_string(*arg, source));
            }
            "on" => {
                fk_on_table = mod_args
                    .first()
                    .and_then(|arg| expr_to_string(*arg, source));
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
                out.push((method_name, args.iter().map(|arg| arg.value).collect()));
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
        "biginteger" => "bigInteger",
        "binary" => "binary",
        "boolean" | "bool" => "boolean",
        "char" => "char",
        "date" => "date",
        "datetime" => "dateTime",
        "datetimetz" => "dateTimeTz",
        "decimal" => "decimal",
        "double" => "double",
        "enum" => "enum",
        "float" => "float",
        "integer" | "int" => "integer",
        "ipaddress" => "ipAddress",
        "json" => "json",
        "jsonb" => "jsonb",
        "longtext" => "longText",
        "macaddress" => "macAddress",
        "mediuminteger" => "mediumInteger",
        "mediumtext" => "mediumText",
        "set" => "set",
        "smallinteger" => "smallInteger",
        "string" | "varchar" => "string",
        "text" => "text",
        "time" => "time",
        "timetz" => "timeTz",
        "timestamp" => "timestamp",
        "timestamptz" => "timestampTz",
        "tinyinteger" => "tinyInteger",
        "tinytext" => "tinyText",
        "uuid" => "uuid",
        "ulid" => "ulid",
        "year" => "year",
        other => other,
    }
    .to_string()
}

pub fn resolve_columns_for_table(table: &str, migrations: &[MigrationEntry]) -> Vec<ColumnEntry> {
    let mut columns: Vec<ColumnEntry> = Vec::new();
    let relevant: Vec<&MigrationEntry> = migrations
        .iter()
        .filter(|migration| migration.table == table)
        .collect();

    for migration in &relevant {
        if migration.operation == "create" {
            columns = migration.columns.clone();
        } else {
            for col in &migration.columns {
                if !columns.iter().any(|existing| existing.name == col.name) {
                    columns.push(col.clone());
                }
            }
            for dropped in &migration.dropped_columns {
                columns.retain(|column| &column.name != dropped);
            }
        }
    }
    columns
}

#[allow(dead_code)]
fn _byte_offset_hint(source: &[u8], offset: usize) -> (usize, usize) {
    byte_offset_to_line_col(source, offset)
}
