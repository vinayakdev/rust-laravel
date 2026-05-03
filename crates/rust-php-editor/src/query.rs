use serde_json::{Value, json};
use std::cmp::Reverse;
use std::fs;
use std::path::{Path, PathBuf};

use super::context::{
    BladeComponentAttrContext, BladeComponentTagContext, BladeModelPropertyContext,
    BladeVariableContext, BuilderArgContext, BuilderRelationHoverContext, ForeachAliasContext,
    HelperContext, HelperStyle, LivewireComponentTagContext, LivewireDirectiveValueContext,
    LivewireDirectiveValueKind, ModelPropertyArrayContext, RouteActionContext, RouteActionKind,
    SymbolContext, SymbolKind, VendorChainContext, VendorMakeContext, ViewDataContext, ViewDataKind,
    builder_method_uses_relation_name,
};
use super::index::ProjectIndex;
use super::index::fuzzy_score;
use crate::types::{
    BladeComponentEntry, ConfigItem, ControllerEntry, ControllerMethodEntry, EnvItem,
    LivewireActionEntry, LivewireComponentEntry, PublicAssetEntry, RouteEntry, ViewEntry,
    ViewVariable,
};
use rust_php_markdown::{
    DocBundle, MarkdownDoc,
    asset::{self, AssetHoverInput},
    blade::{self, BladeComponentHoverInput, LivewireComponentHoverInput},
    config, controller, env, route, view,
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
        SymbolKind::Livewire => index
            .livewire_component_matches(&context.prefix)
            .into_iter()
            .map(|component| {
                livewire_symbol_completion(component, context, line, &index.project_root)
            })
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
    let mut items: Vec<Value> = index
        .blade_variables_for_file(file, &context.prefix)
        .into_iter()
        .map(|variable| blade_view_variable_completion(variable, context, line))
        .collect();

    for var_name in &context.foreach_vars {
        let prefix_lower = context.prefix.to_lowercase();
        if !context.prefix.is_empty() && !var_name.to_lowercase().contains(&prefix_lower) {
            continue;
        }
        items.push(json!({
            "label": var_name,
            "filterText": var_name,
            "kind": 6,
            "detail": "Foreach loop variable",
            "insertTextFormat": 2,
            "documentation": {
                "kind": "markdown",
                "value": format!("`${}` — foreach loop variable", var_name),
            },
            "textEdit": {
                "range": {
                    "start": { "line": line, "character": context.start_character },
                    "end": { "line": line, "character": context.end_character },
                },
                "newText": var_name,
            }
        }));
    }

    items
}

pub fn complete_blade_model_properties(
    index: &ProjectIndex,
    file: &Path,
    context: &BladeModelPropertyContext,
    line: usize,
) -> Vec<Value> {
    if context.variable_name == "loop" {
        return complete_loop_variable_properties(context, line);
    }

    let class_name = match index.blade_variable_class_for_file(file, &context.variable_name) {
        Some(c) => c,
        None => return Vec::new(),
    };

    if matches!(
        class_name.as_str(),
        "LengthAwarePaginator" | "Paginator" | "CursorPaginator"
    ) {
        return complete_paginator_methods(context, &class_name, line);
    }

    let Some(model) = index.model_for_class(&class_name) else {
        return Vec::new();
    };

    let prefix = &context.prefix;
    enum Kind {
        Column(String),
        Relation(String),
        Accessor,
        Append,
        Scope,
        Method,
    }
    let mut candidates: Vec<(u32, usize, String, Kind)> = Vec::new();

    for col in &model.columns {
        if let Some(score) = fuzzy_score(&col.name, prefix) {
            let detail = format!(
                "{} {}",
                col.column_type,
                if col.nullable { "nullable" } else { "" }
            )
            .trim()
            .to_string();
            candidates.push((
                score,
                col.name.len(),
                col.name.clone(),
                Kind::Column(detail),
            ));
        }
    }

    for rel in &model.relations {
        if let Some(score) = fuzzy_score(&rel.method, prefix) {
            let detail = format!("{} → {}", rel.relation_type, rel.related_model);
            candidates.push((
                score,
                rel.method.len(),
                rel.method.clone(),
                Kind::Relation(detail),
            ));
        }
    }

    for name in &model.accessors {
        if let Some(score) = fuzzy_score(name, prefix) {
            candidates.push((score, name.len(), name.clone(), Kind::Accessor));
        }
    }

    for name in &model.appends {
        if let Some(score) = fuzzy_score(name, prefix) {
            candidates.push((score, name.len(), name.clone(), Kind::Append));
        }
    }

    for name in &model.scopes {
        if let Some(score) = fuzzy_score(name, prefix) {
            candidates.push((score, name.len(), name.clone(), Kind::Scope));
        }
    }

    for name in &model.methods {
        if let Some(score) = fuzzy_score(name, prefix) {
            candidates.push((score, name.len(), name.clone(), Kind::Method));
        }
    }

    candidates.sort_by_key(|(score, len, label, _)| (Reverse(*score), *len, label.clone()));
    candidates.dedup_by(|a, b| a.2 == b.2);
    candidates
        .into_iter()
        .map(|(_, _, name, kind)| {
            let (item_kind, detail) = match kind {
                Kind::Column(d) => (5u8, d),
                Kind::Relation(d) => (18, d),
                Kind::Accessor => (5, "accessor".to_string()),
                Kind::Append => (5, "appended attribute".to_string()),
                Kind::Scope => (3, "scope".to_string()),
                Kind::Method => (2, "method".to_string()),
            };
            json!({
                "label": name,
                "kind": item_kind,
                "detail": detail,
                "textEdit": {
                    "range": {
                        "start": { "line": line, "character": context.start_character },
                        "end":   { "line": line, "character": context.end_character }
                    },
                    "newText": name
                }
            })
        })
        .collect()
}

fn complete_loop_variable_properties(
    context: &BladeModelPropertyContext,
    line: usize,
) -> Vec<Value> {
    struct LoopProp {
        name: &'static str,
        ty: &'static str,
        doc: &'static str,
    }
    const PROPS: &[LoopProp] = &[
        LoopProp {
            name: "index",
            ty: "int",
            doc: "Zero-based index of the current iteration.",
        },
        LoopProp {
            name: "iteration",
            ty: "int",
            doc: "One-based iteration count (starts at 1).",
        },
        LoopProp {
            name: "remaining",
            ty: "int",
            doc: "Iterations remaining in the loop.",
        },
        LoopProp {
            name: "count",
            ty: "int",
            doc: "Total number of items in the collection.",
        },
        LoopProp {
            name: "first",
            ty: "bool",
            doc: "Whether this is the first iteration.",
        },
        LoopProp {
            name: "last",
            ty: "bool",
            doc: "Whether this is the last iteration.",
        },
        LoopProp {
            name: "even",
            ty: "bool",
            doc: "Whether this is an even-numbered iteration.",
        },
        LoopProp {
            name: "odd",
            ty: "bool",
            doc: "Whether this is an odd-numbered iteration.",
        },
        LoopProp {
            name: "depth",
            ty: "int",
            doc: "Nesting level of the current loop (1 = outermost).",
        },
        LoopProp {
            name: "parent",
            ty: "?Loop",
            doc: "The parent loop's `$loop` variable when nested.",
        },
    ];
    PROPS
        .iter()
        .filter(|p| context.prefix.is_empty() || p.name.contains(context.prefix.as_str()))
        .map(|p| {
            json!({
                "label": p.name,
                "kind": 5,
                "detail": format!("{} — $loop", p.ty),
                "documentation": { "kind": "markdown", "value": p.doc },
                "textEdit": {
                    "range": {
                        "start": { "line": line, "character": context.start_character },
                        "end":   { "line": line, "character": context.end_character },
                    },
                    "newText": p.name,
                }
            })
        })
        .collect()
}

fn complete_paginator_methods(
    context: &BladeModelPropertyContext,
    class_name: &str,
    line: usize,
) -> Vec<Value> {
    struct PaginatorMethod {
        name: &'static str,
        insert: &'static str,
        detail: &'static str,
        doc: &'static str,
        all_paginators: bool,
    }
    const METHODS: &[PaginatorMethod] = &[
        PaginatorMethod {
            name: "links()",
            insert: "links()",
            detail: "string",
            doc: "Render the pagination links HTML.",
            all_paginators: true,
        },
        PaginatorMethod {
            name: "withQueryString()",
            insert: "withQueryString()",
            detail: "static",
            doc: "Append the current query string to pagination links.",
            all_paginators: true,
        },
        PaginatorMethod {
            name: "appends()",
            insert: "appends([${1:key} => ${2:val}])",
            detail: "static",
            doc: "Append key/value pairs to the pagination query string.",
            all_paginators: true,
        },
        PaginatorMethod {
            name: "onEachSide()",
            insert: "onEachSide(${1:3})",
            detail: "static",
            doc: "Number of links to show on each side of the current page.",
            all_paginators: true,
        },
        PaginatorMethod {
            name: "count()",
            insert: "count()",
            detail: "int",
            doc: "Number of items on the current page.",
            all_paginators: true,
        },
        PaginatorMethod {
            name: "perPage()",
            insert: "perPage()",
            detail: "int",
            doc: "Number of items per page.",
            all_paginators: true,
        },
        PaginatorMethod {
            name: "currentPage()",
            insert: "currentPage()",
            detail: "int",
            doc: "Current page number.",
            all_paginators: true,
        },
        PaginatorMethod {
            name: "hasPages()",
            insert: "hasPages()",
            detail: "bool",
            doc: "Whether there are multiple pages.",
            all_paginators: true,
        },
        PaginatorMethod {
            name: "hasMorePages()",
            insert: "hasMorePages()",
            detail: "bool",
            doc: "Whether there are more pages after the current one.",
            all_paginators: true,
        },
        PaginatorMethod {
            name: "nextPageUrl()",
            insert: "nextPageUrl()",
            detail: "?string",
            doc: "URL of the next page, or null.",
            all_paginators: true,
        },
        PaginatorMethod {
            name: "previousPageUrl()",
            insert: "previousPageUrl()",
            detail: "?string",
            doc: "URL of the previous page, or null.",
            all_paginators: true,
        },
        PaginatorMethod {
            name: "url()",
            insert: "url(${1:\\$page})",
            detail: "string",
            doc: "URL for the given page number.",
            all_paginators: true,
        },
        PaginatorMethod {
            name: "items()",
            insert: "items()",
            detail: "array",
            doc: "Items on the current page.",
            all_paginators: true,
        },
        PaginatorMethod {
            name: "isEmpty()",
            insert: "isEmpty()",
            detail: "bool",
            doc: "Whether the result set is empty.",
            all_paginators: true,
        },
        PaginatorMethod {
            name: "isNotEmpty()",
            insert: "isNotEmpty()",
            detail: "bool",
            doc: "Whether the result set is not empty.",
            all_paginators: true,
        },
        PaginatorMethod {
            name: "total()",
            insert: "total()",
            detail: "int",
            doc: "Total number of matching items (not available on SimplePaginator/CursorPaginator).",
            all_paginators: false,
        },
        PaginatorMethod {
            name: "lastPage()",
            insert: "lastPage()",
            detail: "int",
            doc: "Last available page number (not available on SimplePaginator/CursorPaginator).",
            all_paginators: false,
        },
    ];
    let is_length_aware = class_name == "LengthAwarePaginator";
    METHODS
        .iter()
        .filter(|m| {
            (m.all_paginators || is_length_aware)
                && (context.prefix.is_empty() || m.name.contains(context.prefix.as_str()))
        })
        .map(|m| {
            json!({
                "label": m.name,
                "kind": 2,
                "detail": format!("{} — {}", m.detail, class_name),
                "insertTextFormat": 2,
                "documentation": { "kind": "markdown", "value": m.doc },
                "textEdit": {
                    "range": {
                        "start": { "line": line, "character": context.start_character },
                        "end":   { "line": line, "character": context.end_character },
                    },
                    "newText": m.insert,
                }
            })
        })
        .collect()
}

pub fn complete_foreach_alias(context: &ForeachAliasContext, line: usize) -> Vec<Value> {
    let matches = context.prefix.is_empty()
        || context.suggestion.starts_with(&context.prefix)
        || context.suggestion.contains(&context.prefix);
    if !matches {
        return Vec::new();
    }
    vec![json!({
        "label": context.suggestion,
        "kind": 6,
        "detail": "Singular loop variable",
        "insertTextFormat": 2,
        "filterText": context.suggestion,
        "textEdit": {
            "range": {
                "start": { "line": line, "character": context.start_character },
                "end": { "line": line, "character": context.end_character },
            },
            "newText": context.suggestion,
        },
        "documentation": {
            "kind": "markdown",
            "value": format!("Singular form of `${}`", context.collection_name),
        },
    })]
}

pub fn helper_snippets(context: &HelperContext, line: usize) -> Vec<Value> {
    let mut items: Vec<Value> = ranked_helper_specs(&context.prefix)
        .into_iter()
        .map(|helper| helper_completion(helper, context, line))
        .collect();

    if matches!(context.style, HelperStyle::Php) {
        items.extend(
            ranked_php_snippet_specs(&context.prefix)
                .into_iter()
                .map(|spec| php_snippet_completion(spec, context, line)),
        );
    }

    items
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

pub fn complete_livewire_components(
    index: &ProjectIndex,
    context: &LivewireComponentTagContext,
    line: usize,
) -> Vec<Value> {
    index
        .livewire_component_matches(&context.prefix)
        .into_iter()
        .map(|component| livewire_tag_completion(component, context, line, &index.project_root))
        .collect()
}

pub fn complete_livewire_directive_values(
    index: &ProjectIndex,
    file: &Path,
    context: &LivewireDirectiveValueContext,
    line: usize,
) -> Vec<Value> {
    match context.kind {
        LivewireDirectiveValueKind::Property => index
            .livewire_state_for_file(file, &context.prefix)
            .into_iter()
            .map(|variable| livewire_property_completion(variable, context, line))
            .collect(),
        LivewireDirectiveValueKind::Action => index
            .livewire_actions_for_file(file, &context.prefix)
            .into_iter()
            .map(|action| livewire_action_completion(action, context, line))
            .collect(),
    }
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
    let component = components[0];
    Some(json!({
        "contents": { "kind": "markdown", "value": blade_component_hover_text(component, &index.project_root) },
        "range": range,
    }))
}

pub fn blade_component_definitions(
    index: &ProjectIndex,
    context: &BladeComponentTagContext,
    line: usize,
) -> Vec<Value> {
    let Some(component) = index
        .blade_component_definitions(&context.full_text)
        .into_iter()
        .next()
    else {
        return vec![];
    };
    let Some(file) = component
        .class_file
        .as_ref()
        .or(component.view_file.as_ref())
    else {
        return vec![];
    };
    vec![json!({
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
    })]
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
        SymbolKind::View => {
            if let Some(component) = index.livewire_component_for_view_name(&context.full_text) {
                if let Some(view_file) = component.view_file.as_ref() {
                    return vec![location_link(
                        &index.project_root,
                        view_file,
                        1,
                        1,
                        line,
                        context.start_character,
                        context.end_character,
                    )];
                }
                return livewire_component_locations(
                    index,
                    &component.component,
                    line,
                    context.start_character,
                    context.end_character,
                );
            }
            index
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
                .collect()
        }
        SymbolKind::Livewire => livewire_component_locations(
            index,
            &context.full_text,
            line,
            context.start_character,
            context.end_character,
        ),
        SymbolKind::Asset => asset_location(index, &context.full_text)
            .into_iter()
            .map(|relative| location(&index.project_root, &relative, 1, 1))
            .collect(),
    }
}

pub fn livewire_component_definitions(
    index: &ProjectIndex,
    context: &LivewireComponentTagContext,
    line: usize,
) -> Vec<Value> {
    livewire_component_locations(
        index,
        &context.full_text,
        line,
        livewire_component_selection_start_character(context),
        context.end_character,
    )
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
            if let Some(component) = index.livewire_component_for_view_name(&context.full_text) {
                return Some(livewire_symbol_hover_from_component(
                    component,
                    &index.project_root,
                    range,
                ));
            }
            let view = index
                .view_definitions(&context.full_text)
                .into_iter()
                .next()?;
            Some(json!({
                "contents": { "kind": "markdown", "value": view_hover(index, view) },
                "range": range,
            }))
        }
        SymbolKind::Livewire => livewire_symbol_hover(index, &context.full_text, range),
        SymbolKind::Asset => Some(json!({
            "contents": { "kind": "markdown", "value": asset_hover(index, &context.full_text) },
            "range": range,
        })),
    }
}

pub fn livewire_component_hover(
    index: &ProjectIndex,
    context: &LivewireComponentTagContext,
    line: usize,
) -> Option<Value> {
    let range = hover_range(
        line,
        livewire_component_selection_start_character(context),
        context.end_character,
    );
    livewire_symbol_hover(index, &context.full_text, range)
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

pub fn blade_component_create_actions(
    index: &ProjectIndex,
    context: &BladeComponentTagContext,
) -> Vec<Value> {
    let name = &context.full_text;
    if name.is_empty() || !index.blade_component_definitions(name).is_empty() {
        return Vec::new();
    }

    let root = &index.project_root;

    // dots → directory separators, kebab segments kept as-is
    let rel_path: std::path::PathBuf = name.split('.').collect();
    let view_rel = std::path::Path::new("resources/views/components")
        .join(&rel_path)
        .with_extension("blade.php");
    let view_abs = root.join(&view_rel);
    let view_uri = path_to_file_uri(&view_abs);

    // Convert kebab-case segment to PascalCase
    let pascal = |seg: &str| -> String {
        seg.split('-')
            .map(|w| {
                let mut c = w.chars();
                match c.next() {
                    None => String::new(),
                    Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
                }
            })
            .collect()
    };

    let class_segments: Vec<String> = name.split('.').map(|seg| pascal(seg)).collect();
    let class_rel = std::path::Path::new("app/View/Components")
        .join(class_segments.iter().collect::<std::path::PathBuf>())
        .with_extension("php");
    let class_abs = root.join(&class_rel);
    let class_uri = path_to_file_uri(&class_abs);

    let short_class = class_segments.last().cloned().unwrap_or_default();
    let namespace = if class_segments.len() > 1 {
        format!(
            "App\\View\\Components\\{}",
            class_segments[..class_segments.len() - 1].join("\\")
        )
    } else {
        "App\\View\\Components".to_string()
    };
    let view_name = format!("components.{name}");

    let anon_content = if context.self_closing {
        "<div>\n</div>\n".to_string()
    } else {
        "<div>\n    {{ $slot }}\n</div>\n".to_string()
    };

    let class_php = format!(
        "<?php\n\nnamespace {namespace};\n\nuse Illuminate\\View\\Component;\n\nclass {short_class} extends Component\n{{\n    public function render()\n    {{\n        return view('{view_name}');\n    }}\n}}\n"
    );
    let class_blade = if context.self_closing {
        "<div>\n</div>\n".to_string()
    } else {
        "<div>\n    {{ $slot }}\n</div>\n".to_string()
    };

    let mut actions = vec![
        json!({
            "title": format!("Create anonymous component <x-{name} />"),
            "kind": "quickfix",
            "edit": {
                "documentChanges": [
                    { "kind": "create", "uri": view_uri, "options": { "ignoreIfExists": true } },
                    {
                        "textDocument": { "uri": view_uri, "version": null },
                        "edits": [{
                            "range": { "start": { "line": 0, "character": 0 }, "end": { "line": 0, "character": 0 } },
                            "newText": anon_content
                        }]
                    }
                ]
            }
        }),
        json!({
            "title": format!("Create class component <x-{name} />"),
            "kind": "quickfix",
            "edit": {
                "documentChanges": [
                    { "kind": "create", "uri": class_uri, "options": { "ignoreIfExists": true } },
                    {
                        "textDocument": { "uri": class_uri, "version": null },
                        "edits": [{
                            "range": { "start": { "line": 0, "character": 0 }, "end": { "line": 0, "character": 0 } },
                            "newText": class_php
                        }]
                    },
                    { "kind": "create", "uri": view_uri, "options": { "ignoreIfExists": true } },
                    {
                        "textDocument": { "uri": view_uri, "version": null },
                        "edits": [{
                            "range": { "start": { "line": 0, "character": 0 }, "end": { "line": 0, "character": 0 } },
                            "newText": class_blade
                        }]
                    }
                ]
            }
        }),
    ];

    // Mark anonymous as preferred when there's no existing class component convention
    if let Some(first) = actions.first_mut() {
        if let Some(obj) = first.as_object_mut() {
            obj.insert("isPreferred".to_string(), json!(true));
        }
    }

    actions
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
    let docs = config::build(config::ConfigHoverInput {
        key: item.key.clone(),
        current_value: current_value.map(ToOwned::to_owned),
        env_key: item.env_key.clone(),
        default_value: item.default_value.clone(),
        env_value: item.env_value.clone(),
        detail: summary.clone(),
    });

    json!({
        "label": item.key,
        "kind": 10,
        "detail": docs.detail.clone().or(summary),
        "documentation": {
            "kind": "markdown",
            "value": docs.completion_markdown(),
        },
        "textEdit": replacement_edit(context, line, &item.key),
    })
}

fn route_completion(
    index: &ProjectIndex,
    route: &RouteEntry,
    context: &SymbolContext,
    line: usize,
) -> Value {
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

fn env_completion(
    index: &ProjectIndex,
    item: &EnvItem,
    context: &SymbolContext,
    line: usize,
) -> Value {
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

fn asset_completion(
    index: &ProjectIndex,
    asset: &PublicAssetEntry,
    context: &SymbolContext,
    line: usize,
) -> Value {
    let docs = asset_docs_from_entry(index, asset);
    json!({
        "label": asset.asset_path,
        "kind": 17,
        "detail": docs.detail.clone().unwrap_or_default(),
        "filterText": asset.asset_path,
        "documentation": {
            "kind": "markdown",
            "value": asset_completion_hover(index, asset),
        },
        "textEdit": replacement_edit(context, line, &asset.asset_path),
    })
}

fn helper_completion(helper: &HelperSpec, context: &HelperContext, line: usize) -> Value {
    let doc = MarkdownDoc::new()
        .title(helper.name)
        .blank()
        .separator()
        .blank()
        .line(helper.documentation);
    let mut item = json!({
        "label": helper.name,
        "kind": 3,
        "detail": helper.detail,
        "insertTextFormat": 2,
        "filterText": helper.name,
        "documentation": {
            "kind": "markdown",
            "value": doc.finish_markdown(),
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
    let doc = MarkdownDoc::new()
        .title(name)
        .blank()
        .separator()
        .blank()
        .line("Available as a local variable in the current controller method");
    json!({
        "label": name,
        "kind": 6,
        "detail": "Local variable in current function",
        "documentation": {
            "kind": "markdown",
            "value": doc.finish_markdown(),
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
    let doc = MarkdownDoc::new()
        .title(format!("${}", variable.name))
        .blank()
        .separator()
        .blank()
        .line("Available in the current Blade view");
    json!({
        "label": format!("${}", variable.name),
        "filterText": variable.name,
        "kind": 6,
        "detail": detail,
        "documentation": {
            "kind": "markdown",
            "value": doc.finish_markdown(),
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
        source_uri: Some(path_to_file_uri(
            &index.project_root.join(&method.declared_in),
        )),
        line: method.line,
        detail: Some(format!(
            "{} {}",
            controller.class_name, method.accessibility
        )),
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

fn view_completion(
    index: &ProjectIndex,
    view: &ViewEntry,
    context: &SymbolContext,
    line: usize,
) -> Value {
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

fn livewire_symbol_completion(
    component: &LivewireComponentEntry,
    context: &SymbolContext,
    line: usize,
    project_root: &Path,
) -> Value {
    json!({
        "label": component.component,
        "kind": 7,
        "detail": livewire_component_detail(component),
        "documentation": {
            "kind": "markdown",
            "value": livewire_component_hover_text(component, project_root),
        },
        "textEdit": replacement_edit(context, line, &component.component),
    })
}

fn livewire_tag_completion(
    component: &LivewireComponentEntry,
    context: &LivewireComponentTagContext,
    line: usize,
    project_root: &Path,
) -> Value {
    json!({
        "label": format!("livewire:{}", component.component),
        "kind": 7,
        "detail": livewire_component_detail(component),
        "filterText": component.component,
        "documentation": {
            "kind": "markdown",
            "value": livewire_component_hover_text(component, project_root),
        },
        "textEdit": {
            "range": {
                "start": { "line": line, "character": context.start_character },
                "end": { "line": line, "character": context.end_character },
            },
            "newText": component.component,
        }
    })
}

fn livewire_property_completion(
    variable: &ViewVariable,
    context: &LivewireDirectiveValueContext,
    line: usize,
) -> Value {
    let detail = variable
        .default_value
        .as_ref()
        .map(|value| format!("Livewire property = {value}"))
        .unwrap_or_else(|| "Livewire property".to_string());

    json!({
        "label": variable.name,
        "kind": 6,
        "detail": detail,
        "documentation": {
            "kind": "markdown",
            "value": format!("`${}`\n- available on current Livewire component", variable.name),
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

fn livewire_action_completion(
    action: &LivewireActionEntry,
    context: &LivewireDirectiveValueContext,
    line: usize,
) -> Value {
    json!({
        "label": action.name,
        "kind": 2,
        "detail": format!("Livewire action · {}", context.directive),
        "documentation": {
            "kind": "markdown",
            "value": format!("`{}`\n- public method on current Livewire component", action.name),
        },
        "textEdit": {
            "range": {
                "start": { "line": line, "character": context.start_character },
                "end": { "line": line, "character": context.end_character },
            },
            "newText": action.name,
        }
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

fn livewire_component_selection_start_character(context: &LivewireComponentTagContext) -> usize {
    context.tag_start_character + 1
}

fn livewire_component_detail(component: &LivewireComponentEntry) -> String {
    component
        .class_name
        .as_ref()
        .map(|class| format!("{class} ({})", component.kind))
        .unwrap_or_else(|| format!("Livewire component ({})", component.kind))
}

fn livewire_component_hover_text(
    component: &LivewireComponentEntry,
    project_root: &Path,
) -> String {
    let class_uri = component
        .class_file
        .as_ref()
        .map(|f| path_to_file_uri(&project_root.join(f)));
    let view_file_uri = component
        .view_file
        .as_ref()
        .map(|f| path_to_file_uri(&project_root.join(f)));
    blade::build_livewire(LivewireComponentHoverInput {
        component: component.component.clone(),
        class_name: component.class_name.clone(),
        class_uri,
        view_name: component.view_name.clone(),
        view_file: component
            .view_file
            .as_ref()
            .map(|f| f.display().to_string()),
        view_file_uri,
        properties: component
            .state
            .iter()
            .map(|s| match &s.default_value {
                Some(default) => format!("{} = {}", s.name, default),
                None => s.name.clone(),
            })
            .collect(),
        actions: component.actions.iter().map(|a| a.name.clone()).collect(),
        detail: None,
    })
    .hover_markdown()
}

fn livewire_symbol_hover(index: &ProjectIndex, name: &str, range: Value) -> Option<Value> {
    let component = index
        .livewire_component_definitions(name)
        .into_iter()
        .next()?;
    Some(livewire_symbol_hover_from_component(
        component,
        &index.project_root,
        range,
    ))
}

fn livewire_symbol_hover_from_component(
    component: &LivewireComponentEntry,
    project_root: &Path,
    range: Value,
) -> Value {
    json!({
        "contents": { "kind": "markdown", "value": livewire_component_hover_text(component, project_root) },
        "range": range,
    })
}

fn livewire_component_locations(
    index: &ProjectIndex,
    name: &str,
    line: usize,
    start_character: usize,
    end_character: usize,
) -> Vec<Value> {
    let Some(component) = index
        .livewire_component_definitions(name)
        .into_iter()
        .next()
    else {
        return vec![];
    };
    let Some(file) = component
        .class_file
        .as_ref()
        .or(component.view_file.as_ref())
    else {
        return vec![];
    };
    vec![location_link(
        &index.project_root,
        file,
        component.source.line,
        component.source.column,
        line,
        start_character,
        end_character,
    )]
}

fn blade_component_tag_completion(
    component: &BladeComponentEntry,
    index: &ProjectIndex,
    context: &BladeComponentTagContext,
    line: usize,
) -> Value {
    let name = &component.component;
    let has_slot = component_uses_slot(index, component);

    let name_part = if context.has_x_dash {
        name.clone()
    } else {
        format!("x-{name}")
    };

    let new_text = if has_slot {
        format!("{name_part} $1>\n\t$2\n</x-{name}>$0")
    } else {
        format!("{name_part} $1/>")
    };

    json!({
        "label": format!("x-{name}"),
        "kind": 10,
        "detail": blade_component_detail(component),
        "filterText": name,
        "insertTextFormat": 2,
        "documentation": { "kind": "markdown", "value": blade_component_hover_text(component, &index.project_root) },
        "textEdit": {
            "range": {
                "start": { "line": line, "character": context.start_character },
                "end": { "line": line, "character": context.end_character },
            },
            "newText": new_text,
        }
    })
}

fn component_uses_slot(index: &ProjectIndex, component: &BladeComponentEntry) -> bool {
    let file = match &component.view_file {
        Some(f) => f,
        None => return false,
    };
    let path = index.project_root.join(file);
    let content = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(_) => return false,
    };
    // Check for default slot usage: $slot followed by a non-identifier char
    let bytes = content.as_bytes();
    let needle = b"$slot";
    let mut i = 0;
    while i + needle.len() <= bytes.len() {
        if bytes[i..i + needle.len()] == *needle {
            let after = bytes.get(i + needle.len()).copied().unwrap_or(0);
            if !after.is_ascii_alphanumeric() && after != b'_' {
                return true;
            }
        }
        i += 1;
    }
    false
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
        Some(v) => MarkdownDoc::new()
            .title(name)
            .blank()
            .separator()
            .blank()
            .field("Default", v),
        None => MarkdownDoc::new()
            .title(name)
            .blank()
            .separator()
            .blank()
            .field("Status", "required"),
    };
    json!({
        "label": name,
        "kind": 5,
        "detail": detail,
        "documentation": { "kind": "markdown", "value": doc.finish_markdown() },
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
    let props = component
        .props
        .iter()
        .map(|p| match &p.default_value {
            Some(v) => format!("{} = {v}", p.name),
            None => p.name.clone(),
        })
        .collect();
    let class_uri = component
        .class_file
        .as_ref()
        .map(|f| path_to_file_uri(&project_root.join(f)));
    let view_file_uri = component
        .view_file
        .as_ref()
        .map(|f| path_to_file_uri(&project_root.join(f)));
    blade::build(BladeComponentHoverInput {
        component: component.component.clone(),
        class_name: component.class_name.clone(),
        class_uri,
        view_file: component
            .view_file
            .as_ref()
            .map(|f| f.display().to_string()),
        view_file_uri,
        props,
        detail: None,
    })
    .hover_markdown()
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
    config::build(config::ConfigHoverInput {
        key: item.key.clone(),
        current_value: config_current_value(item).map(ToOwned::to_owned),
        env_key: item.env_key.clone(),
        default_value: item.default_value.clone(),
        env_value: item.env_value.clone(),
        detail: None,
    })
    .hover_markdown()
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
        name: route
            .name
            .as_deref()
            .unwrap_or("<unnamed-route>")
            .to_string(),
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
        source_uri: Some(path_to_file_uri(
            &index.project_root.join(&method.declared_in),
        )),
        line: method.line,
        detail: None,
    })
    .hover_markdown()
}

fn asset_hover(index: &ProjectIndex, asset_path: &str) -> String {
    asset_docs(index, asset_path, false).hover_markdown()
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

fn asset_completion_detail_for_path(asset_path: &str, for_completion: bool) -> String {
    if for_completion {
        format!("public/{}", asset_path)
    } else {
        format!("public/{}", asset_path)
    }
}

fn format_size(bytes: u64) -> String {
    const KIB: u64 = 1024;
    const MIB: u64 = KIB * 1024;

    if bytes >= MIB {
        format!("{:.2} MiB", bytes as f64 / MIB as f64)
    } else if bytes >= KIB {
        format!("{:.2} KiB", bytes as f64 / KIB as f64)
    } else {
        format!("{} B", bytes)
    }
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

struct PhpSnippetSpec {
    name: &'static str,
    detail: &'static str,
    documentation: &'static str,
    body: &'static str,
}

fn php_snippet_specs() -> &'static [PhpSnippetSpec] {
    &[
        PhpSnippetSpec {
            name: "pubf",
            detail: "Public function",
            documentation: "Insert a `public function` stub.",
            body: "public function ${1:name}(${2}): ${3:void}\n{\n\t$0\n}",
        },
        PhpSnippetSpec {
            name: "prof",
            detail: "Protected function",
            documentation: "Insert a `protected function` stub.",
            body: "protected function ${1:name}(${2}): ${3:void}\n{\n\t$0\n}",
        },
        PhpSnippetSpec {
            name: "prif",
            detail: "Private function",
            documentation: "Insert a `private function` stub.",
            body: "private function ${1:name}(${2}): ${3:void}\n{\n\t$0\n}",
        },
        PhpSnippetSpec {
            name: "pubsf",
            detail: "Public static function",
            documentation: "Insert a `public static function` stub.",
            body: "public static function ${1:name}(${2}): ${3:void}\n{\n\t$0\n}",
        },
        PhpSnippetSpec {
            name: "prosf",
            detail: "Protected static function",
            documentation: "Insert a `protected static function` stub.",
            body: "protected static function ${1:name}(${2}): ${3:void}\n{\n\t$0\n}",
        },
        PhpSnippetSpec {
            name: "prisf",
            detail: "Private static function",
            documentation: "Insert a `private static function` stub.",
            body: "private static function ${1:name}(${2}): ${3:void}\n{\n\t$0\n}",
        },
        PhpSnippetSpec {
            name: "abstf",
            detail: "Abstract public function",
            documentation: "Insert an `abstract public function` signature.",
            body: "abstract public function ${1:name}(${2}): ${3:void};",
        },
        PhpSnippetSpec {
            name: "pub",
            detail: "Public function (inline)",
            documentation: "Insert a compact `public function` stub.",
            body: "public function $1($2)${3:: $4} {\n\t$5\n}",
        },
        PhpSnippetSpec {
            name: "priv",
            detail: "Private function (inline)",
            documentation: "Insert a compact `private function` stub.",
            body: "private function $1($2)${3:: $4} {\n\t$5\n}",
        },
        PhpSnippetSpec {
            name: "prot",
            detail: "Protected function (inline)",
            documentation: "Insert a compact `protected function` stub.",
            body: "protected function $1($2)${3:: $4} {\n\t$5\n}",
        },
        PhpSnippetSpec {
            name: "pubp",
            detail: "Public typed property",
            documentation: "Insert a `public` typed property declaration.",
            body: "public ${1:string} \\$$2 = ${3:''};",
        },
        PhpSnippetSpec {
            name: "prop",
            detail: "Protected typed property",
            documentation: "Insert a `protected` typed property declaration.",
            body: "protected ${1:string} \\$$2;",
        },
        PhpSnippetSpec {
            name: "prip",
            detail: "Private typed property",
            documentation: "Insert a `private` typed property declaration.",
            body: "private ${1:string} \\$$2;",
        },
        PhpSnippetSpec {
            name: "rdp",
            detail: "Public readonly property",
            documentation: "Insert a `public readonly` property declaration.",
            body: "public readonly ${1:string} \\$$2;",
        },
        PhpSnippetSpec {
            name: "pubsp",
            detail: "Public static property",
            documentation: "Insert a `public static` property declaration.",
            body: "public static ${1:string} \\$$2;",
        },
        PhpSnippetSpec {
            name: "inv",
            detail: "__invoke method",
            documentation: "Insert a `public function __invoke` stub.",
            body: "public function __invoke(${1}): ${2:mixed}\n{\n\t$0\n}",
        },
        PhpSnippetSpec {
            name: "tostr",
            detail: "__toString method",
            documentation: "Insert a `public function __toString` stub.",
            body: "public function __toString(): string\n{\n\treturn $0;\n}",
        },
        PhpSnippetSpec {
            name: "gets",
            detail: "__get magic method",
            documentation: "Insert a `public function __get` stub.",
            body: "public function __get(string \\$name): mixed\n{\n\t$0\n}",
        },
        PhpSnippetSpec {
            name: "sets",
            detail: "__set magic method",
            documentation: "Insert a `public function __set` stub.",
            body: "public function __set(string \\$name, mixed \\$value): void\n{\n\t$0\n}",
        },
        PhpSnippetSpec {
            name: "construct",
            detail: "Constructor method",
            documentation: "Insert a `public function __construct` stub.",
            body: "public function __construct($1)\n{\n\t$2\n}",
        },
        PhpSnippetSpec {
            name: "idx",
            detail: "Controller index action",
            documentation: "Insert a controller `index` action stub.",
            body: "public function index(): View\n{\n\treturn view('${1:}');\n}",
        },
        PhpSnippetSpec {
            name: "shw",
            detail: "Controller show action",
            documentation: "Insert a controller `show` action stub.",
            body: "public function show(${1:Model} \\$$2): View\n{\n\treturn view('${3:}', compact('$2'));\n}",
        },
        PhpSnippetSpec {
            name: "cre",
            detail: "Controller create action",
            documentation: "Insert a controller `create` action stub.",
            body: "public function create(): View\n{\n\treturn view('${1:}');\n}",
        },
        PhpSnippetSpec {
            name: "sto",
            detail: "Controller store action",
            documentation: "Insert a controller `store` action stub.",
            body: "public function store(${1:Request} \\$request): RedirectResponse\n{\n\t$0\n\n\treturn redirect()->route('${2:}');\n}",
        },
        PhpSnippetSpec {
            name: "edi",
            detail: "Controller edit action",
            documentation: "Insert a controller `edit` action stub.",
            body: "public function edit(${1:Model} \\$$2): View\n{\n\treturn view('${3:}', compact('$2'));\n}",
        },
        PhpSnippetSpec {
            name: "upd",
            detail: "Controller update action",
            documentation: "Insert a controller `update` action stub.",
            body: "public function update(${1:Request} \\$request, ${2:Model} \\$$3): RedirectResponse\n{\n\t$0\n\n\treturn redirect()->route('${4:}');\n}",
        },
        PhpSnippetSpec {
            name: "des",
            detail: "Controller destroy action",
            documentation: "Insert a controller `destroy` action stub.",
            body: "public function destroy(${1:Model} \\$$2): RedirectResponse\n{\n\t$2->delete();\n\n\treturn redirect()->route('${3:}');\n}",
        },
        PhpSnippetSpec {
            name: "afunc",
            detail: "Anonymous function",
            documentation: "Insert an anonymous/lambda function.",
            body: "function($1) {\n\t$2\n}",
        },
        PhpSnippetSpec {
            name: "arr",
            detail: "Array declaration",
            documentation: "Insert a short-form array `[...]`.",
            body: "[$1]",
        },
        // Class-level declarations
        PhpSnippetSpec {
            name: "class",
            detail: "PHP class definition",
            documentation: "Insert a `class` declaration.",
            body: "class $1 ${2:extends $3} ${4:implements $5} {\n\t$6\n}",
        },
        PhpSnippetSpec {
            name: "interface",
            detail: "PHP interface",
            documentation: "Insert an `interface` declaration.",
            body: "interface $1 ${2:extends $3} {\n\t$4\n}",
        },
        PhpSnippetSpec {
            name: "trait",
            detail: "PHP trait",
            documentation: "Insert a `trait` declaration.",
            body: "trait $1 {\n\t$2\n}",
        },
        PhpSnippetSpec {
            name: "enum",
            detail: "PHP 8.1 enum",
            documentation: "Insert an `enum` declaration.",
            body: "enum ${1:Name}${2:: ${3:string}}\n{\n\tcase ${4:Value} = ${5:'value'};\n}",
        },
        PhpSnippetSpec {
            name: "namespace",
            detail: "PHP namespace declaration",
            documentation: "Insert a `namespace` statement.",
            body: "namespace $1;",
        },
        PhpSnippetSpec {
            name: "use",
            detail: "PHP use statement",
            documentation: "Insert a `use` statement.",
            body: "use $1;",
        },
        // Control flow
        PhpSnippetSpec {
            name: "if",
            detail: "If statement",
            documentation: "Insert an `if` block.",
            body: "if ($1) {\n\t$2\n}",
        },
        PhpSnippetSpec {
            name: "ifelse",
            detail: "If-else statement",
            documentation: "Insert an `if/else` block.",
            body: "if ($1) {\n\t$2\n} else {\n\t$3\n}",
        },
        PhpSnippetSpec {
            name: "else",
            detail: "Else block",
            documentation: "Insert an `else` block.",
            body: "else {\n\t$1\n}",
        },
        PhpSnippetSpec {
            name: "elseif",
            detail: "Else-if block",
            documentation: "Insert an `elseif` block.",
            body: "elseif ($1) {\n\t$2\n}",
        },
        PhpSnippetSpec {
            name: "switch",
            detail: "Switch statement",
            documentation: "Insert a `switch` statement.",
            body: "switch ($1) {\n\tcase $2:\n\t\t$3\n\t\tbreak;\n\tdefault:\n\t\t$4\n}",
        },
        PhpSnippetSpec {
            name: "match",
            detail: "PHP match expression",
            documentation: "Insert a `match` expression.",
            body: "match (${1:\\$var}) {\n\t${2:value} => ${3:result},\n\tdefault => ${4:default},\n}",
        },
        PhpSnippetSpec {
            name: "foreach",
            detail: "Foreach loop",
            documentation: "Insert a `foreach` loop.",
            body: "foreach ($1 as $2) {\n\t$3\n}",
        },
        PhpSnippetSpec {
            name: "for",
            detail: "For loop",
            documentation: "Insert a `for` loop.",
            body: "for ($1 = 0; $1 < $2; $1++) {\n\t$3\n}",
        },
        PhpSnippetSpec {
            name: "while",
            detail: "While loop",
            documentation: "Insert a `while` loop.",
            body: "while ($1) {\n\t$2\n}",
        },
        PhpSnippetSpec {
            name: "try",
            detail: "Try-catch block",
            documentation: "Insert a `try/catch` block.",
            body: "try {\n\t$1\n} catch (${2:Exception} \\$e) {\n\t$3\n}",
        },
        PhpSnippetSpec {
            name: "tryf",
            detail: "Try-catch-finally block",
            documentation: "Insert a `try/catch/finally` block.",
            body: "try {\n\t$1\n} catch (${2:Exception} \\$e) {\n\t$3\n} finally {\n\t$4\n}",
        },
        // Expressions
        PhpSnippetSpec {
            name: "fn",
            detail: "PHP arrow function",
            documentation: "Insert an arrow function `fn(...) => ...`.",
            body: "fn(${1}) => ${2:$1}",
        },
        PhpSnippetSpec {
            name: "return",
            detail: "Return statement",
            documentation: "Insert a `return` statement.",
            body: "return $1;",
        },
        PhpSnippetSpec {
            name: "echo",
            detail: "Echo statement",
            documentation: "Insert an `echo` statement.",
            body: "echo $1;",
        },
    ]
}

fn ranked_php_snippet_specs(query: &str) -> Vec<&'static PhpSnippetSpec> {
    let mut matches = php_snippet_specs()
        .iter()
        .filter_map(|spec| {
            let score = fuzzy_score(spec.name, query)?;
            Some((score, spec.name.len(), spec.name, spec))
        })
        .collect::<Vec<_>>();

    matches.sort_by_key(|(score, len, label, _)| (Reverse(*score), *len, *label));
    matches.into_iter().map(|(_, _, _, spec)| spec).collect()
}

fn php_snippet_completion(spec: &PhpSnippetSpec, context: &HelperContext, line: usize) -> Value {
    let doc = MarkdownDoc::new()
        .title(spec.name)
        .blank()
        .separator()
        .blank()
        .line(spec.documentation);
    json!({
        "label": spec.name,
        "kind": 15,
        "detail": spec.detail,
        "insertTextFormat": 2,
        "filterText": spec.name,
        "documentation": {
            "kind": "markdown",
            "value": doc.finish_markdown(),
        },
        "textEdit": {
            "range": {
                "start": { "line": line, "character": context.start_character },
                "end": { "line": line, "character": context.end_character },
            },
            "newText": spec.body,
        }
    })
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
    // Search backward through nested `function` keywords until we find one whose body
    // actually contains the cursor. This handles closures/arrow-functions that appear
    // before the cursor but are already closed (e.g. ->each(static function(...) { })).
    let mut search_end = cursor.min(source.len());

    loop {
        let function_start = source[..search_end].rfind("function")?;
        let signature = &source[function_start..];
        let open_paren_rel = signature.find('(')?;
        let open_paren = function_start + open_paren_rel;
        let close_paren = find_matching_delimiter(source, open_paren, '(', ')')?;
        let body_start = source[close_paren..]
            .find('{')
            .map(|rel| close_paren + rel)?;
        let body_end = find_matching_delimiter(source, body_start, '{', '}')?;

        if cursor >= body_start + 1 && cursor <= body_end {
            return Some((function_start, body_start + 1, body_end));
        }

        // This function doesn't contain the cursor; skip past its keyword and try again.
        if function_start == 0 {
            return None;
        }
        search_end = function_start;
    }
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

pub fn complete_vendor_chain_methods(
    index: &ProjectIndex,
    context: &VendorChainContext,
    line: usize,
) -> Vec<Value> {
    let methods = index.vendor_chainable_methods(&context.class_fqn);
    let mut matches: Vec<(u32, usize, String)> = methods
        .into_iter()
        .filter(|m| !m.starts_with('_'))
        .filter_map(|m| {
            let score = fuzzy_score(&m, &context.prefix)?;
            let len = m.len();
            Some((score, len, m))
        })
        .collect();
    matches.sort_by_key(|(score, len, label)| (Reverse(*score), *len, label.clone()));
    matches.dedup_by(|a, b| a.2 == b.2);
    matches
        .into_iter()
        .map(|(_, _, name)| vendor_chain_method_completion(&name, context, line))
        .collect()
}

/// Walk upward from `current_file` (up to 4 directory levels) looking for a PHP file that:
/// - contains `$form_class_name` (references the form class by name), AND
/// - contains `$model = SomeModel::class`
/// Returns the model short name when found.
fn find_model_via_sibling_resource(
    current_file: Option<&std::path::Path>,
    form_class_name: Option<&str>,
    project_root: &std::path::Path,
) -> Option<String> {
    let file = current_file?;
    let form_class = form_class_name?;

    let mut dir = file.parent()?;
    for _ in 0..4 {
        if !dir.starts_with(project_root) {
            break;
        }
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) != Some("php") {
                    continue;
                }
                if path == file {
                    continue;
                }
                let Ok(content) = std::fs::read_to_string(&path) else {
                    continue;
                };
                if !content.contains(form_class) {
                    continue;
                }
                if let Some(model) = extract_model_class_from_text(&content) {
                    return Some(model);
                }
            }
        }
        dir = dir.parent()?;
    }
    None
}

fn extract_model_class_from_text(source: &str) -> Option<String> {
    for line in source.lines() {
        let t = line.trim();
        if t.contains("$model") && t.contains("::class") {
            let class_pos = t.rfind("::class")?;
            let before = &t[..class_pos];
            let name: String = before
                .chars()
                .rev()
                .take_while(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '\\')
                .collect::<String>()
                .chars()
                .rev()
                .collect();
            if !name.is_empty() {
                return Some(name);
            }
        }
    }
    None
}

fn vendor_chain_method_completion(name: &str, context: &VendorChainContext, line: usize) -> Value {
    json!({
        "label": name,
        "kind": 2,  // Method
        "insertTextFormat": 2,
        "textEdit": {
            "range": {
                "start": { "line": line, "character": context.start_character },
                "end":   { "line": line, "character": context.end_character }
            },
            "newText": format!("{name}($0)")
        }
    })
}

pub fn complete_vendor_make_columns(
    index: &ProjectIndex,
    context: &VendorMakeContext,
    line: usize,
) -> Vec<Value> {
    let resolved_model = context
        .model_class
        .as_deref()
        .map(str::to_string)
        .or_else(|| {
            find_model_via_sibling_resource(
                context.current_file.as_deref(),
                context.current_class_name.as_deref(),
                &index.project_root,
            )
        });
    let model_class = match resolved_model.as_deref() {
        Some(c) if !c.is_empty() => c,
        _ => return Vec::new(),
    };

    let Some(model) = index.model_for_class(model_class) else {
        return Vec::new();
    };

    let prefix = &context.prefix;

    // Dot-notation: user types `relation.col_prefix` — suggest related model's columns.
    if let Some(dot_pos) = prefix.find('.') {
        let relation_name = &prefix[..dot_pos];
        let col_prefix = &prefix[dot_pos + 1..];
        let Some(relation) = model.relations.iter().find(|r| r.method == relation_name) else {
            return Vec::new();
        };
        let Some(related_model) = index.model_for_class(&relation.related_model) else {
            return Vec::new();
        };
        let mut matches: Vec<(u32, usize, String)> = related_model
            .columns
            .iter()
            .filter_map(|col| {
                let score = fuzzy_score(&col.name, col_prefix)?;
                Some((score, col.name.len(), col.name.clone()))
            })
            .collect();
        matches.sort_by_key(|(score, len, label)| (Reverse(*score), *len, label.clone()));
        matches.dedup_by(|a, b| a.2 == b.2);
        return matches
            .into_iter()
            .map(|(_, _, col_name)| {
                let full_name = format!("{relation_name}.{col_name}");
                vendor_make_field_completion(
                    &full_name,
                    &col_name,
                    5,
                    &format!("{} column", relation.related_model),
                    context,
                    line,
                )
            })
            .collect();
    }

    // Flat suggestions: columns + relation names + scopes.
    enum Kind {
        Column(String),
        Relation(String, String),
        Scope,
    }
    let mut candidates: Vec<(u32, usize, String, Kind)> = Vec::new();

    for col in &model.columns {
        if let Some(score) = fuzzy_score(&col.name, prefix) {
            let detail = format!(
                "{} {}",
                col.column_type,
                if col.nullable { "nullable" } else { "" }
            )
            .trim()
            .to_string();
            candidates.push((
                score,
                col.name.len(),
                col.name.clone(),
                Kind::Column(detail),
            ));
        }
    }

    for rel in &model.relations {
        if let Some(score) = fuzzy_score(&rel.method, prefix) {
            let detail = format!("{} {}", rel.relation_type, rel.related_model);
            candidates.push((
                score,
                rel.method.len(),
                rel.method.clone(),
                Kind::Relation(detail, rel.related_model.clone()),
            ));
        }
    }

    for scope in &model.scopes {
        if let Some(score) = fuzzy_score(scope, prefix) {
            candidates.push((score, scope.len(), scope.clone(), Kind::Scope));
        }
    }

    candidates.sort_by_key(|(score, len, label, _)| (Reverse(*score), *len, label.clone()));
    candidates.dedup_by(|a, b| a.2 == b.2);
    candidates
        .into_iter()
        .map(|(_, _, name, kind)| match kind {
            Kind::Column(detail) => {
                vendor_make_field_completion(&name, &name, 5, &detail, context, line)
            }
            Kind::Relation(detail, _related) => {
                vendor_make_field_completion(&name, &name, 18, &detail, context, line)
            }
            Kind::Scope => vendor_make_field_completion(&name, &name, 3, "scope", context, line),
        })
        .collect()
}

fn vendor_make_field_completion(
    new_text: &str,
    label: &str,
    kind: u8,
    detail: &str,
    context: &VendorMakeContext,
    line: usize,
) -> Value {
    json!({
        "label": label,
        "kind": kind,
        "detail": detail,
        "textEdit": {
            "range": {
                "start": { "line": line, "character": context.start_character },
                "end":   { "line": line, "character": context.end_character }
            },
            "newText": new_text
        }
    })
}

pub fn complete_builder_arg_columns(
    index: &ProjectIndex,
    context: &BuilderArgContext,
    line: usize,
) -> Vec<Value> {
    let Some(model) = index.model_for_class(&context.model_class) else {
        return Vec::new();
    };

    if builder_method_uses_relation_name(&context.method_name) {
        return complete_builder_arg_relations(index, model, context, line);
    }

    let prefix = &context.prefix;
    let mut matches: Vec<(u32, usize, String, String)> = model
        .columns
        .iter()
        .filter_map(|col| {
            let score = fuzzy_score(&col.name, prefix)?;
            let detail = format!(
                "{} {}",
                col.column_type,
                if col.nullable { "nullable" } else { "" }
            )
            .trim()
            .to_string();
            Some((score, col.name.len(), col.name.clone(), detail))
        })
        .collect();

    matches.sort_by_key(|(score, len, label, _)| (Reverse(*score), *len, label.clone()));
    matches.dedup_by(|a, b| a.2 == b.2);
    matches
        .into_iter()
        .map(|(_, _, name, detail)| {
            let mut item = json!({
                "label": name,
                "kind": 5,  // Field
                "detail": detail,
                "textEdit": {
                    "range": {
                        "start": { "line": line, "character": context.start_character },
                        "end":   { "line": line, "character": context.end_character }
                    },
                    "newText": name
                }
            });
            item["command"] = retrigger_suggest_command();
            item
        })
        .collect()
}

/// Complete model column names inside a model instance method array (e.g. `->only([`, `->makeVisible([`).
pub fn complete_model_property_array(
    index: &ProjectIndex,
    context: &ModelPropertyArrayContext,
    line: usize,
) -> Vec<Value> {
    let Some(model) = index.model_for_class(&context.model_class) else {
        return Vec::new();
    };

    let prefix = &context.prefix;
    let mut matches: Vec<(u32, usize, String, String)> = model
        .columns
        .iter()
        .filter_map(|col| {
            let score = fuzzy_score(&col.name, prefix)?;
            let detail = format!(
                "{} {}",
                col.column_type,
                if col.nullable { "nullable" } else { "" }
            )
            .trim()
            .to_string();
            Some((score, col.name.len(), col.name.clone(), detail))
        })
        .collect();

    matches.sort_by_key(|(score, len, label, _)| (Reverse(*score), *len, label.clone()));
    matches.dedup_by(|a, b| a.2 == b.2);
    matches
        .into_iter()
        .map(|(_, _, name, detail)| {
            json!({
                "label": name,
                "kind": 5,
                "detail": detail,
                "textEdit": {
                    "range": {
                        "start": { "line": line, "character": context.start_character },
                        "end":   { "line": line, "character": context.end_character }
                    },
                    "newText": name
                }
            })
        })
        .collect()
}

/// Methods that accept `'relation as alias' => fn($q)` key-value pairs in their array argument.
const AGGREGATE_RELATION_METHODS: &[&str] = &[
    "withCount",
    "withSum",
    "withMin",
    "withMax",
    "withAvg",
    "withExists",
    "withAggregate",
    "loadCount",
    "loadSum",
    "loadMin",
    "loadMax",
    "loadAvg",
];

fn complete_builder_arg_relations(
    index: &ProjectIndex,
    model: &crate::types::ModelEntry,
    context: &BuilderArgContext,
    line: usize,
) -> Vec<Value> {
    let prefix = &context.prefix;

    if let Some((relation_path, selected_columns)) = prefix.rsplit_once(':') {
        let Some(target_model) = resolve_relation_path_model(index, model, relation_path) else {
            return Vec::new();
        };
        // Column sub-selection: never add trailing comma here, user is still building 'rel:col,col'
        return complete_builder_related_columns(
            target_model,
            relation_path,
            selected_columns,
            context,
            line,
            ':',
        );
    }

    let (base_path, segment_prefix, relation_model) =
        if let Some((path, fragment)) = prefix.rsplit_once('.') {
            let Some(target_model) = resolve_relation_path_model(index, model, path) else {
                return Vec::new();
            };
            (Some(path), fragment, target_model)
        } else {
            (None, prefix.as_str(), model)
        };

    enum Kind {
        Relation(String),
        Column(String),
    }
    let mut matches: Vec<(u32, usize, String, Kind)> = relation_model
        .relations
        .iter()
        .filter_map(|relation| {
            let score = fuzzy_score(&relation.method, segment_prefix)?;
            let detail = format!("{} {}", relation.relation_type, relation.related_model);
            Some((
                score,
                relation.method.len(),
                relation.method.clone(),
                Kind::Relation(detail),
            ))
        })
        .collect();

    if base_path.is_some() {
        matches.extend(relation_model.columns.iter().filter_map(|column| {
            let score = fuzzy_score(&column.name, segment_prefix)?;
            Some((
                score,
                column.name.len(),
                column.name.clone(),
                Kind::Column(column_detail(column)),
            ))
        }));
    }

    matches.sort_by_key(|(score, len, label, _)| (Reverse(*score), *len, label.clone()));
    matches.dedup_by(|a, b| a.2 == b.2);

    // Build the trailing-comma additionalTextEdit once, reused for each item.
    let trailing_comma_edit = if context.in_array && !context.has_trailing_comma {
        Some(json!([{
            "range": {
                "start": { "line": line, "character": context.quote_end_character },
                "end":   { "line": line, "character": context.quote_end_character }
            },
            "newText": ","
        }]))
    } else {
        None
    };

    let mut items: Vec<Value> = Vec::new();

    // Key-value snippet for aggregate methods in array context.
    if context.in_array && AGGREGATE_RELATION_METHODS.contains(&context.method_name.as_str()) {
        items.push(json!({
            "label": "'relation as alias' => fn",
            "kind": 15,
            "detail": "Constrained aggregate",
            "sortText": "0",
            "insertTextFormat": 2,
            "filterText": context.prefix,
            "textEdit": {
                "range": {
                    "start": { "line": line, "character": context.quote_start_character },
                    "end":   { "line": line, "character": context.quote_end_character }
                },
                "newText": "'${1:relation} as ${2:alias}' => static fn(\\$q) => \\$q->$0,"
            }
        }));
    }

    items.extend(matches.into_iter().map(|(_, _, name, kind)| {
        let new_text = match base_path {
            Some(path) => format!("{path}.{name}"),
            None => name.clone(),
        };
        let (kind_id, detail) = match kind {
            Kind::Relation(detail) => (18, detail),
            Kind::Column(detail) => (5, detail),
        };
        let mut item = json!({
            "label": name,
            "kind": kind_id,
            "detail": detail,
            "textEdit": {
                "range": {
                    "start": { "line": line, "character": context.start_character },
                    "end":   { "line": line, "character": context.end_character }
                },
                "newText": new_text
            }
        });
        if let Some(ref edit) = trailing_comma_edit {
            item["additionalTextEdits"] = edit.clone();
        }
        item["command"] = retrigger_suggest_command();
        item
    }));

    items
}

fn complete_builder_related_columns(
    target_model: &crate::types::ModelEntry,
    relation_path: &str,
    selected_columns: &str,
    context: &BuilderArgContext,
    line: usize,
    separator: char,
) -> Vec<Value> {
    let (existing_columns, column_prefix) = match selected_columns.rsplit_once(',') {
        Some((existing, prefix)) => (Some(existing), prefix),
        None => (None, selected_columns),
    };

    let mut matches: Vec<(u32, usize, String, String)> = target_model
        .columns
        .iter()
        .filter_map(|column| {
            let score = fuzzy_score(&column.name, column_prefix)?;
            Some((
                score,
                column.name.len(),
                column.name.clone(),
                column_detail(column),
            ))
        })
        .collect();

    matches.sort_by_key(|(score, len, label, _)| (Reverse(*score), *len, label.clone()));
    matches.dedup_by(|a, b| a.2 == b.2);
    matches
        .into_iter()
        .map(|(_, _, column_name, detail)| {
            let suffix = match existing_columns {
                Some(existing) if !existing.is_empty() => format!("{existing},{column_name}"),
                _ => column_name.clone(),
            };
            let mut item = json!({
                "label": column_name,
                "kind": 5,  // Field
                "detail": detail,
                "textEdit": {
                    "range": {
                        "start": { "line": line, "character": context.start_character },
                        "end":   { "line": line, "character": context.end_character }
                    },
                    "newText": format!("{relation_path}{separator}{suffix}")
                }
            });
            item["command"] = retrigger_suggest_command();
            item
        })
        .collect()
}

fn column_detail(column: &crate::types::ColumnEntry) -> String {
    format!(
        "{} {}",
        column.column_type,
        if column.nullable { "nullable" } else { "" }
    )
    .trim()
    .to_string()
}

fn retrigger_suggest_command() -> Value {
    json!({
        "title": "Trigger completion",
        "command": "editor.action.triggerSuggest",
    })
}

pub fn builder_relation_definitions(
    index: &ProjectIndex,
    context: &BuilderRelationHoverContext,
    line: usize,
) -> Vec<Value> {
    let root_model = match index.model_for_class(&context.model_class) {
        Some(m) => m,
        None => return Vec::new(),
    };

    let containing_model = if context.path_to_segment.is_empty() {
        root_model
    } else {
        match resolve_relation_path_model(index, root_model, &context.path_to_segment) {
            Some(m) => m,
            None => return Vec::new(),
        }
    };

    let relation = match containing_model
        .relations
        .iter()
        .find(|r| r.method == context.segment)
    {
        Some(r) => r,
        None => return Vec::new(),
    };

    if relation.line == 0 {
        return Vec::new();
    }

    vec![location_link(
        &index.project_root,
        &containing_model.file,
        relation.line,
        1,
        line,
        context.origin_start_character,
        context.origin_end_character,
    )]
}

pub fn builder_relation_hover(
    index: &ProjectIndex,
    context: &BuilderRelationHoverContext,
    line: usize,
) -> Option<Value> {
    let root_model = index.model_for_class(&context.model_class)?;

    let containing_model = if context.path_to_segment.is_empty() {
        root_model
    } else {
        resolve_relation_path_model(index, root_model, &context.path_to_segment)?
    };

    let relation = containing_model
        .relations
        .iter()
        .find(|r| r.method == context.segment)?;

    let text = format!(
        "**{}** — {} → {}",
        relation.method, relation.relation_type, relation.related_model
    );

    Some(json!({
        "contents": { "kind": "markdown", "value": text },
        "range": {
            "start": { "line": line, "character": context.origin_start_character },
            "end":   { "line": line, "character": context.origin_end_character }
        }
    }))
}

fn resolve_relation_path_model<'a>(
    index: &'a ProjectIndex,
    root_model: &'a crate::types::ModelEntry,
    path: &str,
) -> Option<&'a crate::types::ModelEntry> {
    let mut current_model = root_model;

    for segment in path.split('.').filter(|segment| !segment.is_empty()) {
        let relation = current_model
            .relations
            .iter()
            .find(|relation| relation.method == segment)?;
        current_model = index.model_for_class(&relation.related_model)?;
    }

    Some(current_model)
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
        HelperContext, HelperStyle, SymbolKind, detect_blade_component_tag_context,
        detect_blade_variable_context, detect_builder_arg_context,
        detect_livewire_component_tag_context, detect_livewire_directive_value_context,
        detect_route_action_context, detect_symbol_context, detect_vendor_chain_context,
    };
    use crate::lsp::index::ProjectIndex;
    use crate::lsp::overrides::FileOverrides;
    use crate::project;

    use super::{
        blade_component_definitions, complete, complete_blade_view_variables,
        complete_builder_arg_columns, complete_livewire_components,
        complete_livewire_directive_values, complete_route_actions, complete_vendor_chain_methods,
        complete_view_data_variables, definitions, helper_snippets, hover,
        livewire_component_definitions, route_action_code_actions, route_action_definitions,
        route_diagnostics,
    };

    fn workspace_root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(Path::parent)
            .expect("editor crate should be under crates/")
            .to_path_buf()
    }

    fn sandbox_project() -> project::LaravelProject {
        let root = workspace_root().join("laravel-example").join("sandbox-app");
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

    fn vendor_chain_project() -> project::LaravelProject {
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
            "vendor/composer/autoload_classmap.php",
            r#"<?php

return array(
    'Filament\\Panel' => $vendorDir . '/filament/panel/src/Panel.php',
);
"#,
        );
        write_file(
            &root,
            "vendor/filament/panel/src/Panel.php",
            r#"<?php

namespace Filament;

class Panel
{
    public function default(): static
    {
        return $this;
    }

    public function when(bool $condition, ?callable $callback = null): static
    {
        return $this;
    }

    public function getId(): string
    {
        return 'admin';
    }
}
"#,
        );

        project::from_root(root).expect("fixture project should resolve")
    }

    fn proposal_services_project() -> project::LaravelProject {
        let root = unique_temp_project_root();
        fs::create_dir_all(&root).expect("fixture root should exist");
        write_file(&root, "composer.json", r#"{"autoload":{"psr-4":{"App\\":"app/"}}}"#);
        write_file(&root, "config/app.php", "<?php\n\nreturn [];\n");
        write_file(&root, "routes/web.php", "<?php\n");
        write_file(&root, "database/migrations/2024_01_01_000000_create_proposals_table.php", r#"<?php
use Illuminate\Database\Migrations\Migration;
use Illuminate\Database\Schema\Blueprint;
use Illuminate\Support\Facades\Schema;
return new class extends Migration {
    public function up(): void {
        Schema::create('proposals', function (Blueprint $table) {
            $table->id();
            $table->string('name');
            $table->timestamps();
        });
    }
};
"#);
        write_file(&root, "database/migrations/2024_01_01_000001_create_proposal_services_table.php", r#"<?php
use Illuminate\Database\Migrations\Migration;
use Illuminate\Database\Schema\Blueprint;
use Illuminate\Support\Facades\Schema;
return new class extends Migration {
    public function up(): void {
        Schema::create('proposal_services', function (Blueprint $table) {
            $table->id();
            $table->foreignId('proposal_id');
            $table->string('title');
            $table->timestamps();
        });
    }
};
"#);
        write_file(&root, "database/migrations/2024_01_01_000002_create_menu_items_table.php", r#"<?php
use Illuminate\Database\Migrations\Migration;
use Illuminate\Database\Schema\Blueprint;
use Illuminate\Support\Facades\Schema;
return new class extends Migration {
    public function up(): void {
        Schema::create('menu_items', function (Blueprint $table) {
            $table->id();
            $table->foreignId('proposal_service_id');
            $table->string('name');
            $table->timestamps();
        });
    }
};
"#);
        write_file(&root, "app/Models/Proposal.php", r#"<?php
namespace App\Models;
use Illuminate\Database\Eloquent\Model;
class Proposal extends Model {
    public function proposalServices() {
        return $this->hasMany(ProposalService::class);
    }
}
"#);
        write_file(&root, "app/Models/ProposalService.php", r#"<?php
namespace App\Models;
use Illuminate\Database\Eloquent\Model;
class ProposalService extends Model {
    public function menuItems() {
        return $this->hasMany(MenuItem::class);
    }
}
"#);
        write_file(&root, "app/Models/MenuItem.php", r#"<?php
namespace App\Models;
use Illuminate\Database\Eloquent\Model;
class MenuItem extends Model {}
"#);
        project::from_root(root).expect("fixture project should resolve")
    }

    fn builder_relation_project() -> project::LaravelProject {
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
            "database/migrations/2024_01_01_000000_create_venues_table.php",
            r#"<?php

use Illuminate\Database\Migrations\Migration;
use Illuminate\Database\Schema\Blueprint;
use Illuminate\Support\Facades\Schema;

return new class extends Migration
{
    public function up(): void
    {
        Schema::create('venues', function (Blueprint $table) {
            $table->id();
            $table->string('name');
            $table->timestamps();
        });
    }
};
"#,
        );
        write_file(
            &root,
            "database/migrations/2024_01_01_000001_create_proposals_table.php",
            r#"<?php

use Illuminate\Database\Migrations\Migration;
use Illuminate\Database\Schema\Blueprint;
use Illuminate\Support\Facades\Schema;

return new class extends Migration
{
    public function up(): void
    {
        Schema::create('proposals', function (Blueprint $table) {
            $table->id();
            $table->foreignId('venue_id');
            $table->string('name');
            $table->timestamps();
        });
    }
};
"#,
        );
        write_file(
            &root,
            "database/migrations/2024_01_01_000002_create_comments_table.php",
            r#"<?php

use Illuminate\Database\Migrations\Migration;
use Illuminate\Database\Schema\Blueprint;
use Illuminate\Support\Facades\Schema;

return new class extends Migration
{
    public function up(): void
    {
        Schema::create('comments', function (Blueprint $table) {
            $table->id();
            $table->foreignId('proposal_id');
            $table->string('body');
            $table->timestamps();
        });
    }
};
"#,
        );
        write_file(
            &root,
            "app/Models/Venue.php",
            r#"<?php

namespace App\Models;

use Illuminate\Database\Eloquent\Model;

class Venue extends Model
{
    public function proposals()
    {
        return $this->hasMany(Proposal::class);
    }
}
"#,
        );
        write_file(
            &root,
            "app/Models/Proposal.php",
            r#"<?php

namespace App\Models;

use Illuminate\Database\Eloquent\Model;

class Proposal extends Model
{
    public function comments()
    {
        return $this->hasMany(Comment::class);
    }
}
"#,
        );
        write_file(
            &root,
            "app/Models/Comment.php",
            r#"<?php

namespace App\Models;

use Illuminate\Database\Eloquent\Model;

class Comment extends Model
{
}
"#,
        );

        project::from_root(root).expect("fixture project should resolve")
    }

    fn builder_column_project() -> project::LaravelProject {
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
            "app/Models/Article.php",
            r#"<?php

namespace App\Models;

use Illuminate\Database\Eloquent\Model;

class Article extends Model
{
}
"#,
        );
        write_file(
            &root,
            "database/migrations/2024_01_01_000000_create_articles_table.php",
            r#"<?php

use Illuminate\Database\Migrations\Migration;
use Illuminate\Database\Schema\Blueprint;
use Illuminate\Support\Facades\Schema;

return new class extends Migration
{
    public function up(): void
    {
        Schema::create('articles', function (Blueprint $table) {
            $table->id();
            $table->string('title');
            $table->string('phone')->nullable();
            $table->timestamps();
        });
    }
};
"#,
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

    fn livewire_project() -> project::LaravelProject {
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
            "app/Livewire/ContactForm.php",
            r#"<?php

namespace App\Livewire;

use Livewire\Component;

class ContactForm extends Component
{
    public $email = '';
    public string $message = '';

    public function submit()
    {
    }

    public function render()
    {
        return view('livewire.contact-form')->layout('components.layouts.app');
    }
}
"#,
        );
        write_file(
            &root,
            "resources/views/livewire/contact-form.blade.php",
            "<form wire:submit=\"submit\"><input wire:model=\"email\">{{ $email }}</form>\n",
        );
        write_file(
            &root,
            "resources/views/components/layouts/app.blade.php",
            "<main>{{ $slot }}</main>\n",
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
            workspace_root()
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
    fn livewire_render_view_name_has_hover_and_navigation() {
        let project = livewire_project();
        let index = ProjectIndex::build_with_overrides(&project, &FileOverrides::default())
            .expect("index should build");
        let source = "return view('livewire.contact-form')->layout('components.layouts.app');";
        let character = source.find("livewire.contact-form").unwrap() + 5;
        let context = detect_symbol_context(source, 0, character).expect("view context");

        assert_eq!(context.kind, SymbolKind::View);
        assert_eq!(context.full_text, "livewire.contact-form");

        let result = hover(&index, &context, 0);
        assert!(
            result.is_some(),
            "hover should return a result for livewire view name"
        );

        let definitions = definitions(&index, &context, 0);
        let first = definitions.first().expect("definition expected");
        assert!(
            first
                .pointer("/targetUri")
                .and_then(|value| value.as_str())
                .unwrap_or_default()
                .ends_with("/app/Livewire/ContactForm.php"),
            "view('livewire.x') should navigate to the livewire class file"
        );
    }

    #[test]
    fn chained_livewire_layout_uses_view_completion_and_navigation() {
        let project = livewire_project();
        let index = ProjectIndex::build_with_overrides(&project, &FileOverrides::default())
            .expect("index should build");
        let source = "return view('livewire.contact-form')->layout('components.layouts.app');";
        let character = source.find("layouts").unwrap() + 3;
        let context = detect_symbol_context(source, 0, character).expect("layout view context");

        assert_eq!(context.kind, SymbolKind::View);

        let completions = complete(&index, &context, 0);
        assert!(
            completions
                .iter()
                .any(|item| item.get("label").and_then(|value| value.as_str())
                    == Some("components.layouts.app"))
        );

        let definitions = definitions(&index, &context, 0);
        let first = definitions.first().expect("definition expected");
        assert!(
            first
                .pointer("/targetUri")
                .and_then(|value| value.as_str())
                .unwrap_or_default()
                .ends_with("/resources/views/components/layouts/app.blade.php")
        );
    }

    #[test]
    fn vendor_chain_methods_insert_snippets_inside_parentheses() {
        let project = vendor_chain_project();
        let index = ProjectIndex::build_with_overrides(&project, &FileOverrides::default())
            .expect("index should build");
        let source = "<?php\nuse Filament\\Panel;\n\npublic function panel(Panel $panel): Panel\n{\n    return $panel\n        ->wh\n}\n";
        let line = 6;
        let line_text = source.lines().nth(line).expect("line should exist");
        let character = line_text.find("->wh").expect("token") + "->wh".len();
        let context =
            detect_vendor_chain_context(source, line, character).expect("vendor chain context");

        let items = complete_vendor_chain_methods(&index, &context, line);
        let when = items
            .iter()
            .find(|item| item.get("label").and_then(|value| value.as_str()) == Some("when"))
            .expect("when completion should exist");

        assert_eq!(
            when.pointer("/insertTextFormat")
                .and_then(|value| value.as_u64()),
            Some(2)
        );
        assert_eq!(
            when.pointer("/textEdit/newText")
                .and_then(|value| value.as_str()),
            Some("when($0)")
        );
        assert!(
            items.iter().all(|item| {
                item.get("label").and_then(|value| value.as_str()) != Some("getId")
            }),
            "non-chainable vendor methods should stay excluded",
        );
    }

    #[test]
    fn builder_relation_methods_suggest_model_relations() {
        let project = builder_relation_project();
        let index = ProjectIndex::build_with_overrides(&project, &FileOverrides::default())
            .expect("index should build");
        let source = "<?php\nuse App\\Models\\Venue;\n\nVenue::query()->withCount('prop')\n";
        let line = 3;
        let line_text = source.lines().nth(line).expect("line should exist");
        let character =
            line_text.find("->withCount('prop')").expect("token") + "->withCount('prop".len();
        let context =
            detect_builder_arg_context(source, line, character).expect("builder arg context");

        let items = complete_builder_arg_columns(&index, &context, line);
        let proposals = items
            .iter()
            .find(|item| item.get("label").and_then(|value| value.as_str()) == Some("proposals"))
            .expect("relation completion should exist");

        assert_eq!(
            proposals.pointer("/kind").and_then(|value| value.as_u64()),
            Some(18)
        );
        assert_eq!(
            proposals
                .pointer("/textEdit/newText")
                .and_then(|value| value.as_str()),
            Some("proposals")
        );
        assert_eq!(
            proposals
                .pointer("/command/command")
                .and_then(|value| value.as_str()),
            Some("editor.action.triggerSuggest")
        );
    }

    #[test]
    fn builder_static_relation_entrypoint_suggests_model_relations() {
        let project = builder_relation_project();
        let index = ProjectIndex::build_with_overrides(&project, &FileOverrides::default())
            .expect("index should build");
        let source = "<?php\nuse App\\Models\\Venue;\n\nVenue::with('')\n";
        let line = 3;
        let line_text = source.lines().nth(line).expect("line should exist");
        let character = line_text.find("''").expect("token") + 1;
        let context =
            detect_builder_arg_context(source, line, character).expect("builder arg context");

        let items = complete_builder_arg_columns(&index, &context, line);
        let proposals = items
            .iter()
            .find(|item| item.get("label").and_then(|value| value.as_str()) == Some("proposals"))
            .expect("relation completion should exist");

        assert_eq!(
            proposals
                .pointer("/textEdit/newText")
                .and_then(|value| value.as_str()),
            Some("proposals")
        );
    }

    #[test]
    fn builder_array_relation_entrypoint_suggests_model_relations() {
        let project = builder_relation_project();
        let index = ProjectIndex::build_with_overrides(&project, &FileOverrides::default())
            .expect("index should build");
        let source = "<?php\nuse App\\Models\\Venue;\n\nVenue::with(['prop'])\n";
        let line = 3;
        let line_text = source.lines().nth(line).expect("line should exist");
        let character = line_text.find("'prop'").expect("token") + "'prop".len();
        let context =
            detect_builder_arg_context(source, line, character).expect("builder arg context");

        let items = complete_builder_arg_columns(&index, &context, line);
        let proposals = items
            .iter()
            .find(|item| item.get("label").and_then(|value| value.as_str()) == Some("proposals"))
            .expect("relation completion should exist");

        assert_eq!(
            proposals
                .pointer("/textEdit/newText")
                .and_then(|value| value.as_str()),
            Some("proposals")
        );
    }

    #[test]
    fn builder_multiline_array_relation_projection_suggests_related_columns() {
        let project = builder_relation_project();
        let index = ProjectIndex::build_with_overrides(&project, &FileOverrides::default())
            .expect("index should build");
        let source = "<?php\nuse App\\Models\\Venue;\n\nVenue::with([\n    'proposals:na',\n])\n";
        let line = 4;
        let line_text = source.lines().nth(line).expect("line should exist");
        let character = line_text.find("'proposals:na'").expect("token") + "'proposals:na".len();
        let context =
            detect_builder_arg_context(source, line, character).expect("builder arg context");

        let items = complete_builder_arg_columns(&index, &context, line);
        let name = items
            .iter()
            .find(|item| item.get("label").and_then(|value| value.as_str()) == Some("name"))
            .expect("projection column completion should exist");

        assert_eq!(
            name.pointer("/textEdit/newText")
                .and_then(|value| value.as_str()),
            Some("proposals:name")
        );
    }

    #[test]
    fn proposal_services_dot_suggests_menu_items_relation() {
        let project = proposal_services_project();
        let index = ProjectIndex::build_with_overrides(&project, &FileOverrides::default())
            .expect("index should build");
        // Mirrors the user's exact scenario: Proposal::with([...'proposalServices.'...])
        let source = concat!(
            "<?php\nuse App\\Models\\Proposal;\n\n",
            "Proposal::with([\n",
            "    'proposalServices.',\n",
            "])\n",
        );
        let line = 4;
        let line_text = source.lines().nth(line).expect("line should exist");
        let character = line_text.find("'proposalServices.'").expect("token") + "'proposalServices.".len();
        let context =
            detect_builder_arg_context(source, line, character).expect("builder arg context");

        let items = complete_builder_arg_columns(&index, &context, line);
        let menu_items = items
            .iter()
            .find(|item| item.get("label").and_then(|v| v.as_str()) == Some("menuItems"))
            .expect("should suggest ProposalService relations (menuItems) after dot");

        assert_eq!(
            menu_items.pointer("/textEdit/newText").and_then(|v| v.as_str()),
            Some("proposalServices.menuItems")
        );
    }

    #[test]
    fn builder_relation_dot_with_empty_fragment_suggests_nested_relations() {
        let project = builder_relation_project();
        let index = ProjectIndex::build_with_overrides(&project, &FileOverrides::default())
            .expect("index should build");
        let source = "<?php\nuse App\\Models\\Venue;\n\nVenue::query()->with('proposals.')\n";
        let line = 3;
        let line_text = source.lines().nth(line).expect("line should exist");
        let character = line_text.find("->with('proposals.')").expect("token")
            + "->with('proposals.".len();
        let context =
            detect_builder_arg_context(source, line, character).expect("builder arg context");

        let items = complete_builder_arg_columns(&index, &context, line);
        let comments = items
            .iter()
            .find(|item| item.get("label").and_then(|v| v.as_str()) == Some("comments"))
            .expect("should suggest nested relations when fragment is empty after dot");

        assert_eq!(
            comments.pointer("/textEdit/newText").and_then(|v| v.as_str()),
            Some("proposals.comments")
        );
    }

    #[test]
    fn builder_multiline_array_dot_with_empty_fragment_suggests_nested_relations() {
        let project = builder_relation_project();
        let index = ProjectIndex::build_with_overrides(&project, &FileOverrides::default())
            .expect("index should build");
        let source = "<?php\nuse App\\Models\\Venue;\n\nVenue::with([\n    'proposals.',\n])\n";
        let line = 4;
        let line_text = source.lines().nth(line).expect("line should exist");
        let character = line_text.find("'proposals.'").expect("token") + "'proposals.".len();
        let context =
            detect_builder_arg_context(source, line, character).expect("builder arg context");

        let items = complete_builder_arg_columns(&index, &context, line);
        let comments = items
            .iter()
            .find(|item| item.get("label").and_then(|v| v.as_str()) == Some("comments"))
            .expect("multiline array: should suggest nested relations when fragment is empty after dot");

        assert_eq!(
            comments.pointer("/textEdit/newText").and_then(|v| v.as_str()),
            Some("proposals.comments")
        );
    }

    #[test]
    fn builder_chained_multiline_array_dot_suggests_nested_relations() {
        let project = builder_relation_project();
        let index = ProjectIndex::build_with_overrides(&project, &FileOverrides::default())
            .expect("index should build");
        // Matches the user's real-world chained structure:
        //   Venue::with([...'proposals.',...])->with(['..'])->withCount([...])->latest()
        let source = concat!(
            "<?php\nuse App\\Models\\Venue;\n\n",
            "Venue::with([\n",
            "    'name',\n",
            "    'proposals.',\n",   // ← cursor here
            "])->with(['name'])->withCount(['proposals'])->latest();\n",
        );
        let line = 5;
        let line_text = source.lines().nth(line).expect("line should exist");
        let character = line_text.find("'proposals.'").expect("token") + "'proposals.".len();
        let context =
            detect_builder_arg_context(source, line, character).expect("builder arg context");

        let items = complete_builder_arg_columns(&index, &context, line);
        let comments = items
            .iter()
            .find(|item| item.get("label").and_then(|v| v.as_str()) == Some("comments"))
            .expect("chained call: should suggest nested relations after dot");

        assert_eq!(
            comments.pointer("/textEdit/newText").and_then(|v| v.as_str()),
            Some("proposals.comments")
        );
    }

    #[test]
    fn builder_relation_methods_support_nested_relation_paths() {
        let project = builder_relation_project();
        let index = ProjectIndex::build_with_overrides(&project, &FileOverrides::default())
            .expect("index should build");
        let source = "<?php\nuse App\\Models\\Venue;\n\nVenue::query()->with('proposals.com')\n";
        let line = 3;
        let line_text = source.lines().nth(line).expect("line should exist");
        let character = line_text.find("->with('proposals.com')").expect("token")
            + "->with('proposals.com".len();
        let context =
            detect_builder_arg_context(source, line, character).expect("builder arg context");

        let items = complete_builder_arg_columns(&index, &context, line);
        let comments = items
            .iter()
            .find(|item| item.get("label").and_then(|value| value.as_str()) == Some("comments"))
            .expect("nested relation completion should exist");

        assert_eq!(
            comments
                .pointer("/textEdit/newText")
                .and_then(|value| value.as_str()),
            Some("proposals.comments")
        );
    }

    #[test]
    fn builder_nested_relation_path_suggests_related_columns() {
        let project = builder_relation_project();
        let index = ProjectIndex::build_with_overrides(&project, &FileOverrides::default())
            .expect("index should build");
        let source = "<?php\nuse App\\Models\\Venue;\n\nVenue::with('proposals.na')\n";
        let line = 3;
        let line_text = source.lines().nth(line).expect("line should exist");
        let character =
            line_text.find("::with('proposals.na')").expect("token") + "::with('proposals.na".len();
        let context =
            detect_builder_arg_context(source, line, character).expect("builder arg context");

        let items = complete_builder_arg_columns(&index, &context, line);
        let name = items
            .iter()
            .find(|item| item.get("label").and_then(|value| value.as_str()) == Some("name"))
            .expect("related column completion should exist");

        assert_eq!(
            name.pointer("/textEdit/newText")
                .and_then(|value| value.as_str()),
            Some("proposals.name")
        );
    }

    #[test]
    fn builder_relation_projection_suggests_related_columns_after_colon() {
        let project = builder_relation_project();
        let index = ProjectIndex::build_with_overrides(&project, &FileOverrides::default())
            .expect("index should build");
        let source = "<?php\nuse App\\Models\\Venue;\n\nVenue::with('proposals:na')\n";
        let line = 3;
        let line_text = source.lines().nth(line).expect("line should exist");
        let character =
            line_text.find("::with('proposals:na')").expect("token") + "::with('proposals:na".len();
        let context =
            detect_builder_arg_context(source, line, character).expect("builder arg context");

        let items = complete_builder_arg_columns(&index, &context, line);
        let name = items
            .iter()
            .find(|item| item.get("label").and_then(|value| value.as_str()) == Some("name"))
            .expect("projection column completion should exist");

        assert_eq!(
            name.pointer("/textEdit/newText")
                .and_then(|value| value.as_str()),
            Some("proposals:name")
        );
    }

    #[test]
    fn builder_relation_projection_supports_comma_separated_columns() {
        let project = builder_relation_project();
        let index = ProjectIndex::build_with_overrides(&project, &FileOverrides::default())
            .expect("index should build");
        let source = "<?php\nuse App\\Models\\Venue;\n\nVenue::with('proposals:id,na')\n";
        let line = 3;
        let line_text = source.lines().nth(line).expect("line should exist");
        let character = line_text.find("::with('proposals:id,na')").expect("token")
            + "::with('proposals:id,na".len();
        let context =
            detect_builder_arg_context(source, line, character).expect("builder arg context");

        let items = complete_builder_arg_columns(&index, &context, line);
        let name = items
            .iter()
            .find(|item| item.get("label").and_then(|value| value.as_str()) == Some("name"))
            .expect("projection column completion should exist");

        assert_eq!(
            name.pointer("/textEdit/newText")
                .and_then(|value| value.as_str()),
            Some("proposals:id,name")
        );
    }

    #[test]
    fn builder_column_methods_retrigger_completion_after_accept() {
        let project = builder_column_project();
        let index = ProjectIndex::build_with_overrides(&project, &FileOverrides::default())
            .expect("index should build");
        let source = "<?php\n/** @var Article $article */\n$article->where('tit')\n";
        let line = 2;
        let line_text = source.lines().nth(line).expect("line should exist");
        let character = line_text.find("->where('tit')").expect("token") + "->where('tit".len();
        let context =
            detect_builder_arg_context(source, line, character).expect("builder arg context");

        let items = complete_builder_arg_columns(&index, &context, line);
        let first = items.first().expect("column completion should exist");

        assert_eq!(
            first
                .pointer("/command/command")
                .and_then(|value| value.as_str()),
            Some("editor.action.triggerSuggest")
        );
    }

    #[test]
    fn completes_and_navigates_livewire_component_tags() {
        let project = livewire_project();
        let index = ProjectIndex::build_with_overrides(&project, &FileOverrides::default())
            .expect("index should build");
        let source = "<livewire:contact-form />\n";
        let character = source.find("contact-form").unwrap() + "contact".len();
        let context = detect_livewire_component_tag_context(
            "file:///tmp/resources/views/welcome.blade.php",
            source,
            0,
            character,
        )
        .expect("livewire component context");

        let completions = complete_livewire_components(&index, &context, 0);
        assert!(completions.iter().any(|item| {
            item.get("filterText").and_then(|value| value.as_str()) == Some("contact-form")
        }));

        let definitions = livewire_component_definitions(&index, &context, 0);
        let first = definitions.first().expect("definition expected");
        assert!(
            first
                .pointer("/targetUri")
                .and_then(|value| value.as_str())
                .unwrap_or_default()
                .ends_with("/app/Livewire/ContactForm.php")
        );
    }

    #[test]
    fn livewire_public_properties_are_available_in_component_blade() {
        let project = livewire_project();
        let index = ProjectIndex::build_with_overrides(&project, &FileOverrides::default())
            .expect("index should build");
        let file = Path::new("resources/views/livewire/contact-form.blade.php");
        let source = "{{ $ema }}";
        let character = source.find("ema").unwrap() + "ema".len();
        let context = detect_blade_variable_context(
            "file:///tmp/resources/views/livewire/contact-form.blade.php",
            source,
            0,
            character,
        )
        .expect("blade variable context");

        let items = complete_blade_view_variables(&index, file, &context, 0);
        assert!(
            items.iter().any(
                |item| item.get("filterText").and_then(|value| value.as_str()) == Some("email")
            )
        );
    }

    #[test]
    fn livewire_wire_directive_values_complete_component_members() {
        let project = livewire_project();
        let index = ProjectIndex::build_with_overrides(&project, &FileOverrides::default())
            .expect("index should build");
        let file = Path::new("resources/views/livewire/contact-form.blade.php");

        let model_source = r#"<input wire:model="ema">"#;
        let model_character = model_source.find("ema").unwrap() + "ema".len();
        let model_context = detect_livewire_directive_value_context(
            "file:///tmp/resources/views/livewire/contact-form.blade.php",
            model_source,
            0,
            model_character,
        )
        .expect("wire:model context");
        let model_items = complete_livewire_directive_values(&index, file, &model_context, 0);
        assert!(
            model_items
                .iter()
                .any(|item| item.get("label").and_then(|value| value.as_str()) == Some("email"))
        );

        let click_source = r#"<button wire:click="sub">"#;
        let click_character = click_source.find("sub").unwrap() + "sub".len();
        let click_context = detect_livewire_directive_value_context(
            "file:///tmp/resources/views/livewire/contact-form.blade.php",
            click_source,
            0,
            click_character,
        )
        .expect("wire:click context");
        let click_items = complete_livewire_directive_values(&index, file, &click_context, 0);
        assert!(
            click_items
                .iter()
                .any(|item| item.get("label").and_then(|value| value.as_str()) == Some("submit"))
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
    fn asset_hover_definition_and_completion_resolve_public_file() {
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

    #[test]
    fn compact_second_argument_gets_variable_completions() {
        // Regression: detect_view_data_context previously required compact( to immediately
        // precede the cursor quote, so only the first argument matched.
        let source = r#"<?php

class DemoController
{
    public function index()
    {
        $proposals = [];
        $paginate = false;

        return view('proposals.index', compact('proposals', ''));
    }
}
"#;
        let line = source
            .lines()
            .position(|line| line.contains("compact('proposals', '')"))
            .expect("compact line should exist");
        let line_text = source.lines().nth(line).expect("line should exist");
        // Position cursor inside the second empty string ''
        let second_empty = line_text
            .find(", ''")
            .expect("second empty string should exist");
        let character = second_empty + ", '".len();

        let context = crate::lsp::context::detect_view_data_context(
            "file:///tmp/app/Http/Controllers/DemoController.php",
            source,
            line,
            character,
        )
        .expect("view data context should fire for second compact argument");

        let items = complete_view_data_variables(source, &context, line);
        let labels = items
            .iter()
            .filter_map(|item| item.get("label").and_then(|v| v.as_str()))
            .collect::<Vec<_>>();

        assert!(labels.contains(&"proposals"), "should suggest $proposals");
        assert!(labels.contains(&"paginate"), "should suggest $paginate");
    }

    #[test]
    fn compact_skips_inner_closure_and_finds_outer_function_variables() {
        // Regression: enclosing_function_bounds used rfind("function") which found the
        // nearest `static function` closure body (already closed before the cursor) and
        // returned None, so compact() suggested nothing.
        let source = r#"<?php

class ProposalController
{
    public function index()
    {
        $proposals = [];
        $paginate = false;

        $proposals = collect($proposals)->each(static function ($item): void {
            $item->doSomething();
        });

        return view('proposals.index', compact(''));
    }
}
"#;
        let line = source
            .lines()
            .position(|l| l.contains("compact('')"))
            .expect("compact line");
        let line_text = source.lines().nth(line).expect("line");
        let character = line_text.find("''").expect("empty string") + 1;

        let context = crate::lsp::context::detect_view_data_context(
            "file:///tmp/app/Http/Controllers/ProposalController.php",
            source,
            line,
            character,
        )
        .expect("view data context should fire");

        let items = complete_view_data_variables(source, &context, line);
        let labels: Vec<_> = items
            .iter()
            .filter_map(|item| item.get("label").and_then(|v| v.as_str()))
            .collect();

        assert!(labels.contains(&"proposals"), "should suggest $proposals from outer function");
        assert!(labels.contains(&"paginate"), "should suggest $paginate from outer function");
    }

    #[test]
    fn view_navigation_not_blocked_by_preceding_builder_chain() {
        // Regression: find_enclosing_builder_array_call had no statement-boundary guard,
        // so detect_builder_relation_hover_context fired on view('...') strings that
        // appeared after a with([...]) call in the same function, swallowing the view
        // navigation via the else-if chain in definition_result.
        let source = concat!(
            "<?php\nuse App\\Models\\Proposal;\n\n",
            "class ProposalController {\n",
            "    public function index() {\n",
            "        $proposals = Proposal::with([\n",
            "            'client',\n",
            "            'venue',\n",
            "        ])->latest();\n",    // ends with ';' — statement boundary
            "        return view('proposals.index', compact('proposals'));\n",
            "    }\n",
            "}\n",
        );
        let line = source
            .lines()
            .position(|l| l.contains("view('proposals.index'"))
            .expect("view line should exist");
        let line_text = source.lines().nth(line).expect("line should exist");
        let character = line_text
            .find("'proposals.index'")
            .expect("view token should exist")
            + 1;

        // Must NOT fire — view() strings are not relation strings.
        let result = crate::lsp::context::detect_builder_relation_hover_context(
            source, line, character,
        );
        assert!(
            result.is_none(),
            "detect_builder_relation_hover_context should not fire on a view() string"
        );
    }
}
