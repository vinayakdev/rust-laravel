use bumpalo::Bump;
use php_parser::ast::{BinaryOp, ClassMember, Expr, ExprId, MagicConstKind, Stmt, StmtId};
use php_parser::lexer::Lexer;
use php_parser::parser::Parser;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Component, Path, PathBuf};

use crate::analyzers::providers;
use crate::project::LaravelProject;
use crate::types::{ConfigItem, ConfigReport, ConfigSource};

pub fn analyze(project: &LaravelProject) -> Result<ConfigReport, String> {
    let env = load_env_map(project)?;
    let config_files = collect_registered_config_files(project)?;
    let mut items = Vec::new();

    for registered in config_files {
        let source = fs::read_to_string(&registered.file)
            .map_err(|error| format!("failed to read {}: {error}", registered.file.display()))?;

        items.extend(find_config_items(
            &project.root,
            &registered.file,
            &source,
            &registered.namespace,
            &env,
            &registered.source,
        ));
    }

    items.sort_by(|left, right| {
        left.file
            .cmp(&right.file)
            .then(left.line.cmp(&right.line))
            .then(left.column.cmp(&right.column))
            .then(left.key.cmp(&right.key))
            .then(left.source.declared_in.cmp(&right.source.declared_in))
    });

    Ok(ConfigReport {
        project_name: project.name.clone(),
        project_root: project.root.clone(),
        item_count: items.len(),
        items,
    })
}

#[derive(Clone)]
struct RegisteredConfigFile {
    file: PathBuf,
    namespace: String,
    source: ConfigSource,
}

#[derive(Clone)]
struct MergedConfigFile {
    file: PathBuf,
    namespace: String,
    source: ConfigSource,
}

fn collect_registered_config_files(
    project: &LaravelProject,
) -> Result<Vec<RegisteredConfigFile>, String> {
    let config_dir = project.root.join("config");
    let mut files = Vec::new();
    let mut seen = BTreeSet::new();

    let entries = fs::read_dir(&config_dir)
        .map_err(|error| format!("failed to read {}: {error}", config_dir.display()))?;

    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("php") {
            continue;
        }

        let namespace = path
            .file_stem()
            .and_then(|stem| stem.to_str())
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

    for merged in discover_provider_merged_configs(project)? {
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

    files.sort_by(|left, right| {
        left.file
            .cmp(&right.file)
            .then(left.namespace.cmp(&right.namespace))
            .then(left.source.declared_in.cmp(&right.source.declared_in))
            .then(left.source.line.cmp(&right.source.line))
    });

    Ok(files)
}

fn discover_provider_merged_configs(
    project: &LaravelProject,
) -> Result<Vec<MergedConfigFile>, String> {
    let provider_report = providers::analyze(project)?;
    let mut files = Vec::new();
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
        files.extend(extract_provider_merged_configs(
            project,
            &provider.provider_class,
            relative_source_file,
            &source_file,
            &source,
        ));
    }

    Ok(files)
}

fn extract_provider_merged_configs(
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
    for statement in program.statements {
        collect_provider_merged_configs_from_stmt(
            statement,
            source,
            project,
            provider_class,
            declared_in,
            provider_file,
            &mut files,
        );
    }
    files
}

fn collect_provider_merged_configs_from_stmt(
    statement: StmtId<'_>,
    source: &[u8],
    project: &LaravelProject,
    provider_class: &str,
    declared_in: &Path,
    provider_file: &Path,
    files: &mut Vec<MergedConfigFile>,
) {
    match statement {
        Stmt::Expression { expr, .. } => collect_provider_merged_configs_from_expr(
            expr,
            source,
            project,
            provider_class,
            declared_in,
            provider_file,
            files,
        ),
        Stmt::Block { statements, .. }
        | Stmt::Declare {
            body: statements, ..
        } => {
            for statement in *statements {
                collect_provider_merged_configs_from_stmt(
                    statement,
                    source,
                    project,
                    provider_class,
                    declared_in,
                    provider_file,
                    files,
                );
            }
        }
        Stmt::Namespace {
            body: Some(body), ..
        } => {
            for statement in *body {
                collect_provider_merged_configs_from_stmt(
                    statement,
                    source,
                    project,
                    provider_class,
                    declared_in,
                    provider_file,
                    files,
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
                        collect_provider_merged_configs_from_stmt(
                            statement,
                            source,
                            project,
                            provider_class,
                            declared_in,
                            provider_file,
                            files,
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
                collect_provider_merged_configs_from_stmt(
                    statement,
                    source,
                    project,
                    provider_class,
                    declared_in,
                    provider_file,
                    files,
                );
            }
            if let Some(else_block) = else_block {
                for statement in *else_block {
                    collect_provider_merged_configs_from_stmt(
                        statement,
                        source,
                        project,
                        provider_class,
                        declared_in,
                        provider_file,
                        files,
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
                collect_provider_merged_configs_from_stmt(
                    statement,
                    source,
                    project,
                    provider_class,
                    declared_in,
                    provider_file,
                    files,
                );
            }
        }
        _ => {}
    }
}

fn collect_provider_merged_configs_from_expr(
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

    let Some(path_expr) = args.first().map(|arg| arg.value) else {
        return;
    };
    let Some(namespace_expr) = args.get(1).map(|arg| arg.value) else {
        return;
    };

    let Some(config_file) = expr_to_path(path_expr, source, project, provider_file) else {
        return;
    };
    let Some(namespace) = expr_to_string(namespace_expr, source) else {
        return;
    };

    let line_info = path_expr.span().line_info(source);
    let line = line_info.map_or(1, |info| info.line);
    let column = line_info.map_or(1, |info| info.column);

    files.push(MergedConfigFile {
        file: config_file,
        namespace,
        source: ConfigSource {
            kind: "provider-mergeConfigFrom".to_string(),
            declared_in: declared_in.to_path_buf(),
            line,
            column,
            provider_class: Some(provider_class.to_string()),
        },
    });
}

fn load_env_map(project: &LaravelProject) -> Result<BTreeMap<String, String>, String> {
    for name in [".env", ".env.example"] {
        let path = project.root.join(name);
        if path.is_file() {
            let text = fs::read_to_string(&path)
                .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
            let raw = parse_env_pairs(&text);
            return Ok(resolve_env_pairs(&raw));
        }
    }

    Ok(BTreeMap::new())
}

fn parse_env_pairs(text: &str) -> BTreeMap<String, String> {
    let mut env = BTreeMap::new();

    for raw_line in text.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some((key, value)) = line.split_once('=') {
            env.insert(
                key.trim().to_string(),
                strip_quotes(value.trim()).to_string(),
            );
        }
    }

    env
}

fn resolve_env_pairs(raw: &BTreeMap<String, String>) -> BTreeMap<String, String> {
    let mut resolved = BTreeMap::new();
    for key in raw.keys() {
        let value = resolve_env_value(key, raw, &mut Vec::new());
        resolved.insert(key.clone(), value);
    }
    resolved
}

fn resolve_env_value(key: &str, raw: &BTreeMap<String, String>, stack: &mut Vec<String>) -> String {
    if stack.iter().any(|item| item == key) {
        return raw.get(key).cloned().unwrap_or_default();
    }

    let Some(value) = raw.get(key) else {
        return String::new();
    };

    stack.push(key.to_string());
    let expanded = expand_env_placeholders(value, raw, stack);
    stack.pop();
    expanded
}

fn expand_env_placeholders(
    value: &str,
    raw: &BTreeMap<String, String>,
    stack: &mut Vec<String>,
) -> String {
    let mut output = String::new();
    let chars: Vec<char> = value.chars().collect();
    let mut index = 0;

    while index < chars.len() {
        if chars[index] == '$' && chars.get(index + 1) == Some(&'{') {
            let mut end = index + 2;
            while end < chars.len() && chars[end] != '}' {
                end += 1;
            }
            if end < chars.len() {
                let key: String = chars[index + 2..end].iter().collect();
                output.push_str(&resolve_env_value(&key, raw, stack));
                index = end + 1;
                continue;
            }
        }

        output.push(chars[index]);
        index += 1;
    }

    output
}

fn find_config_items(
    root: &Path,
    file: &Path,
    source: &str,
    namespace: &str,
    env: &BTreeMap<String, String>,
    item_source: &ConfigSource,
) -> Vec<ConfigItem> {
    let mut stack: Vec<String> = Vec::new();
    let mut items = Vec::new();

    for (index, raw_line) in source.lines().enumerate() {
        let trimmed = raw_line.trim();
        if trimmed.is_empty() || trimmed.starts_with("//") || trimmed.starts_with('#') {
            continue;
        }

        let close_count = trimmed.chars().filter(|ch| *ch == ']').count();
        if let Some(parsed) = parse_config_assignment(raw_line) {
            let full_key = if stack.is_empty() {
                format!("{namespace}.{}", parsed.key)
            } else {
                format!("{namespace}.{}.{}", stack.join("."), parsed.key)
            };

            let env_value = parsed
                .env_key
                .as_ref()
                .and_then(|env_key| env.get(env_key).cloned());

            items.push(ConfigItem {
                file: strip_root(root, file),
                line: index + 1,
                column: parsed.column,
                key: full_key,
                env_key: parsed.env_key.clone(),
                default_value: parsed.default_value.clone(),
                env_value,
                source: item_source.clone(),
            });

            if parsed.opens_array {
                stack.push(parsed.key);
            }
        } else {
            pop_many(&mut stack, close_count);
        }
    }

    items
}

#[derive(Clone)]
struct ParsedConfigLine {
    key: String,
    column: usize,
    opens_array: bool,
    env_key: Option<String>,
    default_value: Option<String>,
}

fn parse_config_assignment(line: &str) -> Option<ParsedConfigLine> {
    let trimmed = line.trim_start();
    let leading = line.len() - trimmed.len();
    let quote = trimmed.chars().next()?;
    if quote != '\'' && quote != '"' {
        return None;
    }

    let mut key = String::new();
    let mut escaped = false;
    let mut rest_start = None;

    for (index, ch) in trimmed[1..].char_indices() {
        if escaped {
            key.push(ch);
            escaped = false;
            continue;
        }
        if ch == '\\' {
            escaped = true;
            continue;
        }
        if ch == quote {
            rest_start = Some(index + 2);
            break;
        }
        key.push(ch);
    }

    let rest = trimmed.get(rest_start?..)?.trim_start();
    if !rest.starts_with("=>") {
        return None;
    }

    let value = rest[2..].trim_start().trim_end_matches(',');
    let opens_array = value.starts_with('[');
    let (env_key, default_value) = parse_env_call(value)
        .map(|(env_key, default_value)| (Some(env_key), default_value))
        .unwrap_or_else(|| (None, parse_literal_value(value)));

    Some(ParsedConfigLine {
        key,
        column: leading + 1,
        opens_array,
        env_key,
        default_value,
    })
}

fn parse_env_call(value: &str) -> Option<(String, Option<String>)> {
    let env_index = value.find("env(")?;
    let args = extract_call_args(&value[env_index + 4..])?;
    let env_key = parse_quoted_string(args.first()?.trim())?;
    let default_value = args
        .get(1)
        .and_then(|part| parse_literal_value(part.trim()));
    Some((env_key, default_value))
}

fn extract_call_args(text: &str) -> Option<Vec<String>> {
    let mut depth = 1usize;
    let mut current = String::new();
    let mut args = Vec::new();
    let mut in_single = false;
    let mut in_double = false;
    let mut escape = false;

    for ch in text.chars() {
        if in_single {
            current.push(ch);
            if escape {
                escape = false;
            } else if ch == '\\' {
                escape = true;
            } else if ch == '\'' {
                in_single = false;
            }
            continue;
        }

        if in_double {
            current.push(ch);
            if escape {
                escape = false;
            } else if ch == '\\' {
                escape = true;
            } else if ch == '"' {
                in_double = false;
            }
            continue;
        }

        match ch {
            '\'' => {
                in_single = true;
                current.push(ch);
            }
            '"' => {
                in_double = true;
                current.push(ch);
            }
            '(' => {
                depth += 1;
                current.push(ch);
            }
            ')' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    args.push(current.trim().to_string());
                    return Some(args);
                }
                current.push(ch);
            }
            ',' if depth == 1 => {
                args.push(current.trim().to_string());
                current.clear();
            }
            _ => current.push(ch),
        }
    }

    None
}

fn parse_literal_value(value: &str) -> Option<String> {
    let value = value.trim();
    if value.is_empty() {
        return None;
    }

    if let Some(string) = parse_quoted_string(value) {
        return Some(string);
    }

    if matches!(value, "true" | "false" | "null") || value.parse::<i64>().is_ok() {
        return Some(value.to_string());
    }

    if value.starts_with('[') {
        return Some("[...]".to_string());
    }

    None
}

fn parse_quoted_string(value: &str) -> Option<String> {
    let mut chars = value.chars();
    let quote = chars.next()?;
    if quote != '\'' && quote != '"' {
        return None;
    }

    let mut output = String::new();
    let mut escaped = false;
    for ch in value[1..].chars() {
        if escaped {
            output.push(ch);
            escaped = false;
            continue;
        }
        if ch == '\\' {
            escaped = true;
            continue;
        }
        if ch == quote {
            return Some(output);
        }
        output.push(ch);
    }

    None
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
        _ => None,
    }
}

fn expr_to_path(
    expr: ExprId<'_>,
    source: &[u8],
    project: &LaravelProject,
    provider_file: &Path,
) -> Option<PathBuf> {
    let raw = expr_to_path_fragment(expr, source, project, provider_file)?;
    let path = PathBuf::from(raw);
    let absolute = if path.is_absolute() {
        path
    } else {
        project.root.join(path)
    };
    Some(normalize_path(&absolute))
}

fn expr_to_path_fragment(
    expr: ExprId<'_>,
    source: &[u8],
    project: &LaravelProject,
    provider_file: &Path,
) -> Option<String> {
    match expr {
        Expr::String { .. } => expr_to_string(expr, source),
        Expr::MagicConst { kind, .. } => {
            if *kind == MagicConstKind::Dir {
                provider_file
                    .parent()
                    .map(|path| path.display().to_string())
            } else {
                None
            }
        }
        Expr::Binary {
            left, op, right, ..
        } if *op == BinaryOp::Concat => {
            let left = expr_to_path_fragment(left, source, project, provider_file)?;
            let right = expr_to_path_fragment(right, source, project, provider_file)?;
            Some(format!("{left}{right}"))
        }
        Expr::Call { func, args, .. } => {
            let function_name = span_text(func.span(), source)
                .trim()
                .trim_start_matches('\\')
                .to_string();
            let arg = args.first().map(|arg| arg.value)?;
            let inner = expr_to_path_fragment(arg, source, project, provider_file)?;
            match function_name.as_str() {
                "base_path" => Some(project.root.join(inner).display().to_string()),
                "app_path" => Some(project.root.join("app").join(inner).display().to_string()),
                "config_path" => Some(
                    project
                        .root
                        .join("config")
                        .join(inner)
                        .display()
                        .to_string(),
                ),
                _ => None,
            }
        }
        _ => None,
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

fn normalize_path(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            other => normalized.push(other.as_os_str()),
        }
    }
    normalized
}

fn strip_quotes(value: &str) -> &str {
    value
        .strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
        .or_else(|| {
            value
                .strip_prefix('\'')
                .and_then(|value| value.strip_suffix('\''))
        })
        .unwrap_or(value)
}

fn pop_many(stack: &mut Vec<String>, count: usize) {
    for _ in 0..count {
        if stack.pop().is_none() {
            break;
        }
    }
}

fn strip_root(root: &Path, path: &Path) -> PathBuf {
    path.strip_prefix(root).unwrap_or(path).to_path_buf()
}
