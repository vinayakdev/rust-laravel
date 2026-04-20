use bumpalo::Bump;
use php_parser::ast::{ClassMember, Expr, Name, Stmt, UseKind};
use php_parser::lexer::Lexer;
use php_parser::parser::Parser;
use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::path::{Path, PathBuf};

use crate::analyzers::migrations;
use crate::php::ast::{
    byte_offset_to_line_col, expr_to_string, expr_to_string_list, span_text, strip_root,
};
use crate::php::psr4::{Psr4Mapping, collect_psr4_mappings, resolve_class_file};
use crate::project::LaravelProject;
use crate::types::{ColumnEntry, ModelEntry, ModelReport, RelationEntry};

pub fn analyze(project: &LaravelProject) -> Result<ModelReport, String> {
    let mappings = collect_psr4_mappings(&project.root)?;
    let migration_report = migrations::analyze(project)?;

    let files = collect_model_files(&mappings);
    let mut models: Vec<ModelEntry> = files
        .iter()
        .filter_map(|f| parse_model_file(project, f, &mappings))
        .collect();

    for model in &mut models {
        model.columns =
            migrations::resolve_columns_for_table(&model.table, &migration_report.migrations);
    }

    models.sort_by(|a, b| a.class_name.cmp(&b.class_name));

    Ok(ModelReport {
        project_name: project.name.clone(),
        project_root: project.root.clone(),
        model_count: models.len(),
        models,
    })
}

/// Scan every PSR-4 base directory registered for this project (root + all packages).
/// This mirrors how the provider resolver finds any class system-wide, rather than
/// assuming models only live in app/Models/.
fn collect_model_files(mappings: &[Psr4Mapping]) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let mut seen_dirs = std::collections::BTreeSet::new();

    for mapping in mappings {
        let dir = &mapping.base_dir;
        if !dir.is_dir() {
            continue;
        }
        // Avoid scanning the same physical directory twice when multiple prefixes
        // point to the same base (e.g. App\ and App\Models\ both resolving under app/).
        let canonical = dir.canonicalize().unwrap_or_else(|_| dir.clone());
        if !seen_dirs.insert(canonical) {
            continue;
        }
        collect_php_files_recursive(dir, &mut files);
    }

    files.sort();
    files.dedup();
    files
}

fn collect_php_files_recursive(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_php_files_recursive(&path, out);
        } else if path.extension().and_then(|e| e.to_str()) == Some("php") {
            out.push(path);
        }
    }
}

fn parse_model_file(
    project: &LaravelProject,
    file: &Path,
    mappings: &[Psr4Mapping],
) -> Option<ModelEntry> {
    let source = fs::read(file).ok()?;
    let arena = Bump::new();
    let lexer = Lexer::new(&source);
    let mut parser = Parser::new(lexer, &arena);
    let program = parser.parse_program();

    let mut namespace = String::new();
    let mut imports: HashMap<String, String> = HashMap::new();

    for stmt in program.statements.iter() {
        match stmt {
            Stmt::Namespace { name, .. } => {
                if let Some(n) = name {
                    namespace = n
                        .parts
                        .iter()
                        .map(|t| span_text(t.span, &source))
                        .collect::<String>()
                        .trim_start_matches('\\')
                        .to_string();
                }
            }
            Stmt::Use { uses, kind, .. } => {
                if *kind != UseKind::Normal {
                    continue;
                }
                for item in *uses {
                    let fqn = item
                        .name
                        .parts
                        .iter()
                        .map(|t| span_text(t.span, &source))
                        .collect::<String>()
                        .trim_start_matches('\\')
                        .to_string();
                    let key = if let Some(alias) = item.alias {
                        span_text(alias.span, &source)
                    } else {
                        fqn.rsplit('\\').next().unwrap_or(&fqn).to_string()
                    };
                    imports.insert(key, fqn);
                }
            }
            Stmt::Class {
                name,
                extends,
                members,
                span,
                ..
            } => {
                let class_name = span_text(name.span, &source);

                if !looks_like_model(extends, &source, &imports) {
                    continue;
                }

                let (line, _) = byte_offset_to_line_col(&source, span.start);
                let relative = strip_root(&project.root, file);

                return Some(build_model_entry(
                    project,
                    &class_name,
                    &namespace,
                    members,
                    &source,
                    &imports,
                    mappings,
                    relative,
                    line,
                ));
            }
            _ => {}
        }
    }
    None
}

/// Decides whether a class is an Eloquent model by inspecting what it extends.
/// Resolves short names through the file's import map.
fn looks_like_model(
    extends: &Option<Name<'_>>,
    source: &[u8],
    imports: &HashMap<String, String>,
) -> bool {
    let Some(e) = extends else { return false };

    // NsSeparator tokens are included in parts, so collect directly (no extra join)
    let raw = e
        .parts
        .iter()
        .map(|t| span_text(t.span, source))
        .collect::<String>()
        .trim_start_matches('\\')
        .to_string();

    // Resolve short name through imports
    let resolved = if raw.contains('\\') {
        raw.clone()
    } else {
        imports.get(&raw).cloned().unwrap_or_else(|| raw.clone())
    };

    let short = resolved.rsplit('\\').next().unwrap_or(resolved.as_str());

    short.ends_with("Model")
        || short == "Authenticatable"
        || short == "Pivot"
        || short == "MorphPivot"
        || resolved == "Illuminate\\Database\\Eloquent\\Model"
        || resolved == "Illuminate\\Foundation\\Auth\\User"
}

fn build_model_entry(
    project: &LaravelProject,
    class_name: &str,
    namespace: &str,
    members: &[ClassMember<'_>],
    source: &[u8],
    imports: &HashMap<String, String>,
    mappings: &[Psr4Mapping],
    file: PathBuf,
    line: usize,
) -> ModelEntry {
    let mut table: Option<String> = None;
    let mut primary_key = "id".to_string();
    let mut key_type = "int".to_string();
    let mut incrementing = true;
    let mut timestamps = true;
    let mut connection: Option<String> = None;
    let mut fillable: Vec<String> = Vec::new();
    let mut guarded: Vec<String> = Vec::new();
    let mut hidden: Vec<String> = Vec::new();
    let mut casts: BTreeMap<String, String> = BTreeMap::new();
    let mut appends: Vec<String> = Vec::new();
    let mut with_eager: Vec<String> = Vec::new();
    let mut traits: Vec<String> = Vec::new();
    let mut relations: Vec<RelationEntry> = Vec::new();
    let mut scopes: Vec<String> = Vec::new();
    let mut accessors: Vec<String> = Vec::new();
    let mut mutators: Vec<String> = Vec::new();

    for member in members.iter() {
        match member {
            ClassMember::Property {
                entries, modifiers, ..
            } => {
                // Only look at non-static properties
                let is_static = modifiers
                    .iter()
                    .any(|t| span_text(t.span, source) == "static");
                if is_static {
                    continue;
                }

                for entry in entries.iter() {
                    let prop_name = span_text(entry.name.span, source)
                        .trim_start_matches('$')
                        .to_string();
                    let Some(default) = entry.default else {
                        continue;
                    };

                    match prop_name.as_str() {
                        "table" => {
                            table = expr_to_string(default, source);
                        }
                        "primaryKey" => {
                            primary_key =
                                expr_to_string(default, source).unwrap_or_else(|| "id".to_string());
                        }
                        "keyType" => {
                            key_type = expr_to_string(default, source)
                                .unwrap_or_else(|| "int".to_string());
                        }
                        "incrementing" => {
                            let raw = span_text(default.span(), source);
                            incrementing = raw.to_lowercase() != "false";
                        }
                        "timestamps" => {
                            let raw = span_text(default.span(), source);
                            timestamps = raw.to_lowercase() != "false";
                        }
                        "connection" => {
                            connection = expr_to_string(default, source);
                        }
                        "fillable" => {
                            fillable = expr_to_string_list(default, source);
                        }
                        "guarded" => {
                            guarded = expr_to_string_list(default, source);
                        }
                        "hidden" => {
                            hidden = expr_to_string_list(default, source);
                        }
                        "casts" => {
                            casts = extract_string_map(default, source);
                        }
                        "appends" => {
                            appends = expr_to_string_list(default, source);
                        }
                        "with" => {
                            with_eager = expr_to_string_list(default, source);
                        }
                        _ => {}
                    }
                }
            }
            ClassMember::TraitUse {
                traits: used_traits,
                ..
            } => {
                for t in used_traits.iter() {
                    let trait_name = t
                        .parts
                        .iter()
                        .map(|tok| span_text(tok.span, source))
                        .collect::<Vec<_>>()
                        .join("\\");
                    let short = trait_name
                        .rsplit('\\')
                        .next()
                        .unwrap_or(&trait_name)
                        .to_string();
                    traits.push(short);
                }
            }
            ClassMember::Method { name, body, .. } => {
                let method_name = span_text(name.span, source);

                // Scopes: scopeXxx → "xxx"
                if let Some(scope) = method_name.strip_prefix("scope") {
                    if scope
                        .chars()
                        .next()
                        .map(|c| c.is_uppercase())
                        .unwrap_or(false)
                    {
                        let snake = camel_to_snake(scope);
                        scopes.push(snake);
                        continue;
                    }
                }

                // Accessors (old style): getXxxAttribute → "xxx"
                if let Some(inner) = method_name
                    .strip_prefix("get")
                    .and_then(|s| s.strip_suffix("Attribute"))
                {
                    if inner
                        .chars()
                        .next()
                        .map(|c| c.is_uppercase())
                        .unwrap_or(false)
                    {
                        accessors.push(camel_to_snake(inner));
                        continue;
                    }
                }

                // Mutators (old style): setXxxAttribute → "xxx"
                if let Some(inner) = method_name
                    .strip_prefix("set")
                    .and_then(|s| s.strip_suffix("Attribute"))
                {
                    if inner
                        .chars()
                        .next()
                        .map(|c| c.is_uppercase())
                        .unwrap_or(false)
                    {
                        mutators.push(camel_to_snake(inner));
                        continue;
                    }
                }

                // Relations: detect by return expression
                if let Some(rel) =
                    extract_relation(&method_name, body, source, imports, mappings, &project.root)
                {
                    relations.push(rel);
                }
            }
            ClassMember::Const { consts, .. } => {
                for c in consts.iter() {
                    let const_name = span_text(c.name.span, source);
                    match const_name.as_str() {
                        "CREATED_AT" | "UPDATED_AT" => {} // just noting timestamps customization
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }

    // Check for soft deletes via trait
    let soft_deletes = traits.iter().any(|t| t == "SoftDeletes");

    let (table_name, table_inferred) = match table {
        Some(t) => (t, false),
        None => (infer_table_name(class_name), true),
    };

    ModelEntry {
        file,
        line,
        class_name: class_name.to_string(),
        namespace: namespace.to_string(),
        table: table_name,
        table_inferred,
        primary_key,
        key_type,
        incrementing,
        timestamps,
        soft_deletes,
        connection,
        fillable,
        guarded,
        hidden,
        casts,
        appends,
        with: with_eager,
        traits,
        relations,
        scopes,
        accessors,
        mutators,
        columns: Vec::new(), // populated after migration cross-ref
    }
}

fn extract_relation(
    method_name: &str,
    body: &[php_parser::ast::StmtId<'_>],
    source: &[u8],
    imports: &HashMap<String, String>,
    mappings: &[Psr4Mapping],
    project_root: &Path,
) -> Option<RelationEntry> {
    const RELATION_METHODS: &[&str] = &[
        "hasOne",
        "hasMany",
        "belongsTo",
        "belongsToMany",
        "hasManyThrough",
        "hasOneThrough",
        "morphTo",
        "morphOne",
        "morphMany",
        "morphToMany",
        "morphedByMany",
        "hasOneOfMany",
        "hasManyOfMany",
    ];

    // Walk the method body for a $this->relationMethod(...) or return $this->relationMethod(...)
    let mut found: Option<RelationEntry> = None;

    for stmt in body {
        match stmt {
            Stmt::Return {
                expr: Some(expr), ..
            }
            | Stmt::Expression { expr, .. } => {
                if let Some(rel) = try_extract_relation_call(
                    *expr,
                    source,
                    imports,
                    mappings,
                    project_root,
                    method_name,
                    RELATION_METHODS,
                ) {
                    found = Some(rel);
                    break;
                }
            }
            _ => {}
        }
    }
    found
}

fn try_extract_relation_call(
    expr: php_parser::ast::ExprId<'_>,
    source: &[u8],
    imports: &HashMap<String, String>,
    mappings: &[Psr4Mapping],
    project_root: &Path,
    method_name: &str,
    relation_methods: &[&str],
) -> Option<RelationEntry> {
    match expr {
        Expr::MethodCall { method, args, .. } => {
            let called = span_text(method.span(), source);

            if !relation_methods.contains(&called.as_str()) {
                // Could be chained like $this->hasMany(...)->withDefault() — peel one level
                if let Expr::MethodCall {
                    target,
                    method: inner_m,
                    args: inner_args,
                    ..
                } = expr
                {
                    let inner = span_text(inner_m.span(), source);
                    if relation_methods.contains(&inner.as_str()) {
                        return build_relation_entry(
                            method_name,
                            &inner,
                            inner_args,
                            source,
                            imports,
                            mappings,
                            project_root,
                        );
                    }
                    let _ = target;
                }
                return None;
            }

            build_relation_entry(
                method_name,
                &called,
                args,
                source,
                imports,
                mappings,
                project_root,
            )
        }
        _ => None,
    }
}

fn build_relation_entry(
    method_name: &str,
    relation_type: &str,
    args: &[php_parser::ast::Arg<'_>],
    source: &[u8],
    imports: &HashMap<String, String>,
    mappings: &[Psr4Mapping],
    project_root: &Path,
) -> Option<RelationEntry> {
    // First arg is related model class (ClassConstFetch or string)
    let related_raw = args
        .first()
        .and_then(|a| resolve_class_name(a.value, source, imports))?;

    let related_model_file = resolve_class_file(&related_raw, mappings)
        .map(|p| p.strip_prefix(project_root).unwrap_or(&p).to_path_buf());

    let foreign_key = args.get(1).and_then(|a| expr_to_string(a.value, source));
    let local_key = args.get(2).and_then(|a| expr_to_string(a.value, source));

    let pivot_table = if relation_type == "belongsToMany" {
        args.get(1).and_then(|a| expr_to_string(a.value, source))
    } else {
        None
    };

    let (foreign_key, local_key) = if relation_type == "belongsToMany" {
        (
            args.get(2).and_then(|a| expr_to_string(a.value, source)),
            args.get(3).and_then(|a| expr_to_string(a.value, source)),
        )
    } else {
        (foreign_key, local_key)
    };

    Some(RelationEntry {
        method: method_name.to_string(),
        relation_type: relation_type.to_string(),
        related_model: related_raw,
        related_model_file,
        foreign_key,
        local_key,
        pivot_table,
        line: 0,
    })
}

fn resolve_class_name(
    expr: php_parser::ast::ExprId<'_>,
    source: &[u8],
    imports: &HashMap<String, String>,
) -> Option<String> {
    match expr {
        Expr::ClassConstFetch {
            class, constant, ..
        } => {
            let const_name = span_text(constant.span(), source);
            if const_name.to_lowercase() != "class" {
                return None;
            }
            let raw = span_text(class.span(), source)
                .trim_start_matches('\\')
                .to_string();
            if raw.contains('\\') {
                Some(raw)
            } else {
                Some(imports.get(&raw).cloned().unwrap_or(raw))
            }
        }
        Expr::String { .. } => expr_to_string(expr, source),
        _ => None,
    }
}

fn extract_string_map(
    expr: php_parser::ast::ExprId<'_>,
    source: &[u8],
) -> BTreeMap<String, String> {
    let mut map = BTreeMap::new();
    let Expr::Array { items, .. } = expr else {
        return map;
    };
    for item in items.iter() {
        let Some(key) = item.key.and_then(|k| expr_to_string(k, source)) else {
            continue;
        };
        let val = span_text(item.value.span(), source)
            .trim_matches('\'')
            .trim_matches('"')
            .to_string();
        map.insert(key, val);
    }
    map
}

fn infer_table_name(class_name: &str) -> String {
    pluralize(&camel_to_snake(class_name))
}

fn camel_to_snake(s: &str) -> String {
    let mut result = String::new();
    for (i, ch) in s.chars().enumerate() {
        if ch.is_uppercase() && i > 0 {
            result.push('_');
        }
        for lc in ch.to_lowercase() {
            result.push(lc);
        }
    }
    result
}

fn pluralize(word: &str) -> String {
    if word.ends_with("ss")
        || word.ends_with("x")
        || word.ends_with("z")
        || word.ends_with("ch")
        || word.ends_with("sh")
    {
        format!("{word}es")
    } else if word.ends_with('y')
        && !matches!(word.len(), 0)
        && !matches!(
            word.as_bytes().get(word.len().wrapping_sub(2)),
            Some(b'a' | b'e' | b'i' | b'o' | b'u')
        )
    {
        format!("{}ies", &word[..word.len() - 1])
    } else if word.ends_with('s') {
        word.to_string()
    } else {
        format!("{word}s")
    }
}

#[allow(dead_code)]
fn _unused_col(_: ColumnEntry) {}
