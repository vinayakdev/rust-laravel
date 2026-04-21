use serde_json::{Value, json};
use std::path::Path;

use super::context::{
    HelperContext, HelperStyle, RouteActionContext, RouteActionKind, SymbolContext, SymbolKind,
};
use super::index::ProjectIndex;
use crate::types::{ConfigItem, ControllerEntry, ControllerMethodEntry, EnvItem, RouteEntry, ViewEntry};

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
        SymbolKind::Env => index
            .env_matches(&context.prefix)
            .into_iter()
            .map(|item| env_completion(item, context, line))
            .collect(),
        SymbolKind::View => index
            .view_matches(&context.prefix)
            .into_iter()
            .map(|view| view_completion(view, context, line))
            .collect(),
    }
}

pub fn helper_snippets(context: &HelperContext, line: usize) -> Vec<Value> {
    helper_specs()
        .iter()
        .filter(|helper| helper.name.starts_with(&context.prefix))
        .map(|helper| helper_completion(helper, context, line))
        .collect()
}

pub fn complete_route_actions(
    index: &ProjectIndex,
    context: &RouteActionContext,
    line: usize,
) -> Vec<Value> {
    match context.kind {
        RouteActionKind::ControllerClass | RouteActionKind::LegacyControllerString => index
            .controller_matches(&context.prefix)
            .into_iter()
            .map(|controller| controller_completion(controller, context, line))
            .collect(),
        RouteActionKind::ControllerMethodArray | RouteActionKind::LegacyMethodString => context
            .controller
            .as_deref()
            .into_iter()
            .flat_map(|controller| index.controller_methods(controller, &context.prefix))
            .map(|(controller, method)| {
                controller_method_completion(controller, method, context, line)
            })
            .collect(),
    }
}

pub fn definitions(index: &ProjectIndex, context: &SymbolContext, line: usize) -> Vec<Value> {
    match context.kind {
        SymbolKind::Config => index
            .config_definitions(&context.full_text)
            .into_iter()
            .map(|item| {
                location_link(
                    &index.project_root,
                    &item.file,
                    item.line,
                    item.column,
                    line,
                    context.start_character,
                    context.end_character,
                )
            })
            .collect(),
        SymbolKind::Route => index
            .route_definitions(&context.full_text)
            .into_iter()
            .map(|route| {
                location_link(
                    &index.project_root,
                    &route.file,
                    route.line,
                    route.column,
                    line,
                    context.start_character,
                    context.end_character,
                )
            })
            .collect(),
        SymbolKind::Env => index
            .env_definitions(&context.full_text)
            .into_iter()
            .map(|item| {
                location_link(
                    &index.project_root,
                    &item.file,
                    item.line,
                    item.column,
                    line,
                    context.start_character,
                    context.end_character,
                )
            })
            .collect(),
        SymbolKind::View => index
            .view_definitions(&context.full_text)
            .into_iter()
            .map(|view| {
                location_link(
                    &index.project_root,
                    &view.file,
                    1,
                    1,
                    line,
                    context.start_character,
                    context.end_character,
                )
            })
            .collect(),
    }
}

pub fn route_action_definitions(
    index: &ProjectIndex,
    context: &RouteActionContext,
    line: usize,
) -> Vec<Value> {
    match context.kind {
        RouteActionKind::ControllerClass | RouteActionKind::LegacyControllerString => {
            let controller = context.full_text.as_str();

            index
                .controller_definitions(controller)
                .into_iter()
                .map(|entry| {
                    location_link(
                        &index.project_root,
                        &entry.file,
                        entry.line,
                        1,
                        line,
                        context.start_character,
                        context.end_character,
                    )
                })
                .collect()
        }
        RouteActionKind::ControllerMethodArray | RouteActionKind::LegacyMethodString => {
            let Some(controller) = context.controller.as_deref() else {
                return Vec::new();
            };

            index
                .controller_method_definitions(controller, &context.full_text)
                .into_iter()
                .map(|(_, method)| {
                    location_link(
                        &index.project_root,
                        &method.declared_in,
                        method.line,
                        1,
                        line,
                        context.start_character,
                        context.end_character,
                    )
                })
                .collect()
        }
    }
}

pub fn hover(index: &ProjectIndex, context: &SymbolContext, line: usize) -> Option<Value> {
    let range = hover_range(line, context.start_character, context.end_character);
    match context.kind {
        SymbolKind::Config => {
            let item = index
                .config_definitions(&context.full_text)
                .into_iter()
                .next()?;
            Some(json!({
                "contents": { "kind": "markdown", "value": config_hover(item) },
                "range": range,
            }))
        }
        SymbolKind::Route => {
            let route = index
                .route_definitions(&context.full_text)
                .into_iter()
                .next()?;
            Some(json!({
                "contents": { "kind": "markdown", "value": route_hover(route) },
                "range": range,
            }))
        }
        SymbolKind::Env => {
            let item = index
                .env_definitions(&context.full_text)
                .into_iter()
                .next()?;
            Some(json!({
                "contents": { "kind": "markdown", "value": env_hover(item) },
                "range": range,
            }))
        }
        SymbolKind::View => {
            let view = index
                .view_definitions(&context.full_text)
                .into_iter()
                .next()?;
            Some(json!({
                "contents": { "kind": "markdown", "value": view_hover(view) },
                "range": range,
            }))
        }
    }
}

pub fn route_action_hover(
    index: &ProjectIndex,
    context: &RouteActionContext,
    line: usize,
) -> Option<Value> {
    let range = hover_range(line, context.start_character, context.end_character);
    match context.kind {
        RouteActionKind::ControllerClass | RouteActionKind::LegacyControllerString => {
            let controller = context.full_text.as_str();
            let item = index
                .controller_definitions(controller)
                .into_iter()
                .next()?;
            Some(json!({
                "contents": { "kind": "markdown", "value": controller_hover(item) },
                "range": range,
            }))
        }
        RouteActionKind::ControllerMethodArray | RouteActionKind::LegacyMethodString => {
            let controller = context.controller.as_deref()?;
            let (owner, method) = index
                .controller_method_definitions(controller, &context.full_text)
                .into_iter()
                .next()?;
            Some(json!({
                "contents": { "kind": "markdown", "value": controller_method_hover(owner, method) },
                "range": range,
            }))
        }
    }
}

pub fn route_diagnostics(index: &ProjectIndex, relative_file: &Path, source: &str) -> Vec<Value> {
    index
        .routes_for_file(relative_file)
        .into_iter()
        .filter_map(|route| {
            let target = route.controller_target.as_ref()?;
            if target.status == "ok" {
                return None;
            }

            let severity = match target.status.as_str() {
                "missing-method" | "missing-controller" | "ambiguous-controller" => 1,
                _ => 2,
            };
            let line_index = route.line.saturating_sub(1);
            let end_character = source
                .lines()
                .nth(line_index)
                .map(|line| line.chars().count())
                .unwrap_or(1)
                .max(1);

            Some(json!({
                "range": {
                    "start": { "line": line_index, "character": 0 },
                    "end": { "line": line_index, "character": end_character },
                },
                "severity": severity,
                "source": "rust-php",
                "code": diagnostic_code(target.status.as_str()),
                "message": diagnostic_message(route),
                "data": {
                    "controller": target.controller,
                    "method": target.method,
                    "status": target.status,
                }
            }))
        })
        .collect()
}

pub fn route_action_code_actions(index: &ProjectIndex, diagnostics: &[Value]) -> Vec<Value> {
    diagnostics
        .iter()
        .filter_map(|diagnostic| {
            let code = diagnostic.get("code").and_then(Value::as_str)?;
            if code != "rust-php/missing-controller-method" {
                return None;
            }
            let controller = diagnostic
                .pointer("/data/controller")
                .and_then(Value::as_str)?;
            let method = diagnostic.pointer("/data/method").and_then(Value::as_str)?;
            create_missing_method_action(index, controller, method, diagnostic)
        })
        .collect()
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

fn env_completion(item: &EnvItem, context: &SymbolContext, line: usize) -> Value {
    json!({
        "label": item.key,
        "kind": 21,
        "detail": minify_completion_value(&item.value),
        "documentation": {
            "kind": "markdown",
            "value": env_hover(item),
        },
        "textEdit": replacement_edit(context, line, &item.key),
    })
}

fn helper_completion(helper: &HelperSpec, context: &HelperContext, line: usize) -> Value {
    let mut item = json!({
        "label": helper.name,
        "kind": 3,
        "detail": helper.detail,
        "insertTextFormat": 2,
        "filterText": helper.name,
        "documentation": {
            "kind": "markdown",
            "value": helper.documentation,
        },
        "textEdit": {
            "range": {
                "start": { "line": line, "character": context.start_character },
                "end": { "line": line, "character": context.end_character },
            },
            "newText": helper.render(context.style),
        }
    });

    if helper.retrigger_completion {
        item["command"] = json!({
            "title": "Trigger completion",
            "command": "editor.action.triggerSuggest",
        });
    }

    item
}

fn controller_completion(
    controller: &ControllerEntry,
    context: &RouteActionContext,
    line: usize,
) -> Value {
    let insert = controller.class_name.as_str();
    json!({
        "label": controller.class_name,
        "kind": 7,
        "detail": controller.fqn,
        "documentation": {
            "kind": "markdown",
            "value": controller_hover(controller),
        },
        "textEdit": {
            "range": {
                "start": { "line": line, "character": context.start_character },
                "end": { "line": line, "character": context.end_character },
            },
            "newText": insert,
        }
    })
}

fn controller_method_completion(
    controller: &ControllerEntry,
    method: &ControllerMethodEntry,
    context: &RouteActionContext,
    line: usize,
) -> Value {
    let new_text = match context.kind {
        RouteActionKind::LegacyMethodString => method.name.clone(),
        _ => method.name.clone(),
    };

    json!({
        "label": method.name,
        "kind": 2,
        "detail": format!("{} {}", controller.class_name, method.accessibility),
        "documentation": {
            "kind": "markdown",
            "value": controller_method_hover(controller, method),
        },
        "textEdit": {
            "range": {
                "start": { "line": line, "character": context.start_character },
                "end": { "line": line, "character": context.end_character },
            },
            "newText": new_text,
        }
    })
}

fn hover_range(line: usize, start_character: usize, end_character: usize) -> Value {
    json!({
        "start": { "line": line, "character": start_character },
        "end": { "line": line, "character": end_character },
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

fn view_completion(view: &ViewEntry, context: &SymbolContext, line: usize) -> Value {
    json!({
        "label": view.name,
        "kind": 17,
        "detail": view.file.display().to_string(),
        "documentation": {
            "kind": "markdown",
            "value": view_hover(view),
        },
        "textEdit": replacement_edit(context, line, &view.name),
    })
}

fn view_hover(view: &ViewEntry) -> String {
    let mut lines = vec![
        format!("`{}`", view.name),
        format!("- kind: `{}`", view.kind),
        format!("- file: `{}`", view.file.display()),
    ];

    if !view.usages.is_empty() {
        lines.push(format!("- usages: `{}`", view.usages.len()));
    }
    if !view.props.is_empty() {
        let prop_names = view
            .props
            .iter()
            .map(|p| p.name.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        lines.push(format!("- props: `{prop_names}`"));
    }

    lines.join("\n")
}

fn env_hover(item: &EnvItem) -> String {
    [
        format!("`{}`", item.key),
        format!("- value: `{}`", item.value),
        format!(
            "- source: `{}`:{}:{}",
            item.file.display(),
            item.line,
            item.column
        ),
    ]
    .join("\n")
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

fn controller_hover(controller: &ControllerEntry) -> String {
    let mut lines = vec![
        format!("`{}`", controller.fqn),
        format!("- callable methods: `{}`", controller.callable_method_count),
        format!("- total methods: `{}`", controller.method_count),
        format!(
            "- source: `{}`:{}",
            controller.file.display(),
            controller.line
        ),
    ];

    if let Some(parent) = &controller.extends {
        lines.push(format!("- extends: `{parent}`"));
    }
    if !controller.traits.is_empty() {
        lines.push(format!("- traits: `{}`", controller.traits.join(", ")));
    }

    lines.join("\n")
}

fn controller_method_hover(controller: &ControllerEntry, method: &ControllerMethodEntry) -> String {
    [
        format!("`{}::{}`", controller.class_name, method.name),
        format!("- controller: `{}`", controller.fqn),
        format!("- route callable: `{}`", method.accessible_from_route),
        format!("- visibility: `{}`", method.visibility),
        format!("- source kind: `{}`", method.source_kind),
        format!("- notes: `{}`", method.accessibility),
        format!(
            "- source: `{}`:{}",
            method.declared_in.display(),
            method.line
        ),
    ]
    .join("\n")
}

fn diagnostic_code(status: &str) -> &'static str {
    match status {
        "missing-method" => "rust-php/missing-controller-method",
        "missing-controller" => "rust-php/missing-controller",
        "ambiguous-controller" => "rust-php/ambiguous-controller",
        "not-route-callable" => "rust-php/not-route-callable",
        _ => "rust-php/controller-route-problem",
    }
}

fn diagnostic_message(route: &RouteEntry) -> String {
    let Some(target) = route.controller_target.as_ref() else {
        return "Route action could not be resolved.".to_string();
    };

    match target.status.as_str() {
        "missing-method" => format!(
            "Route action `{}` is missing method `{}` on `{}`.",
            route.action.as_deref().unwrap_or_default(),
            target.method,
            target.controller
        ),
        "missing-controller" => format!(
            "Route action `{}` references controller `{}` that could not be found.",
            route.action.as_deref().unwrap_or_default(),
            target.controller
        ),
        "ambiguous-controller" => format!(
            "Route action `{}` matches multiple controllers named `{}`.",
            route.action.as_deref().unwrap_or_default(),
            target.controller
        ),
        "not-route-callable" => format!(
            "Route action `{}` is not route-callable: {}",
            route.action.as_deref().unwrap_or_default(),
            target.notes.join("; ")
        ),
        _ => route
            .action
            .clone()
            .unwrap_or_else(|| "Route action issue".to_string()),
    }
}

fn create_missing_method_action(
    index: &ProjectIndex,
    controller: &str,
    method: &str,
    diagnostic: &Value,
) -> Option<Value> {
    if !is_valid_php_method_name(method) {
        return None;
    }

    let entry = index
        .controller_definitions(controller)
        .into_iter()
        .next()?;
    let uri = path_to_file_uri(&index.project_root.join(&entry.file));
    let insert_line = entry.class_end_line.saturating_sub(1);
    let new_text = format!(
        "\n    public function {method}()\n    {{\n        // TODO: implement {method}.\n    }}\n"
    );
    let mut changes = serde_json::Map::new();
    changes.insert(
        uri,
        Value::Array(vec![json!({
            "range": {
                "start": { "line": insert_line, "character": 0 },
                "end": { "line": insert_line, "character": 0 }
            },
            "newText": new_text
        })]),
    );

    Some(json!({
        "title": format!("Create controller method `{method}` in {}", entry.class_name),
        "kind": "quickfix",
        "diagnostics": [diagnostic.clone()],
        "edit": {
            "changes": changes
        },
        "isPreferred": true
    }))
}

fn is_valid_php_method_name(method: &str) -> bool {
    let mut chars = method.chars();
    let Some(first) = chars.next() else {
        return false;
    };

    if !(first.is_ascii_alphabetic() || first == '_') {
        return false;
    }

    chars.all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
}

struct HelperSpec {
    name: &'static str,
    detail: &'static str,
    documentation: &'static str,
    body: &'static str,
    retrigger_completion: bool,
}

impl HelperSpec {
    fn render(&self, style: HelperStyle) -> String {
        match style {
            HelperStyle::Php => format!("{};", self.body),
            HelperStyle::BladeEcho => self.body.to_string(),
        }
    }
}

fn helper_specs() -> &'static [HelperSpec] {
    &[
        HelperSpec {
            name: "app",
            detail: "Laravel helper snippet",
            documentation: "Insert `app($1)`.",
            body: "app(${1:})",
            retrigger_completion: false,
        },
        HelperSpec {
            name: "asset",
            detail: "Laravel helper snippet",
            documentation: "Insert `asset('$1')`.",
            body: "asset('${1:path}')",
            retrigger_completion: true,
        },
        HelperSpec {
            name: "auth",
            detail: "Laravel helper snippet",
            documentation: "Insert `auth()`.",
            body: "auth()",
            retrigger_completion: false,
        },
        HelperSpec {
            name: "back",
            detail: "Laravel helper snippet",
            documentation: "Insert `back()`.",
            body: "back()",
            retrigger_completion: false,
        },
        HelperSpec {
            name: "cache",
            detail: "Laravel helper snippet",
            documentation: "Insert `cache('$1')`.",
            body: "cache('${1:key}')",
            retrigger_completion: true,
        },
        HelperSpec {
            name: "config",
            detail: "Laravel helper snippet",
            documentation: "Insert `config('$1')`.",
            body: "config('${1:key}')",
            retrigger_completion: true,
        },
        HelperSpec {
            name: "env",
            detail: "Laravel helper snippet",
            documentation: "Insert `env('$1')`.",
            body: "env('${1:key}')",
            retrigger_completion: true,
        },
        HelperSpec {
            name: "old",
            detail: "Laravel helper snippet",
            documentation: "Insert `old('$1')`.",
            body: "old('${1:key}')",
            retrigger_completion: true,
        },
        HelperSpec {
            name: "redirect",
            detail: "Laravel helper snippet",
            documentation: "Insert `redirect('${1:path}')`.",
            body: "redirect('${1:path}')",
            retrigger_completion: true,
        },
        HelperSpec {
            name: "request",
            detail: "Laravel helper snippet",
            documentation: "Insert `request()`.",
            body: "request()",
            retrigger_completion: false,
        },
        HelperSpec {
            name: "response",
            detail: "Laravel helper snippet",
            documentation: "Insert `response()`.",
            body: "response()",
            retrigger_completion: false,
        },
        HelperSpec {
            name: "route",
            detail: "Laravel helper snippet",
            documentation: "Insert `route('$1')`.",
            body: "route('${1:name}')",
            retrigger_completion: true,
        },
        HelperSpec {
            name: "session",
            detail: "Laravel helper snippet",
            documentation: "Insert `session('$1')`.",
            body: "session('${1:key}')",
            retrigger_completion: true,
        },
        HelperSpec {
            name: "to_route",
            detail: "Laravel helper snippet",
            documentation: "Insert `to_route('$1')`.",
            body: "to_route('${1:name}')",
            retrigger_completion: true,
        },
        HelperSpec {
            name: "url",
            detail: "Laravel helper snippet",
            documentation: "Insert `url('$1')`.",
            body: "url('${1:path}')",
            retrigger_completion: true,
        },
        HelperSpec {
            name: "view",
            detail: "Laravel helper snippet",
            documentation: "Insert `view('$1')`.",
            body: "view('${1:name}')",
            retrigger_completion: true,
        },
        HelperSpec {
            name: "__",
            detail: "Laravel helper snippet",
            documentation: "Insert `__('$1')`.",
            body: "__('${1:key}')",
            retrigger_completion: true,
        },
    ]
}

fn location_link(
    project_root: &std::path::Path,
    relative_file: &std::path::Path,
    target_line: usize,
    target_column: usize,
    line: usize,
    start_character: usize,
    end_character: usize,
) -> Value {
    let absolute = project_root.join(relative_file);
    json!({
        "originSelectionRange": {
            "start": { "line": line, "character": start_character },
            "end": { "line": line, "character": end_character },
        },
        "targetUri": path_to_file_uri(&absolute),
        "targetRange": {
            "start": { "line": target_line.saturating_sub(1), "character": target_column.saturating_sub(1) },
            "end": { "line": target_line.saturating_sub(1), "character": target_column.saturating_sub(1) },
        },
        "targetSelectionRange": {
            "start": { "line": target_line.saturating_sub(1), "character": target_column.saturating_sub(1) },
            "end": { "line": target_line.saturating_sub(1), "character": target_column.saturating_sub(1) },
        },
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

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    use serde_json::json;

    use crate::lsp::context::{detect_route_action_context, detect_symbol_context};
    use crate::lsp::index::ProjectIndex;
    use crate::lsp::overrides::FileOverrides;
    use crate::project;

    use super::{
        complete_route_actions, definitions, hover, route_action_code_actions, route_action_definitions,
        route_diagnostics,
    };

    fn sandbox_project() -> project::LaravelProject {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("laravel-example")
            .join("sandbox-app");
        project::from_root(root).expect("sandbox project should resolve")
    }

    fn unique_temp_project_root() -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time should move forward")
            .as_nanos();
        std::env::temp_dir().join(format!("rust-php-lsp-query-{nonce}"))
    }

    fn write_file(root: &Path, relative: &str, contents: &str) {
        let path = root.join(relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("parent directory should exist");
        }
        fs::write(path, contents).expect("fixture file should be written");
    }

    fn view_project() -> project::LaravelProject {
        let root = unique_temp_project_root();
        fs::create_dir_all(&root).expect("fixture root should exist");

        write_file(
            &root,
            "composer.json",
            r#"{
  "autoload": {
    "psr-4": {
      "App\\": "app/"
    }
  }
}"#,
        );
        write_file(&root, "config/app.php", "<?php\n\nreturn [];\n");
        write_file(&root, "routes/web.php", "<?php\n");
        write_file(
            &root,
            "app/Http/Controllers/ViewController.php",
            r#"<?php

namespace App\Http\Controllers;

class ViewController
{
    public function __invoke()
    {
        return view('admin.users.index');
    }
}
"#,
        );
        write_file(
            &root,
            "resources/views/admin/users/index.blade.php",
            "<div>{{ $title }}</div>\n",
        );

        project::from_root(root).expect("fixture project should resolve")
    }

    fn symbol_project() -> project::LaravelProject {
        let root = unique_temp_project_root();
        fs::create_dir_all(&root).expect("fixture root should exist");

        write_file(
            &root,
            "composer.json",
            r#"{
  "autoload": {
    "psr-4": {
      "App\\": "app/"
    }
  }
}"#,
        );
        write_file(
            &root,
            ".env",
            "APP_NAME=Fixture App\nAPP_ENV=local\nAPP_DEBUG=true\n",
        );
        write_file(&root, "config/app.php", "<?php\n\nreturn [];\n");
        write_file(
            &root,
            "config/debug.php",
            r#"<?php

return [
    'enabled' => env('APP_DEBUG', false),
];
"#,
        );
        write_file(
            &root,
            "routes/web.php",
            r#"<?php

use Illuminate\Support\Facades\Route;

Route::get('/dashboard', function () {
    return 'ok';
})->name('dashboard.home');
"#,
        );

        project::from_root(root).expect("fixture project should resolve")
    }

    #[test]
    fn completes_only_route_callable_controller_methods() {
        let project = sandbox_project();
        let index = ProjectIndex::build_with_overrides(&project, &FileOverrides::default())
            .expect("index should build");
        let source = "Route::get('/', [WebsiteController::class, '']);";
        let character = source.find("''").unwrap() + 1;
        let context =
            detect_route_action_context("file:///tmp/routes/web.php", source, 0, character)
                .expect("route action context");

        let items = complete_route_actions(&index, &context, 0);
        let labels = items
            .iter()
            .filter_map(|item| item.get("label").and_then(|value| value.as_str()))
            .collect::<Vec<_>>();

        assert!(labels.contains(&"home"));
        assert!(labels.contains(&"team"));
        assert!(labels.contains(&"publish"));
        assert!(!labels.contains(&"sustainability"));
        assert!(!labels.contains(&"docs"));
    }

    #[test]
    fn resolves_legacy_controller_method_definition() {
        let project = sandbox_project();
        let index = ProjectIndex::build_with_overrides(&project, &FileOverrides::default())
            .expect("index should build");
        let source = "Route::get('/', 'WebsiteController@home');";
        let character = source.find("home").unwrap() + 3;
        let context =
            detect_route_action_context("file:///tmp/routes/web.php", source, 0, character)
                .expect("legacy method context");

        let definitions = route_action_definitions(&index, &context, 0);
        let first = definitions.first().expect("definition expected");

        assert_eq!(
            first
                .pointer("/targetRange/start/line")
                .and_then(|value| value.as_u64()),
            Some(13)
        );
        assert!(
            first
                .pointer("/targetUri")
                .and_then(|value| value.as_str())
                .unwrap_or_default()
                .ends_with("/app/Http/Controllers/WebsiteController.php")
        );
        assert_eq!(
            first
                .pointer("/originSelectionRange/start/character")
                .and_then(|value| value.as_u64()),
            Some(source.find("home").unwrap() as u64)
        );
        assert_eq!(
            first
                .pointer("/originSelectionRange/end/character")
                .and_then(|value| value.as_u64()),
            Some((source.find("home").unwrap() + "home".len()) as u64)
        );
    }

    #[test]
    fn emits_missing_method_diagnostic_and_quick_fix() {
        let project = sandbox_project();
        let index = ProjectIndex::build_with_overrides(&project, &FileOverrides::default())
            .expect("index should build");
        let source = std::fs::read_to_string(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("laravel-example")
                .join("sandbox-app")
                .join("routes")
                .join("starter.php"),
        )
        .expect("starter route file should load");

        let diagnostics = route_diagnostics(&index, Path::new("routes/starter.php"), &source);
        let missing = diagnostics
            .iter()
            .find(|item| {
                item.get("code").and_then(|value| value.as_str())
                    == Some("rust-php/missing-controller-method")
            })
            .expect("missing method diagnostic should exist");

        let actions = route_action_code_actions(&index, std::slice::from_ref(missing));
        let action = actions.first().expect("quick fix should exist");

        assert!(
            action
                .get("title")
                .and_then(|value| value.as_str())
                .unwrap_or_default()
                .contains("Create controller method `missingLanding`")
        );
    }

    #[test]
    fn does_not_resolve_definition_for_missing_controller_method() {
        let project = sandbox_project();
        let index = ProjectIndex::build_with_overrides(&project, &FileOverrides::default())
            .expect("index should build");
        let source = "Route::get('/', [WebsiteController::class, 'missingLanding']);";
        let character = source.find("missingLanding").unwrap() + 3;
        let context =
            detect_route_action_context("file:///tmp/routes/web.php", source, 0, character)
                .expect("route action context");

        let definitions = route_action_definitions(&index, &context, 0);

        assert!(definitions.is_empty());
    }

    #[test]
    fn does_not_offer_quick_fix_for_invalid_php_method_name() {
        let project = sandbox_project();
        let index = ProjectIndex::build_with_overrides(&project, &FileOverrides::default())
            .expect("index should build");

        let diagnostic = json!({
            "code": "rust-php/missing-controller-method",
            "data": {
                "controller": "App\\Http\\Controllers\\WebsiteController",
                "method": "page.sustainability"
            }
        });

        let actions = route_action_code_actions(&index, &[diagnostic]);
        assert!(actions.is_empty());
    }

    #[test]
    fn view_hover_and_definition_use_exact_view_token_range() {
        let project = view_project();
        let index = ProjectIndex::build_with_overrides(&project, &FileOverrides::default())
            .expect("index should build");
        let source = "return view('admin.users.index');";
        let character = source.find("users").unwrap() + 2;
        let context = detect_symbol_context(source, 0, character).expect("view context");

        let hover = hover(&index, &context, 0).expect("hover expected");
        assert_eq!(
            hover
                .pointer("/range/start/character")
                .and_then(|value| value.as_u64()),
            Some(source.find("admin.users.index").unwrap() as u64)
        );
        assert_eq!(
            hover
                .pointer("/range/end/character")
                .and_then(|value| value.as_u64()),
            Some((source.find("admin.users.index").unwrap() + "admin.users.index".len()) as u64)
        );

        let definitions = definitions(&index, &context, 0);
        let first = definitions.first().expect("definition expected");
        assert_eq!(
            first
                .pointer("/originSelectionRange/start/character")
                .and_then(|value| value.as_u64()),
            Some(source.find("admin.users.index").unwrap() as u64)
        );
        assert_eq!(
            first
                .pointer("/originSelectionRange/end/character")
                .and_then(|value| value.as_u64()),
            Some((source.find("admin.users.index").unwrap() + "admin.users.index".len()) as u64)
        );
        assert!(
            first
                .pointer("/targetUri")
                .and_then(|value| value.as_str())
                .unwrap_or_default()
                .ends_with("/resources/views/admin/users/index.blade.php")
        );
    }

    #[test]
    fn symbol_definitions_use_exact_origin_selection_range() {
        let project = symbol_project();
        let index = ProjectIndex::build_with_overrides(&project, &FileOverrides::default())
            .expect("index should build");

        let cases = [
            (
                "return config('debug.enabled');",
                "debug.enabled",
                "config/debug.php",
            ),
            (
                "return route('dashboard.home');",
                "dashboard.home",
                "routes/web.php",
            ),
            (
                "return env('APP_DEBUG');",
                "APP_DEBUG",
                ".env",
            ),
        ];

        for (source, token, target_suffix) in cases {
            let character = source.find(token).unwrap() + 2;
            let context = detect_symbol_context(source, 0, character).expect("symbol context");
            let definitions = definitions(&index, &context, 0);
            let first = definitions.first().expect("definition expected");

            assert_eq!(
                first
                    .pointer("/originSelectionRange/start/character")
                    .and_then(|value| value.as_u64()),
                Some(source.find(token).unwrap() as u64)
            );
            assert_eq!(
                first
                    .pointer("/originSelectionRange/end/character")
                    .and_then(|value| value.as_u64()),
                Some((source.find(token).unwrap() + token.len()) as u64)
            );
            assert!(
                first
                    .pointer("/targetUri")
                    .and_then(|value| value.as_str())
                    .unwrap_or_default()
                    .ends_with(target_suffix)
            );
        }
    }
}
