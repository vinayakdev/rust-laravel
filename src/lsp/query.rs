use serde_json::{Value, json};

use super::context::{SymbolContext, SymbolKind};
use super::index::ProjectIndex;
use crate::types::{ConfigItem, RouteEntry};

pub fn complete(index: &ProjectIndex, context: &SymbolContext, line: usize) -> Vec<Value> {
    match context.kind {
        SymbolKind::Config => index
            .config_matches(&context.prefix)
            .into_iter()
            .map(|item| config_completion(item, context, line))
            .collect(),
        SymbolKind::Route => index
            .route_matches(&context.prefix)
            .into_iter()
            .map(|route| route_completion(route, context, line))
            .collect(),
    }
}

pub fn definitions(index: &ProjectIndex, context: &SymbolContext) -> Vec<Value> {
    match context.kind {
        SymbolKind::Config => index
            .config_definitions(&context.full_text)
            .into_iter()
            .map(|item| location(&index.project_root, &item.file, item.line, item.column))
            .collect(),
        SymbolKind::Route => index
            .route_definitions(&context.full_text)
            .into_iter()
            .map(|route| location(&index.project_root, &route.file, route.line, route.column))
            .collect(),
    }
}

pub fn hover(index: &ProjectIndex, context: &SymbolContext) -> Option<Value> {
    match context.kind {
        SymbolKind::Config => {
            let item = index
                .config_definitions(&context.full_text)
                .into_iter()
                .next()?;
            Some(json!({
                "contents": {
                    "kind": "markdown",
                    "value": config_hover(item),
                }
            }))
        }
        SymbolKind::Route => {
            let route = index
                .route_definitions(&context.full_text)
                .into_iter()
                .next()?;
            Some(json!({
                "contents": {
                    "kind": "markdown",
                    "value": route_hover(route),
                }
            }))
        }
    }
}

fn config_completion(item: &ConfigItem, context: &SymbolContext, line: usize) -> Value {
    let current_value = config_current_value(item);
    let summary = current_value.map(minify_completion_value);

    json!({
        "label": item.key,
        "kind": 10,
        "detail": summary,
        "documentation": {
            "kind": "markdown",
            "value": config_hover(item),
        },
        "textEdit": replacement_edit(context, line, &item.key),
    })
}

fn route_completion(route: &RouteEntry, context: &SymbolContext, line: usize) -> Value {
    let name = route.name.as_deref().unwrap_or_default();
    let detail = format!("{} {}", route.methods.join("|"), route.uri);

    json!({
        "label": name,
        "kind": 18,
        "detail": detail,
        "documentation": {
            "kind": "markdown",
            "value": route_hover(route),
        },
        "textEdit": replacement_edit(context, line, name),
    })
}

fn replacement_edit(context: &SymbolContext, line: usize, new_text: &str) -> Value {
    json!({
        "range": {
            "start": { "line": line, "character": context.start_character },
            "end": { "line": line, "character": context.end_character },
        },
        "newText": new_text,
    })
}

fn config_hover(item: &ConfigItem) -> String {
    let mut lines = vec![format!("`{}`", item.key)];

    if let Some(current_value) = config_current_value(item) {
        lines.push(format!("- current value: `{current_value}`"));
    }
    if let Some(env_key) = &item.env_key {
        lines.push(format!("- env key: `{env_key}`"));
    }
    if let Some(default_value) = &item.default_value {
        lines.push(format!("- default: `{default_value}`"));
    }
    if let Some(env_value) = &item.env_value {
        lines.push(format!("- resolved env: `{env_value}`"));
    }

    lines.join("\n")
}

fn config_current_value(item: &ConfigItem) -> Option<&str> {
    item.env_value.as_deref().or(item.default_value.as_deref())
}

fn minify_completion_value(value: &str) -> String {
    let compact = value.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut chars = compact.chars();
    let shortened: String = chars.by_ref().take(28).collect();
    if chars.next().is_some() {
        format!("{shortened}..")
    } else {
        shortened
    }
}

fn route_hover(route: &RouteEntry) -> String {
    let mut lines = vec![
        format!("`{}`", route.name.as_deref().unwrap_or("<unnamed-route>")),
        format!("- methods: `{}`", route.methods.join(", ")),
        format!("- uri: `{}`", route.uri),
    ];

    if let Some(action) = &route.action {
        lines.push(format!("- action: `{action}`"));
    }
    if !route.resolved_middleware.is_empty() {
        lines.push(format!(
            "- middleware: `{}`",
            route.resolved_middleware.join(", ")
        ));
    }
    if !route.parameter_patterns.is_empty() {
        let patterns = route
            .parameter_patterns
            .iter()
            .map(|(name, pattern)| format!("{name}={pattern}"))
            .collect::<Vec<_>>()
            .join(", ");
        lines.push(format!("- parameter patterns: `{patterns}`"));
    }
    lines.push(format!(
        "- source: `{}`:{}:{}",
        route.file.display(),
        route.line,
        route.column
    ));
    lines.push(format!(
        "- registration: `{}` from `{}`",
        route.registration.kind,
        route.registration.declared_in.display()
    ));

    lines.join("\n")
}

fn location(
    project_root: &std::path::Path,
    relative_file: &std::path::Path,
    line: usize,
    column: usize,
) -> Value {
    let absolute = project_root.join(relative_file);
    json!({
        "uri": path_to_file_uri(&absolute),
        "range": {
            "start": { "line": line.saturating_sub(1), "character": column.saturating_sub(1) },
            "end": { "line": line.saturating_sub(1), "character": column.saturating_sub(1) },
        }
    })
}

fn path_to_file_uri(path: &std::path::Path) -> String {
    let raw = path.to_string_lossy();
    let encoded = raw
        .chars()
        .flat_map(|ch| match ch {
            ' ' => "%20".chars().collect::<Vec<_>>(),
            '#' => "%23".chars().collect::<Vec<_>>(),
            '?' => "%3F".chars().collect::<Vec<_>>(),
            '%' => "%25".chars().collect::<Vec<_>>(),
            _ => vec![ch],
        })
        .collect::<String>();

    format!("file://{encoded}")
}
