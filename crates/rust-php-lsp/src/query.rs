use serde_json::{Value, json};
use std::cmp::Reverse;
use std::fs;
use std::path::{Path, PathBuf};

use super::context::{
    BladeComponentAttrContext, BladeComponentTagContext, BladeVariableContext, HelperContext,
    HelperStyle, RouteActionContext, RouteActionKind, SymbolContext, SymbolKind, ViewDataContext,
    ViewDataKind,
};
use super::index::ProjectIndex;
use super::index::fuzzy_score;
use crate::types::{
    BladeComponentEntry, ConfigItem, ControllerEntry, ControllerMethodEntry, EnvItem,
    PublicAssetEntry, RouteEntry, ViewEntry, ViewVariable,
};
use rust_php_markdown::{
    asset::{self, AssetHoverInput},
    controller, env, route, view, DocBundle,
};

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
            .map(|route| route_completion(index, route, context, line))
            .collect(),
        SymbolKind::Env => index
            .env_matches(&context.prefix)
            .into_iter()
            .map(|item| env_completion(index, item, context, line))
            .collect(),
        SymbolKind::View => index
            .view_matches(&context.prefix)
            .into_iter()
            .map(|view| view_completion(index, view, context, line))
            .collect(),
        SymbolKind::Asset => index
            .public_asset_matches(&context.prefix)
            .into_iter()
            .map(|asset| asset_completion(index, asset, context, line))
            .collect(),
    }
}

pub fn complete_view_data_variables(
    source: &str,
    context: &ViewDataContext,
    line: usize,
) -> Vec<Value> {
    let candidates = match context.kind {
        ViewDataKind::CompactVariable => local_view_data_variables(source, context.cursor_offset),
    };

    let mut matches = candidates
        .into_iter()
        .filter_map(|name| {
            let score = fuzzy_score(&name, &context.prefix)?;
            Some((score, name.len(), name))
        })
        .collect::<Vec<_>>();

    matches.sort_by_key(|(score, len, label)| (Reverse(*score), *len, label.clone()));
    matches
        .into_iter()
        .map(|(_, _, name)| view_data_variable_completion(&name, context, line))
        .collect()
}

pub fn complete_blade_view_variables(
    index: &ProjectIndex,
    file: &Path,
    context: &BladeVariableContext,
    line: usize,
) -> Vec<Value> {
    index
        .blade_variables_for_file(file, &context.prefix)
        .into_iter()
        .map(|variable| blade_view_variable_completion(variable, context, line))
        .collect()
}

pub fn helper_snippets(context: &HelperContext, line: usize) -> Vec<Value> {
    ranked_helper_specs(&context.prefix)
        .into_iter()
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
            .map(|controller| controller_completion(index, controller, context, line))
            .collect(),
        RouteActionKind::ControllerMethodArray | RouteActionKind::LegacyMethodString => context
            .controller
            .as_deref()
            .into_iter()
            .flat_map(|controller| index.controller_methods(controller, &context.prefix))
            .map(|(controller, method)| {
                controller_method_completion(index, controller, method, context, line)
            })
            .collect(),
    }
}

pub fn complete_blade_components(
    index: &ProjectIndex,
    context: &BladeComponentTagContext,
    line: usize,
) -> Vec<Value> {
    let mut seen = std::collections::HashSet::new();
    index
        .blade_component_matches(&context.prefix)
        .into_iter()
        .filter(|c| seen.insert(c.component.clone()))
        .map(|component| blade_component_tag_completion(component, index, context, line))
        .collect()
}

pub fn complete_blade_component_props(
    index: &ProjectIndex,
    context: &BladeComponentAttrContext,
    line: usize,
) -> Vec<Value> {
    let Some(component) = index
        .blade_component_definitions(&context.component)
        .into_iter()
        .next()
    else {
        return Vec::new();
    };

    let mut matches: Vec<_> = component
        .props
        .iter()
        .filter(|prop| !context.already_present.contains(&prop.name))
        .filter_map(|prop| {
            let score = fuzzy_score(&prop.name, &context.prefix)?;
            Some((score, prop.name.len(), prop.name.as_str(), prop))
        })
        .collect();

    matches.sort_by_key(|(score, len, label, _)| (Reverse(*score), *len, *label));
    matches
        .into_iter()
        .map(|(_, _, _, prop)| blade_component_attr_completion(prop, context, line))
        .collect()
}

pub fn blade_component_hover(
    index: &ProjectIndex,
    context: &BladeComponentTagContext,
    line: usize,
) -> Option<Value> {
    let components = index.blade_component_definitions(&context.full_text);
    if components.is_empty() {
        return None;
    }
    let range = hover_range(
        line,
        blade_component_selection_start_character(context),
        context.end_character,
    );
    Some(json!({
        "contents": { "kind": "markdown", "value": blade_component_hover_text_all(&components, &index.project_root) },
        "range": range,
    }))
}

pub fn blade_component_definitions(
    index: &ProjectIndex,
    context: &BladeComponentTagContext,
    line: usize,
) -> Vec<Value> {
    index
        .blade_component_definitions(&context.full_text)
        .into_iter()
        .filter_map(|component| {
            let file = component
                .class_file
                .as_ref()
                .or(component.view_file.as_ref())?;
            Some(json!({
                "originSelectionRange": {
                    "start": {
                        "line": line,
                        "character": blade_component_selection_start_character(context),
                    },
                    "end": { "line": line, "character": context.end_character },
                },
                "targetUri": path_to_file_uri(&index.project_root.join(file)),
                "targetRange": {
                    "start": { "line": 0, "character": 0 },
                    "end": { "line": 0, "character": 0 },
                },
                "targetSelectionRange": {
                    "start": { "line": 0, "character": 0 },
                    "end": { "line": 0, "character": 0 },
                },
            }))
        })
        .collect()
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
        SymbolKind::Asset => asset_location(index, &context.full_text)
            .into_iter()
            .map(|relative| location(&index.project_root, &relative, 1, 1))
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
                "contents": { "kind": "markdown", "value": route_hover(index, route) },
                "range": range,
            }))
        }
        SymbolKind::Env => {
            let item = index
                .env_definitions(&context.full_text)
                .into_iter()
                .next()?;
            Some(json!({
                "contents": { "kind": "markdown", "value": env_hover(index, item) },
                "range": range,
            }))
        }
        SymbolKind::View => {
            let view = index
                .view_definitions(&context.full_text)
                .into_iter()
                .next()?;
            Some(json!({
                "contents": { "kind": "markdown", "value": view_hover(index, view) },
                "range": range,
            }))
        }
        SymbolKind::Asset => Some(json!({
            "contents": { "kind": "markdown", "value": asset_hover(index, &context.full_text) },
            "range": range,
        })),
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
                "contents": { "kind": "markdown", "value": controller_hover(index, item) },
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
                "contents": { "kind": "markdown", "value": controller_method_hover(index, owner, method) },
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

pub fn asset_code_actions(index: &ProjectIndex, context: &SymbolContext) -> Vec<Value> {
    let Some(relative) = asset_location(index, &context.full_text) else {
        return Vec::new();
    };
    let absolute = index.project_root.join(relative);

    vec![json!({
        "title": "Open asset in Zed",
        "command": "rust-php.openAssetInZed",
        "arguments": [absolute.display().to_string()],
    })]
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

fn route_completion(index: &ProjectIndex, route: &RouteEntry, context: &SymbolContext, line: usize) -> Value {
    let name = route.name.as_deref().unwrap_or_default();
    let detail = format!("{} {}", route.methods.join("|"), route.uri);
    let parameter_patterns = route
        .parameter_patterns
        .iter()
        .map(|(name, pattern)| (name.clone(), pattern.clone()))
        .collect::<Vec<_>>();
    let docs = route::build(route::RouteHoverInput {
        name: name.to_string(),
        methods: route.methods.clone(),
        uri: route.uri.clone(),
        action: route.action.clone(),
        resolved_middleware: route.resolved_middleware.clone(),
        parameter_patterns,
        source: route.file.display().to_string(),
        source_uri: Some(path_to_file_uri(&index.project_root.join(&route.file))),
        line: route.line,
        column: route.column,
        detail: Some(detail.clone()),
    });

    json!({
        "label": name,
        "kind": 18,
        "detail": docs.detail.clone().unwrap_or(detail),
        "documentation": {
            "kind": "markdown",
            "value": docs.completion_markdown(),
        },
        "textEdit": replacement_edit(context, line, name),
    })
}

fn env_completion(index: &ProjectIndex, item: &EnvItem, context: &SymbolContext, line: usize) -> Value {
    let docs = env::build(env::EnvHoverInput {
        key: item.key.clone(),
        value: item.value.clone(),
        source: item.file.display().to_string(),
        source_uri: Some(path_to_file_uri(&index.project_root.join(&item.file))),
        line: item.line,
        column: item.column,
        detail: Some(minify_completion_value(&item.value)),
    });
    json!({
        "label": item.key,
        "kind": 21,
        "detail": docs.detail.clone().unwrap_or_default(),
        "documentation": {
            "kind": "markdown",
            "value": docs.completion_markdown(),
        },
        "textEdit": replacement_edit(context, line, &item.key),
    })
}

fn asset_completion(index: &ProjectIndex, asset: &PublicAssetEntry, context: &SymbolContext, line: usize) -> Value {
    let docs = asset_docs_from_entry(index, asset);
    json!({
        "label": asset.asset_path,
        "kind": 17,
        "detail": docs.detail.clone().unwrap_or_default(),
        "filterText": asset.asset_path,
        "documentation": {
            "kind": "markdown",
            "value": docs.completion_markdown(),
        },
        "textEdit": replacement_edit(context, line, &asset.asset_path),
    })
}

fn asset_docs_from_entry(index: &ProjectIndex, asset: &PublicAssetEntry) -> DocBundle {
    let file_name = std::path::Path::new(&asset.asset_path)
        .file_name()
        .and_then(|n| n.to_str())
        .map(|s| s.to_string());

    let file_uri = Some(path_to_file_uri(&index.project_root.join(&asset.file)));

    rust_php_markdown::asset::build(AssetHoverInput {
        asset_path: asset.asset_path.clone(),
        file_name,
        file_uri,
        size_display: Some(format_size(asset.size_bytes as u64)),
        extension: asset.extension.clone(),
        usages: asset.usages.len(),
        status: asset::AssetStatus::Resolved,
        completion_detail: asset_completion_detail(asset),
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

fn view_data_variable_completion(name: &str, context: &ViewDataContext, line: usize) -> Value {
    json!({
        "label": name,
        "kind": 6,
        "detail": "Local variable in current function",
        "documentation": {
            "kind": "markdown",
            "value": format!("`{name}`\n- available as a local variable in the current controller method"),
        },
        "textEdit": {
            "range": {
                "start": { "line": line, "character": context.start_character },
                "end": { "line": line, "character": context.end_character },
            },
            "newText": name,
        }
    })
}

fn blade_view_variable_completion(
    variable: &ViewVariable,
    context: &BladeVariableContext,
    line: usize,
) -> Value {
    let detail = variable
        .default_value
        .as_ref()
        .map(|value| format!("Blade view variable = {value}"))
        .unwrap_or_else(|| "Blade view variable".to_string());
    json!({
        "label": format!("${}", variable.name),
        "filterText": variable.name,
        "kind": 6,
        "detail": detail,
        "documentation": {
            "kind": "markdown",
            "value": format!("`${}`\n- available in the current Blade view", variable.name),
        },
        "textEdit": {
            "range": {
                "start": { "line": line, "character": context.start_character },
                "end": { "line": line, "character": context.end_character },
            },
            "newText": variable.name,
        }
    })
}

fn controller_completion(
    index: &ProjectIndex,
    controller: &ControllerEntry,
    context: &RouteActionContext,
    line: usize,
) -> Value {
    let insert = controller.class_name.as_str();
    let docs = controller::build(controller::ControllerHoverInput {
        label: controller.class_name.clone(),
        fqn: controller.fqn.clone(),
        callable_methods: controller.callable_method_count,
        total_methods: controller.method_count,
        source: controller.file.display().to_string(),
        source_uri: Some(path_to_file_uri(&index.project_root.join(&controller.file))),
        line: controller.line,
        extends: controller.extends.clone(),
        traits: controller.traits.clone(),
        detail: Some(controller.fqn.clone()),
    });
    json!({
        "label": controller.class_name,
        "kind": 7,
        "detail": docs.detail.clone().unwrap_or_else(|| controller.fqn.clone()),
        "documentation": {
            "kind": "markdown",
            "value": docs.completion_markdown(),
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
    index: &ProjectIndex,
    controller: &ControllerEntry,
    method: &ControllerMethodEntry,
    context: &RouteActionContext,
    line: usize,
) -> Value {
    let new_text = match context.kind {
        RouteActionKind::LegacyMethodString => method.name.clone(),
        _ => method.name.clone(),
    };
    let docs = controller::build_method(controller::ControllerMethodHoverInput {
        label: format!("{}::{}", controller.class_name, method.name),
        controller_fqn: controller.fqn.clone(),
        route_callable: method.accessible_from_route,
        visibility: method.visibility.clone(),
        source_kind: method.source_kind.clone(),
        notes: method.accessibility.clone(),
        source: method.declared_in.display().to_string(),
        source_uri: Some(path_to_file_uri(&index.project_root.join(&method.declared_in))),
        line: method.line,
        detail: Some(format!("{} {}", controller.class_name, method.accessibility)),
    });

    json!({
        "label": method.name,
        "kind": 2,
        "detail": format!("{} {}", controller.class_name, method.accessibility),
        "documentation": {
            "kind": "markdown",
            "value": docs.completion_markdown(),
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

fn blade_component_selection_start_character(context: &BladeComponentTagContext) -> usize {
    context.tag_start_character + 1
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

fn view_completion(index: &ProjectIndex, view: &ViewEntry, context: &SymbolContext, line: usize) -> Value {
    let docs = view::build(view::ViewHoverInput {
        name: view.name.clone(),
        kind: view.kind.clone(),
        file: view.file.display().to_string(),
        file_uri: Some(path_to_file_uri(&index.project_root.join(&view.file))),
        usages: view.usages.len(),
        props: view.props.iter().map(|p| p.name.clone()).collect(),
        detail: Some(view.file.display().to_string()),
    });
    json!({
        "label": view.name,
        "kind": 17,
        "detail": docs.detail.clone().unwrap_or_else(|| view.file.display().to_string()),
        "documentation": {
            "kind": "markdown",
            "value": docs.completion_markdown(),
        },
        "textEdit": replacement_edit(context, line, &view.name),
    })
}

fn view_hover(index: &ProjectIndex, view: &ViewEntry) -> String {
    view::build(view::ViewHoverInput {
        name: view.name.clone(),
        kind: view.kind.clone(),
        file: view.file.display().to_string(),
        file_uri: Some(path_to_file_uri(&index.project_root.join(&view.file))),
        usages: view.usages.len(),
        props: view.props.iter().map(|p| p.name.clone()).collect(),
        detail: None,
    })
    .hover_markdown()
}

fn blade_component_tag_completion(
    component: &BladeComponentEntry,
    context: &BladeComponentTagContext,
    line: usize,
    project_root: &Path,
) -> Value {
    let name = &component.component;
    let snippet = format!("{name} $1>\n\t$2\n</x-{name}>$0");
    json!({
        "label": format!("x-{name}"),
        "kind": 10,
        "detail": blade_component_detail(component),
        "filterText": name,
        "insertTextFormat": 2,
        "documentation": { "kind": "markdown", "value": blade_component_hover_text(component, project_root) },
        "textEdit": {
            "range": {
                "start": { "line": line, "character": context.start_character },
                "end": { "line": line, "character": context.end_character },
            },
            "newText": snippet,
        },
        "additionalTextEdits": [],
    })
}

fn blade_component_attr_completion(
    prop: &ViewVariable,
    context: &BladeComponentAttrContext,
    line: usize,
) -> Value {
    let name = &prop.name;
    let (new_text, detail) = if context.already_typed_colon {
        (format!(":{name}=\"${{1}}\""), format!(":{name}=\"...\""))
    } else {
        (format!("{name}=\"${{1}}\""), format!("{name}=\"...\""))
    };
    let doc = match &prop.default_value {
        Some(v) => format!("`{name}` — default: `{v}`"),
        None => format!("`{name}` — required prop"),
    };
    json!({
        "label": name,
        "kind": 5,
        "detail": detail,
        "documentation": { "kind": "markdown", "value": doc },
        "insertTextFormat": 2,
        "textEdit": {
            "range": {
                "start": { "line": line, "character": context.start_character },
                "end": { "line": line, "character": context.end_character },
            },
            "newText": new_text,
        }
    })
}

fn blade_component_detail(component: &BladeComponentEntry) -> String {
    if let Some(class) = &component.class_name {
        format!("{class} ({})", component.kind)
    } else {
        format!("Blade component ({})", component.kind)
    }
}

fn blade_component_hover_text(component: &BladeComponentEntry, project_root: &Path) -> String {
    blade_component_hover_text_all(&[component], project_root)
}

fn blade_component_hover_text_all(
    components: &[&BladeComponentEntry],
    project_root: &Path,
) -> String {
    let first = components[0];
    let mut lines = vec![format!("`x-{}`", first.component)];

    for c in components {
        if let Some(class) = &c.class_name {
            if let Some(class_file) = &c.class_file {
                let absolute = project_root.join(class_file);
                let uri = path_to_file_uri(&absolute);
                lines.push(format!("- class: [`{class}`]({})", uri));
            } else {
                lines.push(format!("- class: `{class}`"));
            }
        }
    }
    let mut seen_files = std::collections::HashSet::new();
    for c in components {
        // if let Some(file) = &c.class_file {
        //     if seen_files.insert(file.clone()) {
        //         let absolute = project_root.join(file);
        //         let uri = path_to_file_uri(&absolute);
        //         lines.push(format!("- class file: [{}]({})", file.display(), uri));
        //     }
        // }
        if let Some(file) = &c.view_file {
            if seen_files.insert(file.clone()) {
                let absolute = project_root.join(file);
                let uri = path_to_file_uri(&absolute);
                lines.push(format!("- blade file: [{}]({})", file.display(), uri));
            }
        }
    }

    let mut seen_props = std::collections::HashSet::new();
    let props: Vec<String> = components
        .iter()
        .flat_map(|c| c.props.iter())
        .filter(|p| seen_props.insert(p.name.clone()))
        .map(|p| match &p.default_value {
            Some(v) => format!("{} = {v}", p.name),
            None => p.name.clone(),
        })
        .collect();
    if !props.is_empty() {
        lines.push(format!("- props: `{}`", props.join(", ")));
    }

    lines.join("\n")
}

fn asset_hover(index: &ProjectIndex, asset_path: &str) -> String {
    asset_docs(index, asset_path, false).hover_markdown()
}

fn asset_completion_detail(asset: &PublicAssetEntry) -> String {
    let mut detail = format!("public/{}", asset.asset_path);

    if let Some(extension) = &asset.extension {
        detail.push_str(&format!(" · .{extension}"));
    }
    if asset.size_bytes > 0 {
        detail.push_str(&format!(" · {} bytes", asset.size_bytes));
    }
    if !asset.usages.is_empty() {
        detail.push_str(&format!(" · {} uses", asset.usages.len()));
    }

    detail
}

fn asset_completion_hover(index: &ProjectIndex, asset: &PublicAssetEntry) -> String {
    asset_docs_from_entry(index, asset).completion_markdown()
}

fn asset_docs(index: &ProjectIndex, asset_path: &str, for_completion: bool) -> DocBundle {
    let (file_name, size_display, file_uri, status) =
        if let Some(relative) = asset_relative_file(asset_path) {
            let absolute = index.project_root.join(&relative);
            if absolute.exists() {
                let file_name = absolute
                    .file_name()
                    .and_then(|n| n.to_str())
                    .map(|s| s.to_string());
                let size_display = if for_completion {
                    fs::metadata(&absolute)
                        .ok()
                        .map(|metadata| format_size(metadata.len()))
                } else {
                    fs::metadata(&absolute)
                        .ok()
                        .map(|metadata| format_size(metadata.len()))
                };
                (
                    file_name,
                    size_display,
                    Some(path_to_file_uri(&absolute)),
                    asset::AssetStatus::Resolved,
                )
            } else {
                (None, None, None, asset::AssetStatus::Missing)
            }
        } else {
            (None, None, None, asset::AssetStatus::Unresolved)
        };

    rust_php_markdown::asset::build(AssetHoverInput {
        asset_path: asset_path.to_string(),
        file_name,
        file_uri,
        size_display,
        extension: None,
        usages: 0,
        status,
        completion_detail: asset_completion_detail_for_path(asset_path, for_completion),
    })
}

fn env_hover(index: &ProjectIndex, item: &EnvItem) -> String {
    env::build(env::EnvHoverInput {
        key: item.key.clone(),
        value: item.value.clone(),
        source: item.file.display().to_string(),
        source_uri: Some(path_to_file_uri(&index.project_root.join(&item.file))),
        line: item.line,
        column: item.column,
        detail: None,
    })
    .hover_markdown()
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

fn route_hover(index: &ProjectIndex, route: &RouteEntry) -> String {
    let parameter_patterns = route
        .parameter_patterns
        .iter()
        .map(|(name, pattern)| (name.clone(), pattern.clone()))
        .collect::<Vec<_>>();
    route::build(route::RouteHoverInput {
        name: route.name.as_deref().unwrap_or("<unnamed-route>").to_string(),
        methods: route.methods.clone(),
        uri: route.uri.clone(),
        action: route.action.clone(),
        resolved_middleware: route.resolved_middleware.clone(),
        parameter_patterns,
        source: route.file.display().to_string(),
        source_uri: Some(path_to_file_uri(&index.project_root.join(&route.file))),
        line: route.line,
        column: route.column,
        detail: None,
    })
    .hover_markdown()
}

fn controller_hover(index: &ProjectIndex, controller: &ControllerEntry) -> String {
    controller::build(controller::ControllerHoverInput {
        label: controller.class_name.clone(),
        fqn: controller.fqn.clone(),
        callable_methods: controller.callable_method_count,
        total_methods: controller.method_count,
        source: controller.file.display().to_string(),
        source_uri: Some(path_to_file_uri(&index.project_root.join(&controller.file))),
        line: controller.line,
        extends: controller.extends.clone(),
        traits: controller.traits.clone(),
        detail: None,
    })
    .hover_markdown()
}

fn controller_method_hover(
    index: &ProjectIndex,
    controller: &ControllerEntry,
    method: &ControllerMethodEntry,
) -> String {
    controller::build_method(controller::ControllerMethodHoverInput {
        label: format!("{}::{}", controller.class_name, method.name),
        controller_fqn: controller.fqn.clone(),
        route_callable: method.accessible_from_route,
        visibility: method.visibility.clone(),
        source_kind: method.source_kind.clone(),
        notes: method.accessibility.clone(),
        source: method.declared_in.display().to_string(),
        source_uri: Some(path_to_file_uri(&index.project_root.join(&method.declared_in))),
        line: method.line,
        detail: None,
    })
    .hover_markdown()
}

fn asset_location(index: &ProjectIndex, asset_path: &str) -> Option<PathBuf> {
    let relative = asset_relative_file(asset_path)?;
    let absolute = index.project_root.join(&relative);
    absolute.exists().then_some(relative)
}

fn asset_relative_file(asset_path: &str) -> Option<PathBuf> {
    let trimmed = asset_path.trim();
    if trimmed.is_empty() || trimmed.contains("://") || trimmed.starts_with("//") {
        return None;
    }

    let relative = trimmed.trim_start_matches('/');
    if relative.is_empty() {
        return None;
    }

    Some(Path::new("public").join(relative))
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

fn ranked_helper_specs(query: &str) -> Vec<&'static HelperSpec> {
    let mut matches = helper_specs()
        .iter()
        .filter_map(|helper| {
            let score = fuzzy_score(helper.name, query)?;
            Some((score, helper.name.len(), helper.name, helper))
        })
        .collect::<Vec<_>>();

    matches.sort_by_key(|(score, len, label, _)| (Reverse(*score), *len, *label));
    matches
        .into_iter()
        .map(|(_, _, _, helper)| helper)
        .collect()
}

fn local_view_data_variables(source: &str, cursor: usize) -> Vec<String> {
    let Some((signature_start, body_start, body_end)) = enclosing_function_bounds(source, cursor)
    else {
        return Vec::new();
    };

    let mut names = extract_function_parameters(&source[signature_start..body_start]);
    names.extend(extract_assigned_variables(&source[body_start..body_end]));
    names.retain(|name| name != "this");
    names.sort();
    names.dedup();
    names
}

fn enclosing_function_bounds(source: &str, cursor: usize) -> Option<(usize, usize, usize)> {
    let before = &source[..cursor.min(source.len())];
    let function_start = before.rfind("function")?;
    let signature = &source[function_start..];
    let open_paren_rel = signature.find('(')?;
    let open_paren = function_start + open_paren_rel;
    let close_paren = find_matching_delimiter(source, open_paren, '(', ')')?;
    let body_start = source[close_paren..]
        .find('{')
        .map(|rel| close_paren + rel)?;
    let body_end = find_matching_delimiter(source, body_start, '{', '}')?;

    if cursor < body_start || cursor > body_end {
        return None;
    }

    Some((function_start, body_start + 1, body_end))
}

fn extract_function_parameters(signature: &str) -> Vec<String> {
    let Some(open_paren) = signature.find('(') else {
        return Vec::new();
    };
    let Some(close_paren) = find_matching_delimiter(signature, open_paren, '(', ')') else {
        return Vec::new();
    };

    split_top_level(&signature[open_paren + 1..close_paren], ',')
        .into_iter()
        .filter_map(|part| extract_dollar_variable_name(&part))
        .collect()
}

fn extract_assigned_variables(body: &str) -> Vec<String> {
    let mut names = Vec::new();
    let bytes = body.as_bytes();
    let mut index = 0usize;

    while index < bytes.len() {
        if bytes[index] != b'$' {
            index += 1;
            continue;
        }
        if index > 0 {
            let prev = body[..index].chars().next_back().unwrap_or(' ');
            if prev.is_ascii_alphanumeric() || prev == '_' {
                index += 1;
                continue;
            }
        }

        let rest = &body[index + 1..];
        let name_len = rest
            .chars()
            .take_while(|ch| ch.is_ascii_alphanumeric() || *ch == '_')
            .map(char::len_utf8)
            .sum::<usize>();
        if name_len == 0 {
            index += 1;
            continue;
        }

        let name = &rest[..name_len];
        let after_name = &rest[name_len..];
        let trimmed = after_name.trim_start();
        if trimmed.starts_with("->") || trimmed.starts_with("::") {
            index += 1;
            continue;
        }
        if trimmed.starts_with('=') {
            let mut chars = trimmed.chars();
            chars.next();
            let next = chars.next();
            if next != Some('=') {
                names.push(name.to_string());
            }
        }

        index += 1 + name_len;
    }

    names
}

fn extract_dollar_variable_name(text: &str) -> Option<String> {
    let dollar = text.find('$')?;
    let after = &text[dollar + 1..];
    let name: String = after
        .chars()
        .take_while(|ch| ch.is_ascii_alphanumeric() || *ch == '_')
        .collect();
    if name.is_empty() { None } else { Some(name) }
}

fn find_matching_delimiter(
    source: &str,
    open_index: usize,
    open: char,
    close: char,
) -> Option<usize> {
    if source[open_index..].chars().next()? != open {
        return None;
    }

    let mut depth = 0usize;
    let mut in_single = false;
    let mut in_double = false;
    let mut escape = false;

    for (relative, ch) in source[open_index..].char_indices() {
        let index = open_index + relative;

        if in_single {
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
            '\'' => in_single = true,
            '"' => in_double = true,
            _ if ch == open => depth += 1,
            _ if ch == close => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return Some(index);
                }
            }
            _ => {}
        }
    }

    None
}

fn split_top_level(source: &str, separator: char) -> Vec<String> {
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut paren = 0usize;
    let mut bracket = 0usize;
    let mut brace = 0usize;
    let mut in_single = false;
    let mut in_double = false;
    let mut escape = false;

    for ch in source.chars() {
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
                paren += 1;
                current.push(ch);
            }
            ')' => {
                paren = paren.saturating_sub(1);
                current.push(ch);
            }
            '[' => {
                bracket += 1;
                current.push(ch);
            }
            ']' => {
                bracket = bracket.saturating_sub(1);
                current.push(ch);
            }
            '{' => {
                brace += 1;
                current.push(ch);
            }
            '}' => {
                brace = brace.saturating_sub(1);
                current.push(ch);
            }
            _ if ch == separator && paren == 0 && bracket == 0 && brace == 0 => {
                parts.push(current.trim().to_string());
                current.clear();
            }
            _ => current.push(ch),
        }
    }

    if !current.trim().is_empty() {
        parts.push(current.trim().to_string());
    }

    parts
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

fn location(
    project_root: &std::path::Path,
    relative_file: &std::path::Path,
    target_line: usize,
    target_column: usize,
) -> Value {
    let absolute = project_root.join(relative_file);
    json!({
        "uri": path_to_file_uri(&absolute),
        "range": {
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

    use crate::lsp::context::{
        detect_blade_component_tag_context, detect_blade_variable_context,
        detect_route_action_context, detect_symbol_context, HelperContext, HelperStyle,
        SymbolKind,
    };
    use crate::lsp::index::ProjectIndex;
    use crate::lsp::overrides::FileOverrides;
    use crate::project;

    use super::{
        asset_code_actions, blade_component_definitions, complete, complete_blade_view_variables,
        complete_route_actions, complete_view_data_variables, definitions, helper_snippets, hover,
        route_action_code_actions, route_action_definitions, route_diagnostics,
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
        $headline = 'Team';
        $cta = 'Invite';

        return view('admin.users.index', compact('headline', 'cta'), ['version' => 2]);
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

    fn blade_component_project() -> project::LaravelProject {
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
        write_file(&root, "app/View/Components/ProfileCard.php", "<?php\n");
        write_file(&root, "config/app.php", "<?php\n\nreturn [];\n");
        write_file(&root, "routes/web.php", "<?php\n");
        write_file(
            &root,
            "resources/views/components/profile-card.blade.php",
            "<div>Card</div>\n",
        );
        write_file(
            &root,
            "resources/views/pages/workspaces/virtual-office/index.blade.php",
            "<x-profile-card />\n",
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

    fn asset_project() -> project::LaravelProject {
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
            "public/assets/images/virtual/centers-card-img.png",
            "png",
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
    fn completes_route_callable_controller_methods_while_editing_existing_method() {
        let project = sandbox_project();
        let index = ProjectIndex::build_with_overrides(&project, &FileOverrides::default())
            .expect("index should build");
        let source = "Route::get('/', [WebsiteController::class, 'hom'])->name('home');";
        let character = source.find("hom']").unwrap() + "hom".len();
        let context =
            detect_route_action_context("file:///tmp/routes/web.php", source, 0, character)
                .expect("route action context");

        let items = complete_route_actions(&index, &context, 0);
        let home = items
            .iter()
            .find(|item| item.get("label").and_then(|value| value.as_str()) == Some("home"))
            .expect("home completion should exist");

        assert_eq!(
            home.pointer("/textEdit/newText")
                .and_then(|value| value.as_str()),
            Some("home")
        );
        assert_eq!(
            home.pointer("/textEdit/range/start/character")
                .and_then(|value| value.as_u64()),
            Some(source.find("hom").unwrap() as u64)
        );
        assert_eq!(
            home.pointer("/textEdit/range/end/character")
                .and_then(|value| value.as_u64()),
            Some((source.find("hom").unwrap() + "hom".len()) as u64)
        );
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
    fn blade_component_definition_uses_x_prefixed_origin_selection_range() {
        let project = blade_component_project();
        let index = ProjectIndex::build_with_overrides(&project, &FileOverrides::default())
            .expect("index should build");
        let source = "<x-profile-card />\n";
        let character = source.find("profile-card").unwrap() + 3;
        let context = detect_blade_component_tag_context(
            "file:///tmp/resources/views/pages/workspaces/virtual-office/index.blade.php",
            source,
            0,
            character,
        )
        .expect("blade component context");

        let definitions = blade_component_definitions(&index, &context, 0);
        assert!(!definitions.is_empty());
        let first = definitions.first().expect("definition expected");

        assert_eq!(
            first
                .pointer("/originSelectionRange/start/character")
                .and_then(|value| value.as_u64()),
            Some(source.find("x-profile-card").unwrap() as u64)
        );
        assert_eq!(
            first
                .pointer("/originSelectionRange/end/character")
                .and_then(|value| value.as_u64()),
            Some((source.find("x-profile-card").unwrap() + "x-profile-card".len()) as u64)
        );
        assert!(
            first
                .pointer("/targetUri")
                .and_then(|value| value.as_str())
                .unwrap_or_default()
                .ends_with("/app/View/Components/ProfileCard.php")
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
            ("return env('APP_DEBUG');", "APP_DEBUG", ".env"),
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

    #[test]
    fn asset_hover_and_definition_resolve_public_file() {
        let project = asset_project();
        let index = ProjectIndex::build_with_overrides(&project, &FileOverrides::default())
            .expect("index should build");
        let source = "return secure_asset('assets/images/virtual/centers-card-img.png');";
        let character = source.find("virtual").unwrap() + 2;
        let context = detect_symbol_context(source, 0, character).expect("asset context");

        assert_eq!(context.kind, SymbolKind::Asset);

        let hover = hover(&index, &context, 0).expect("hover expected");
        let hover_value = hover
            .pointer("/contents/value")
            .and_then(|value| value.as_str())
            .unwrap_or_default();
        assert!(hover_value.contains("public/assets/images/virtual/centers-card-img.png"));
        assert!(hover_value.contains("/public/assets/images/virtual/centers-card-img.png"));
        assert!(hover_value.contains("[assets/images/virtual/centers-card-img.png](file://"));

        let definitions = definitions(&index, &context, 0);
        let first = definitions.first().expect("definition expected");
        assert!(
            first
                .pointer("/uri")
                .and_then(|value| value.as_str())
                .unwrap_or_default()
                .ends_with("/public/assets/images/virtual/centers-card-img.png")
        );

        let actions = asset_code_actions(&index, &context);
        let first = actions.first().expect("asset code action expected");
        assert_eq!(
            first.get("title").and_then(|value| value.as_str()),
            Some("Open asset in Zed")
        );
        assert!(
            first
                .pointer("/arguments/0")
                .and_then(|value| value.as_str())
                .unwrap_or_default()
                .ends_with("/public/assets/images/virtual/centers-card-img.png")
        );

        let completions = complete(&index, &context, 0);
        let labels = completions
            .iter()
            .filter_map(|item| item.get("label").and_then(|value| value.as_str()))
            .collect::<Vec<_>>();
        assert!(labels.contains(&"assets/images/virtual/centers-card-img.png"));
    }

    #[test]
    fn asset_completion_matches_partial_fuzzy_queries() {
        let project = asset_project();
        let index = ProjectIndex::build_with_overrides(&project, &FileOverrides::default())
            .expect("index should build");
        let source = "return asset('assets/images/vir');";
        let character = source.find("vir").unwrap() + 3;
        let context = detect_symbol_context(source, 0, character).expect("asset context");

        let completions = complete(&index, &context, 0);
        let labels = completions
            .iter()
            .filter_map(|item| item.get("label").and_then(|value| value.as_str()))
            .collect::<Vec<_>>();

        assert!(labels.contains(&"assets/images/virtual/centers-card-img.png"));
    }

    #[test]
    fn helper_snippets_follow_the_shared_fuzzy_search_standard() {
        let context = HelperContext {
            prefix: "set".to_string(),
            start_character: 0,
            end_character: 3,
            style: HelperStyle::Php,
        };

        let items = helper_snippets(&context, 0);
        let labels = items
            .iter()
            .filter_map(|item| item.get("label").and_then(|value| value.as_str()))
            .collect::<Vec<_>>();

        assert!(labels.contains(&"asset"));
    }

    #[test]
    fn completes_local_controller_variables_inside_compact_strings() {
        let source = r#"<?php

class DemoController
{
    public function index(array $requestFilters)
    {
        $pageTitle = 'Blade IDE Sandbox';
        $currentUser = ['name' => 'Maya'];
        $examples = [];
        $breadcrumbs = ['Home'];

        return view('ide-lab.index', compact(''));
    }
}
"#;
        let line = source
            .lines()
            .position(|line| line.contains("compact('')"))
            .expect("compact line should exist");
        let line_text = source.lines().nth(line).expect("line should exist");
        let character = line_text
            .find("''")
            .expect("empty compact token should exist")
            + 1;
        let context = crate::lsp::context::detect_view_data_context(
            "file:///tmp/app/Http/Controllers/DemoController.php",
            source,
            line,
            character,
        )
        .expect("view data context");

        let items = complete_view_data_variables(source, &context, line);
        let labels = items
            .iter()
            .filter_map(|item| item.get("label").and_then(|value| value.as_str()))
            .collect::<Vec<_>>();

        assert!(labels.contains(&"currentUser"));
        assert!(labels.contains(&"pageTitle"));
        assert!(labels.contains(&"examples"));
        assert!(labels.contains(&"breadcrumbs"));
        assert!(labels.contains(&"requestFilters"));
    }

    #[test]
    fn completes_indexed_view_variables_inside_blade_echo_and_php_blocks() {
        let project = view_project();
        let index = ProjectIndex::build_with_overrides(&project, &FileOverrides::default())
            .expect("index should build");
        let file = Path::new("resources/views/admin/users/index.blade.php");
        let source = "<div>\n    {{ $ }}\n    @php\n        $he\n    @endphp\n</div>\n";

        let echo_line = source
            .lines()
            .position(|line| line.contains("{{ $ }}"))
            .expect("echo line should exist");
        let echo_text = source.lines().nth(echo_line).expect("line should exist");
        let echo_character = echo_text.find("$ ").expect("dollar should exist") + 1;
        let echo_context = detect_blade_variable_context(
            "file:///tmp/resources/views/admin/users/index.blade.php",
            source,
            echo_line,
            echo_character,
        )
        .expect("blade echo variable context");

        let echo_items = complete_blade_view_variables(&index, file, &echo_context, echo_line);
        let echo_labels = echo_items
            .iter()
            .filter_map(|item| item.get("label").and_then(|value| value.as_str()))
            .collect::<Vec<_>>();

        assert!(echo_labels.contains(&"$headline"));
        assert!(echo_labels.contains(&"$cta"));
        assert!(echo_labels.contains(&"$version"));

        let php_line = source
            .lines()
            .position(|line| line.contains("$he"))
            .expect("php line should exist");
        let php_text = source.lines().nth(php_line).expect("line should exist");
        let php_character = php_text.find("$he").expect("prefix should exist") + "$he".len();
        let php_context = detect_blade_variable_context(
            "file:///tmp/resources/views/admin/users/index.blade.php",
            source,
            php_line,
            php_character,
        )
        .expect("blade php variable context");

        let php_items = complete_blade_view_variables(&index, file, &php_context, php_line);
        let php_labels = php_items
            .iter()
            .filter_map(|item| item.get("label").and_then(|value| value.as_str()))
            .collect::<Vec<_>>();

        assert!(php_labels.contains(&"$headline"));
        assert!(!php_labels.contains(&"$cta"));
    }
}
