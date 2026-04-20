use php_parser::Span;
use php_parser::ast::{BinaryOp, Expr, ExprId, MagicConstKind};
use std::path::{Component, Path, PathBuf};

pub fn span_text(span: Span, source: &[u8]) -> String {
    String::from_utf8_lossy(span.as_str(source)).into_owned()
}

pub fn byte_offset_to_line_col(source: &[u8], offset: usize) -> (usize, usize) {
    let before = &source[..offset.min(source.len())];
    let line = before.iter().filter(|&&b| b == b'\n').count() + 1;
    let col = before
        .iter()
        .rev()
        .position(|&b| b == b'\n')
        .unwrap_or(offset)
        + 1;
    (line, col)
}

pub fn parse_php_string_literal(value: &[u8]) -> String {
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

// Returns the string name for an expression. Falls back to the raw span text so
// that class identifiers like `Route` in static calls are always resolved.
pub fn expr_name(expr: ExprId<'_>, source: &[u8]) -> Option<String> {
    match expr {
        Expr::Variable { name, .. } => {
            Some(span_text(*name, source).trim_start_matches('$').to_string())
        }
        Expr::String { value, .. } => Some(String::from_utf8_lossy(value).into_owned()),
        _ => Some(span_text(expr.span(), source)),
    }
}

pub fn expr_to_string(expr: ExprId<'_>, source: &[u8]) -> Option<String> {
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

pub fn expr_to_string_list(expr: ExprId<'_>, source: &[u8]) -> Vec<String> {
    match expr {
        Expr::Array { items, .. } => items
            .iter()
            .filter_map(|item| expr_to_string(item.value, source))
            .collect(),
        _ => expr_to_string(expr, source).into_iter().collect(),
    }
}

// Resolves a PHP expression to an absolute filesystem path.
// Handles string literals, __DIR__, concatenation, and the common Laravel
// path helpers: base_path, app_path, config_path, resource_path.
pub fn expr_to_path(
    expr: ExprId<'_>,
    source: &[u8],
    project_root: &Path,
    provider_file: &Path,
) -> Option<PathBuf> {
    let raw = expr_to_path_fragment(expr, source, project_root, provider_file)?;
    let path = PathBuf::from(raw);
    let absolute = if path.is_absolute() {
        path
    } else {
        project_root.join(path)
    };
    Some(normalize_path(&absolute))
}

fn expr_to_path_fragment(
    expr: ExprId<'_>,
    source: &[u8],
    project_root: &Path,
    provider_file: &Path,
) -> Option<String> {
    match expr {
        Expr::String { .. } => expr_to_string(expr, source),
        Expr::MagicConst { kind, .. } => {
            if *kind == MagicConstKind::Dir {
                provider_file.parent().map(|p| p.display().to_string())
            } else {
                None
            }
        }
        Expr::Binary {
            left, op, right, ..
        } if *op == BinaryOp::Concat => {
            let left = expr_to_path_fragment(left, source, project_root, provider_file)?;
            let right = expr_to_path_fragment(right, source, project_root, provider_file)?;
            Some(format!("{left}{right}"))
        }
        Expr::Call { func, args, .. } => {
            let function_name = span_text(func.span(), source)
                .trim()
                .trim_start_matches('\\')
                .to_string();
            let arg = args.first().map(|arg| arg.value)?;
            let inner = expr_to_path_fragment(arg, source, project_root, provider_file)?;
            match function_name.as_str() {
                "base_path" => Some(project_root.join(inner).display().to_string()),
                "app_path" => Some(project_root.join("app").join(inner).display().to_string()),
                "config_path" => Some(
                    project_root
                        .join("config")
                        .join(inner)
                        .display()
                        .to_string(),
                ),
                "resource_path" => Some(
                    project_root
                        .join("resources")
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

pub fn normalize_path(path: &Path) -> PathBuf {
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

pub fn strip_root(root: &Path, path: &Path) -> PathBuf {
    path.strip_prefix(root).unwrap_or(path).to_path_buf()
}
