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
        .and_then(|s| s.to_str())
        .map(|s| s.chars().take(17).collect::<String>())
        .unwrap_or_default();

    let relative = strip_root(&project.root, file);
    let mut entries = Vec::new();

    for stmt in program.statements.iter() {
        // Named class: class CreateXxx extends Migration { ... }
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

        // Anonymous class: return new class extends Migration { ... }
        // Parsed as Stmt::Return { expr: Expr::New { class: Expr::AnonymousClass { .. } } }
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
    if !flatten_method_chain(expr, source, &mut chain) {
        return;
    }
    if chain.is_empty() {
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
                    .find(|(m, _)| m.eq_ignore_ascii_case("references"))
                    .and_then(|(_, a)| a.first())
                    .and_then(|a| expr_to_string(*a, source));
                let on_table = chain
                    .iter()
                    .find(|(m, _)| m.eq_ignore_ascii_case("on"))
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
        "timestamps" | "nullabletimestamps" => {
            let nullable = first_method_lc == "nullabletimestamps";
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

    let (col_type, col_name, auto_unsigned, auto_primary) = match first_method_lc.as_str() {
        "id" => ("bigIncrements".to_string(), "id".to_string(), true, true),
        "bigincrements" => {
            let name = col_name.unwrap_or_else(|| "id".to_string());
            ("bigIncrements".to_string(), name, true, true)
        }
        "increments" => {
            let name = col_name.unwrap_or_else(|| "id".to_string());
            ("increments".to_string(), name, true, true)
        }
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
        "foreignid" => {
            let name = col_name.unwrap_or_default();
            ("unsignedBigInteger".to_string(), name, true, false)
        }
        "foreignuuid" => {
            let name = col_name.unwrap_or_default();
            ("uuid".to_string(), name, false, false)
        }
        "unsignedbiginteger"
        | "unsignedinteger"
        | "unsignedsmallinteger"
        | "unsignedtinyinteger"
        | "unsignedmediuminteger" => {
            let name = col_name.unwrap_or_default();
            (first_method.clone(), name, true, false)
        }
        _ => {
            let Some(name) = col_name else { return };
            let ct = column_type_str(&first_method_lc);
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
    if (first_method_lc == "enum" || first_method_lc == "set") && first_args.len() >= 2 {
        entry.enum_values = expr_to_string_list(first_args[1], source);
    }

    // Apply modifiers from the rest of the chain
    let mut fk_references: Option<String> = None;
    let mut fk_on_table: Option<String> = None;
    for (modifier, mod_args) in chain.iter().skip(1) {
        match modifier.to_ascii_lowercase().as_str() {
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn make_temp_project() -> Result<(LaravelProject, PathBuf), Box<dyn std::error::Error>> {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)?
            .as_nanos()
            .to_string();
        let root = std::env::temp_dir().join(format!("rust_php_migrations_{unique}"));

        fs::create_dir_all(root.join("config"))?;
        fs::create_dir_all(root.join("routes"))?;
        fs::create_dir_all(root.join("database/migrations"))?;
        fs::write(root.join("composer.json"), r#"{"autoload":{"psr-4":{}}}"#)?;

        Ok((
            LaravelProject {
                root: root.clone(),
                name: "temp-project".to_string(),
            },
            root,
        ))
    }

    #[test]
    fn captures_multiple_schema_calls_in_one_migration_file()
    -> Result<(), Box<dyn std::error::Error>> {
        let (project, root) = make_temp_project()?;
        let migration = root.join("database/migrations/2024_01_10_090000_create_blogs_table.php");

        fs::write(
            &migration,
            r#"<?php

use Illuminate\Database\Migrations\Migration;
use Illuminate\Database\Schema\Blueprint;
use Illuminate\Support\Facades\Schema;

return new class extends Migration {
    public function up(): void
    {
        Schema::create('blogs', static function (Blueprint $table) {
            $table->id();
            $table->string('title')->nullable();
        });

        Schema::table('blogs', static function (Blueprint $table) {
            $table->string('name')->nullable();
        });
    }
};
"#,
        )?;

        let report = analyze(&project).map_err(std::io::Error::other)?;
        let columns = resolve_columns_for_table("blogs", &report.migrations);
        let column_names = columns.iter().map(|c| c.name.as_str()).collect::<Vec<_>>();

        assert_eq!(report.migrations.len(), 2);
        assert_eq!(column_names, vec!["id", "title", "name"]);

        fs::remove_dir_all(root)?;
        Ok(())
    }

    #[test]
    fn drops_column_added_earlier_in_same_migration_file_case_insensitively()
    -> Result<(), Box<dyn std::error::Error>> {
        let (project, root) = make_temp_project()?;
        let migration = root.join("database/migrations/2024_01_10_090000_create_blogs_table.php");

        fs::write(
            &migration,
            r#"<?php

use Illuminate\Database\Migrations\Migration;
use Illuminate\Database\Schema\Blueprint;
use Illuminate\Support\Facades\Schema;

return new class extends Migration {
    public function up(): void
    {
        Schema::create('blogs', static function (Blueprint $table) {
            $table->id();
        });

        Schema::table('blogs', static function (Blueprint $table) {
            $table->string('name')->nullable();
        });

        Schema::table('blogs', static function (Blueprint $table) {
            $table->dropcolumn('name');
        });
    }
};
"#,
        )?;

        let report = analyze(&project).map_err(std::io::Error::other)?;
        let columns = resolve_columns_for_table("blogs", &report.migrations);
        let column_names = columns.iter().map(|c| c.name.as_str()).collect::<Vec<_>>();

        assert_eq!(report.migrations.len(), 3);
        assert_eq!(column_names, vec!["id"]);

        fs::remove_dir_all(root)?;
        Ok(())
    }

    #[test]
    fn drops_soft_deletes_and_timestamps_helpers() -> Result<(), Box<dyn std::error::Error>> {
        let (project, root) = make_temp_project()?;
        let migration = root.join("database/migrations/2024_01_10_090000_create_blogs_table.php");

        fs::write(
            &migration,
            r#"<?php

use Illuminate\Database\Migrations\Migration;
use Illuminate\Database\Schema\Blueprint;
use Illuminate\Support\Facades\Schema;

return new class extends Migration {
    public function up(): void
    {
        Schema::create('blogs', static function (Blueprint $table) {
            $table->id();
            $table->softDeletes();
            $table->timestamps();
        });

        Schema::table('blogs', static function (Blueprint $table) {
            $table->dropSoftDeletes();
            $table->dropTimestamps();
        });
    }
};
"#,
        )?;

        let report = analyze(&project).map_err(std::io::Error::other)?;
        let columns = resolve_columns_for_table("blogs", &report.migrations);
        let column_names = columns.iter().map(|c| c.name.as_str()).collect::<Vec<_>>();

        assert_eq!(column_names, vec!["id"]);

        fs::remove_dir_all(root)?;
        Ok(())
    }
}
