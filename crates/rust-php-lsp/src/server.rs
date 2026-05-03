use std::collections::{HashMap, HashSet};
use std::fs::OpenOptions;
use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

use bumpalo::Bump;
use php_parser::lexer::Lexer;
use php_parser::parser::Parser;
use serde_json::{Value, json};

use super::context::{
    self, detect_blade_component_attr_context, detect_blade_component_tag_context,
    detect_blade_model_property_context, detect_blade_variable_context, detect_builder_arg_context,
    detect_foreach_alias_context, detect_helper_context, detect_livewire_component_tag_context,
    detect_livewire_directive_value_context, detect_route_action_context, detect_symbol_context,
    detect_vendor_chain_context, detect_vendor_make_context, detect_view_data_context,
};
use super::index::ProjectIndex;
use super::overrides::FileOverrides;
use super::query;
use crate::analyzers::routes;
use crate::project;

pub fn run_stdio() -> Result<(), String> {
    let stdin = io::stdin();
    let mut stdin = stdin.lock();
    let stdout = io::stdout();
    let mut stdout = stdout.lock();
    let mut state = ServerState::default();

    while let Some(message) = read_message(&mut stdin)? {
        if let Some(response) = handle_message(&mut state, message)? {
            write_message(&mut stdout, &response)?;
        }
        if state.exiting {
            break;
        }
    }

    Ok(())
}

#[derive(Default)]
struct ServerState {
    project_root: Option<PathBuf>,
    project: Option<project::LaravelProject>,
    index: Option<ProjectIndex>,
    documents: HashMap<String, String>,
    dirty_documents: HashSet<String>,
    parse_failed_documents: HashSet<String>,
    shutdown_requested: bool,
    exiting: bool,
}

fn handle_message(state: &mut ServerState, message: Value) -> Result<Option<Value>, String> {
    let method = message.get("method").and_then(Value::as_str);
    let id = message.get("id").cloned();

    match method {
        Some("initialize") => {
            let params = message.get("params").cloned().unwrap_or_else(|| json!({}));
            initialize(state, params);
            Ok(id.map(|id| success(id, initialize_result())))
        }
        Some("initialized") => Ok(None),
        Some("shutdown") => {
            state.shutdown_requested = true;
            Ok(id.map(|id| success(id, Value::Null)))
        }
        Some("exit") => {
            state.exiting = true;
            Ok(None)
        }
        Some("textDocument/didOpen") => {
            if let Some(params) = message.get("params") {
                if let (Some(uri), Some(text)) = (
                    params.pointer("/textDocument/uri").and_then(Value::as_str),
                    params.pointer("/textDocument/text").and_then(Value::as_str),
                ) {
                    state.documents.insert(uri.to_string(), text.to_string());
                    state.dirty_documents.remove(uri);
                    state.parse_failed_documents.remove(uri);
                    log_lsp_event(format!("didOpen uri={uri} bytes={}", text.len()));
                    if uri_affects_index(state, uri) {
                        if document_is_reindexable(state, uri) {
                            reindex_for_uri(state, uri);
                        } else if let Some(source) = state.documents.get(uri) {
                            log_php_parse_errors(uri, source, "didOpen");
                        }
                    }
                }
            }
            Ok(None)
        }
        Some("textDocument/didChange") => {
            if let Some(params) = message.get("params") {
                if let Some(uri) = params.pointer("/textDocument/uri").and_then(Value::as_str) {
                    if let Some(text) = params
                        .pointer("/contentChanges/0/text")
                        .and_then(Value::as_str)
                    {
                        state.documents.insert(uri.to_string(), text.to_string());
                        state.dirty_documents.insert(uri.to_string());
                        state.parse_failed_documents.remove(uri);
                        log_lsp_event(format!(
                            "didChange uri={uri} bytes={} dirty=true reindex=deferred",
                            text.len()
                        ));
                    }
                }
            }
            Ok(None)
        }
        Some("textDocument/didSave") => {
            if let Some(uri) = message
                .pointer("/params/textDocument/uri")
                .and_then(Value::as_str)
            {
                state.dirty_documents.remove(uri);
                log_lsp_event(format!("didSave uri={uri} dirty=false"));
                if !document_is_reindexable(state, uri) {
                    state.parse_failed_documents.insert(uri.to_string());
                    log_lsp_event(format!(
                        "didSave uri={uri} reindex=skipped reason=parse-error"
                    ));
                    if let Some(source) = state.documents.get(uri) {
                        log_php_parse_errors(uri, source, "didSave");
                    }
                } else {
                    state.parse_failed_documents.remove(uri);
                }
                if uri_affects_index(state, uri) && !state.parse_failed_documents.contains(uri) {
                    reindex_for_uri(state, uri);
                }
            }
            Ok(None)
        }
        Some("textDocument/didClose") => {
            if let Some(uri) = message
                .pointer("/params/textDocument/uri")
                .and_then(Value::as_str)
            {
                let parse_failed = state.parse_failed_documents.contains(uri);
                state.documents.remove(uri);
                state.dirty_documents.remove(uri);
                state.parse_failed_documents.remove(uri);
                log_lsp_event(format!("didClose uri={uri}"));
                if parse_failed {
                    log_lsp_event(format!(
                        "didClose uri={uri} reindex=skipped reason=parse-error"
                    ));
                } else if uri_affects_index(state, uri) {
                    reindex_for_uri(state, uri);
                }
            }
            Ok(None)
        }
        Some("textDocument/completion") => {
            Ok(id.map(|id| success(id, completion_result(state, message.get("params")))))
        }
        Some("textDocument/definition") => {
            Ok(id.map(|id| success(id, definition_result(state, message.get("params")))))
        }
        Some("textDocument/hover") => {
            Ok(id.map(|id| success(id, hover_result(state, message.get("params")))))
        }
        Some("textDocument/diagnostic") => {
            Ok(id.map(|id| success(id, diagnostic_result(state, message.get("params")))))
        }
        Some("textDocument/codeAction") => {
            Ok(id.map(|id| success(id, code_action_result(state, message.get("params")))))
        }
        Some("textDocument/onTypeFormatting") => {
            Ok(id.map(|id| success(id, on_type_format_result(state, message.get("params")))))
        }
        Some("workspace/executeCommand") => {
            Ok(id.map(
                |id| match execute_command_result(state, message.get("params")) {
                    Ok(result) => success(id, result),
                    Err(message) => error(id, -32603, &message),
                },
            ))
        }
        Some(_) | None => {
            if let Some(id) = id {
                Ok(Some(success(id, Value::Null)))
            } else {
                Ok(None)
            }
        }
    }
}

fn initialize(state: &mut ServerState, params: Value) {
    let root_path = params
        .get("rootUri")
        .and_then(Value::as_str)
        .and_then(file_uri_to_path)
        .or_else(|| {
            params
                .get("rootPath")
                .and_then(Value::as_str)
                .map(PathBuf::from)
        })
        .or_else(|| {
            params
                .pointer("/workspaceFolders/0/uri")
                .and_then(Value::as_str)
                .and_then(file_uri_to_path)
        });

    state.project_root = root_path.clone();
    state.project = root_path.and_then(|root| project::from_root(root).ok());
    log_lsp_event(format!(
        "initialize root={}",
        state
            .project
            .as_ref()
            .map(|project| project.root.display().to_string())
            .unwrap_or_else(|| "<unresolved>".to_string())
    ));
    state.index = rebuild_index(state).ok();
}

fn completion_result(state: &ServerState, params: Option<&Value>) -> Value {
    let Some((uri, source, line, character)) = source_and_position(state, params) else {
        return json!({ "isIncomplete": false, "items": [] });
    };
    let Some(index) = &state.index else {
        return json!({ "isIncomplete": false, "items": [] });
    };

    let (items, is_incomplete) = if let Some(context) =
        detect_livewire_component_tag_context(uri, source, line, character)
    {
        log_lsp_event(format!(
            "completion uri={uri} line={} char={} context=livewire-component-tag prefix={:?}",
            line, character, context.prefix
        ));
        (
            query::complete_livewire_components(index, &context, line),
            true,
        )
    } else if let Some(context) = detect_blade_component_tag_context(uri, source, line, character) {
        log_lsp_event(format!(
            "completion uri={uri} line={} char={} context=blade-component-tag prefix={:?}",
            line, character, context.prefix
        ));
        (
            query::complete_blade_components(index, &context, line),
            true,
        )
    } else if let Some(context) = detect_blade_component_attr_context(uri, source, line, character)
    {
        log_lsp_event(format!(
            "completion uri={uri} line={} char={} context=blade-component-attr component={:?} prefix={:?}",
            line, character, context.component, context.prefix
        ));
        (
            query::complete_blade_component_props(index, &context, line),
            true,
        )
    } else if let Some(context) = detect_view_data_context(uri, source, line, character) {
        log_lsp_event(format!(
            "completion uri={uri} line={} char={} context=view-data prefix={:?}",
            line, character, context.prefix
        ));
        (
            query::complete_view_data_variables(source, &context, line),
            true,
        )
    } else if let Some(context) = detect_foreach_alias_context(uri, source, line, character) {
        log_lsp_event(format!(
            "completion uri={uri} line={} char={} context=foreach-alias collection={:?} prefix={:?}",
            line, character, context.collection_name, context.prefix
        ));
        (query::complete_foreach_alias(&context, line), false)
    } else if let Some(context) = detect_blade_model_property_context(uri, source, line, character)
    {
        let relative = file_uri_to_path(uri).and_then(|path| {
            path.strip_prefix(&index.project_root)
                .ok()
                .map(PathBuf::from)
        });
        log_lsp_event(format!(
            "completion uri={uri} line={} char={} context=blade-model-property var={:?} prefix={:?}",
            line, character, context.variable_name, context.prefix
        ));
        (
            relative
                .as_deref()
                .map(|file| query::complete_blade_model_properties(index, file, &context, line))
                .unwrap_or_default(),
            true,
        )
    } else if let Some(context) = detect_blade_variable_context(uri, source, line, character) {
        let relative = file_uri_to_path(uri).and_then(|path| {
            path.strip_prefix(&index.project_root)
                .ok()
                .map(PathBuf::from)
        });
        log_lsp_event(format!(
            "completion uri={uri} line={} char={} context=blade-variable prefix={:?}",
            line, character, context.prefix
        ));
        (
            relative
                .as_deref()
                .map(|file| query::complete_blade_view_variables(index, file, &context, line))
                .unwrap_or_default(),
            true,
        )
    } else if let Some(context) =
        detect_livewire_directive_value_context(uri, source, line, character)
    {
        let relative = file_uri_to_path(uri).and_then(|path| {
            path.strip_prefix(&index.project_root)
                .ok()
                .map(PathBuf::from)
        });
        log_lsp_event(format!(
            "completion uri={uri} line={} char={} context=livewire-directive directive={} prefix={:?}",
            line, character, context.directive, context.prefix
        ));
        (
            relative
                .as_deref()
                .map(|file| query::complete_livewire_directive_values(index, file, &context, line))
                .unwrap_or_default(),
            true,
        )
    } else if let Some(context) = detect_builder_arg_context(source, line, character) {
        log_lsp_event(format!(
            "completion uri={uri} line={} char={} context=builder-arg model={:?} prefix={:?}",
            line, character, context.model_class, context.prefix
        ));
        (
            query::complete_builder_arg_columns(index, &context, line),
            true,
        )
    } else if let Some(context) = detect_vendor_chain_context(source, line, character) {
        log_lsp_event(format!(
            "completion uri={uri} line={} char={} context=vendor-chain class={:?} prefix={:?}",
            line, character, context.class_fqn, context.prefix
        ));
        (
            query::complete_vendor_chain_methods(index, &context, line),
            true,
        )
    } else if let Some(context) = detect_vendor_make_context(uri, source, line, character) {
        log_lsp_event(format!(
            "completion uri={uri} line={} char={} context=vendor-make class={:?} model={:?} prefix={:?}",
            line, character, context.class_short, context.model_class, context.prefix
        ));
        (
            query::complete_vendor_make_columns(index, &context, line),
            true,
        )
    } else if let Some(context) = detect_symbol_context(source, line, character) {
        log_lsp_event(format!(
            "completion uri={uri} line={} char={} context=symbol prefix={:?}",
            line, character, context.prefix
        ));
        (query::complete(index, &context, line), true)
    } else if let Some(context) = detect_route_action_context(uri, source, line, character) {
        log_lsp_event(format!(
            "completion uri={uri} line={} char={} context=route-action kind={:?} controller={:?} prefix={:?}",
            line, character, context.kind, context.controller, context.prefix
        ));
        (query::complete_route_actions(index, &context, line), true)
    } else if let Some(context) = detect_helper_context(uri, source, line, character) {
        log_lsp_event(format!(
            "completion uri={uri} line={} char={} context=helper prefix={:?}",
            line, character, context.prefix
        ));
        (query::helper_snippets(&context, line), true)
    } else {
        log_lsp_event(format!(
            "completion uri={uri} line={} char={} context=none",
            line, character
        ));
        (Vec::new(), false)
    };

    json!({
        "isIncomplete": is_incomplete,
        "items": items,
    })
}

fn definition_result(state: &ServerState, params: Option<&Value>) -> Value {
    let Some((uri, source, line, character)) = source_and_position(state, params) else {
        return Value::Null;
    };
    let Some(index) = &state.index else {
        return Value::Null;
    };

    let definitions = if let Some(context) =
        detect_livewire_component_tag_context(uri, source, line, character)
    {
        query::livewire_component_definitions(index, &context, line)
    } else if let Some(context) = detect_blade_component_tag_context(uri, source, line, character) {
        query::blade_component_definitions(index, &context, line)
    } else if let Some(context) = detect_symbol_context(source, line, character) {
        query::definitions(index, &context, line)
    } else if let Some(context) = detect_route_action_context(uri, source, line, character) {
        query::route_action_definitions(index, &context, line)
    } else {
        Vec::new()
    };

    if definitions.is_empty() {
        Value::Null
    } else {
        Value::Array(definitions)
    }
}

fn hover_result(state: &ServerState, params: Option<&Value>) -> Value {
    let Some((uri, source, line, character)) = source_and_position(state, params) else {
        return Value::Null;
    };
    let Some(index) = &state.index else {
        return Value::Null;
    };

    if let Some(context) = detect_livewire_component_tag_context(uri, source, line, character) {
        return query::livewire_component_hover(index, &context, line).unwrap_or(Value::Null);
    }
    if let Some(context) = detect_blade_component_tag_context(uri, source, line, character) {
        return query::blade_component_hover(index, &context, line).unwrap_or(Value::Null);
    }
    if let Some(context) = detect_symbol_context(source, line, character) {
        return query::hover(index, &context, line).unwrap_or(Value::Null);
    }
    if let Some(context) = detect_route_action_context(uri, source, line, character) {
        return query::route_action_hover(index, &context, line).unwrap_or(Value::Null);
    }

    Value::Null
}

fn on_type_format_result(state: &ServerState, params: Option<&Value>) -> Value {
    let Some(params) = params else {
        return Value::Null;
    };
    let Some(uri) = params.pointer("/textDocument/uri").and_then(Value::as_str) else {
        return Value::Null;
    };
    let Some(ch) = params.pointer("/ch").and_then(Value::as_str) else {
        return Value::Null;
    };
    // Only handle quote characters.
    if ch != "'" && ch != "\"" {
        return Value::Null;
    }
    let Some(trigger_line) = params.pointer("/position/line").and_then(Value::as_u64) else {
        return Value::Null;
    };
    let Some(trigger_character) = params.pointer("/position/character").and_then(Value::as_u64)
    else {
        return Value::Null;
    };
    let trigger_line = trigger_line as usize;
    let trigger_character = trigger_character as usize;

    let Some(source) = state.documents.get(uri) else {
        return Value::Null;
    };
    let lines: Vec<&str> = source.lines().collect();
    let Some(line_text) = lines.get(trigger_line) else {
        return Value::Null;
    };

    // Convert trigger_character (LSP char index) to a byte offset.
    // trigger_character points to the cursor position — which for an auto-paired quote is
    // between the two quotes: ['|'] where | is the cursor.
    // The closing auto-paired quote is therefore at exactly trigger_character.
    let trigger_byte = line_text
        .char_indices()
        .nth(trigger_character)
        .map(|(b, _)| b)
        .unwrap_or(line_text.len());

    let quote_char = ch.chars().next().unwrap();

    // Confirm the character at trigger_byte is the same quote — this is how we know the
    // editor auto-paired it (opening typed → closing placed at cursor). If the user typed
    // the closing quote manually, the cursor is past the quote and this check fails.
    if line_text[trigger_byte..].chars().next() != Some(quote_char) {
        return Value::Null;
    }

    // Already has a trailing comma after the closing quote — nothing to do.
    let after_close_byte = trigger_byte + quote_char.len_utf8();
    if line_text[after_close_byte..].trim_start().starts_with(',') {
        return Value::Null;
    }

    // Check that the cursor is inside a builder method array argument.
    // Scan backward for an unclosed `->method([` or `::method([`.
    if !cursor_is_inside_builder_array(&lines, trigger_line, trigger_byte) {
        return Value::Null;
    }

    // Insert ',' immediately after the closing quote.
    // trigger_character is the column of the closing quote; +1 is the column after it.
    let insert_character = trigger_character + 1;
    json!([{
        "range": {
            "start": { "line": trigger_line, "character": insert_character },
            "end":   { "line": trigger_line, "character": insert_character }
        },
        "newText": ","
    }])
}

/// Returns true when `(line, col_byte)` appears to be inside a `->method([` or `::method([`
/// array argument, by scanning backward up to 30 lines for an unclosed array opener.
fn cursor_is_inside_builder_array(lines: &[&str], line: usize, col_byte: usize) -> bool {
    let start = line.saturating_sub(30);
    for idx in (start..=line).rev() {
        let text = lines[idx];
        let limit = if idx == line {
            col_byte.min(text.len())
        } else {
            text.len()
        };
        let compact: String = text[..limit]
            .chars()
            .filter(|c| !c.is_whitespace())
            .collect();

        for &method in context::BUILDER_RELATION_METHODS {
            if compact.contains(&format!("->{method}(["))
                || compact.contains(&format!("::{method}(["))
            {
                return true;
            }
        }

        // A closing `])` means the array ended before reaching the opener — stop.
        if compact.ends_with("])") || compact.ends_with("]);") {
            break;
        }
    }
    false
}

fn diagnostic_result(state: &ServerState, params: Option<&Value>) -> Value {
    let Some(params) = params else {
        return json!({ "kind": "full", "items": [] });
    };
    let Some(uri) = params.pointer("/textDocument/uri").and_then(Value::as_str) else {
        return json!({ "kind": "full", "items": [] });
    };
    if state.dirty_documents.contains(uri) {
        log_lsp_event(format!("diagnostics uri={uri} skipped=dirty items=0"));
        return json!({ "kind": "full", "items": [] });
    }
    if state.parse_failed_documents.contains(uri) {
        log_lsp_event(format!("diagnostics uri={uri} skipped=parse-error items=0"));
        return json!({ "kind": "full", "items": [] });
    }
    let Some(index) = &state.index else {
        return json!({ "kind": "full", "items": [] });
    };
    let Some(source) = state.documents.get(uri) else {
        return json!({ "kind": "full", "items": [] });
    };
    let Some(path) = file_uri_to_path(uri) else {
        return json!({ "kind": "full", "items": [] });
    };
    let Ok(relative) = path.strip_prefix(&index.project_root) else {
        return json!({ "kind": "full", "items": [] });
    };

    let items = query::route_diagnostics(index, relative, source);
    log_lsp_event(format!(
        "diagnostics uri={uri} skipped=false items={}",
        items.len()
    ));
    json!({
        "kind": "full",
        "items": items,
    })
}

fn code_action_result(state: &ServerState, params: Option<&Value>) -> Value {
    let Some(params) = params else {
        return Value::Array(Vec::new());
    };
    let Some((uri, source, line, character)) = source_and_position_from_range(state, Some(params))
    else {
        return Value::Array(Vec::new());
    };
    let Some(index) = &state.index else {
        return Value::Array(Vec::new());
    };
    let mut actions = params
        .pointer("/context/diagnostics")
        .and_then(Value::as_array)
        .cloned()
        .map(|diagnostics| query::route_action_code_actions(index, &diagnostics))
        .unwrap_or_default();

    if let Some(context) = detect_symbol_context(source, line, character) {
        if context.kind == super::context::SymbolKind::Asset {
            actions.extend(query::asset_code_actions(index, &context));
        }
    }

    if let Some(context) = detect_blade_component_tag_context(uri, source, line, character) {
        actions.extend(query::blade_component_create_actions(index, &context));
    }

    Value::Array(actions)
}

fn execute_command_result(_state: &ServerState, params: Option<&Value>) -> Result<Value, String> {
    let Some(params) = params else {
        return Ok(Value::Null);
    };
    let Some(command) = params.get("command").and_then(Value::as_str) else {
        return Ok(Value::Null);
    };

    match command {
        "rust-php.openAssetInZed" => {
            let path = params
                .pointer("/arguments/0")
                .and_then(Value::as_str)
                .ok_or_else(|| "missing asset path argument".to_string())?;
            open_in_zed(Path::new(path))?;
            Ok(Value::Null)
        }
        _ => Ok(Value::Null),
    }
}

fn source_and_position<'a>(
    state: &'a ServerState,
    params: Option<&'a Value>,
) -> Option<(&'a str, &'a str, usize, usize)> {
    let params = params?;
    let uri = params
        .pointer("/textDocument/uri")
        .and_then(Value::as_str)?;
    let line = params.pointer("/position/line").and_then(Value::as_u64)? as usize;
    let character = params
        .pointer("/position/character")
        .and_then(Value::as_u64)? as usize;

    state
        .documents
        .get(uri)
        .map(|text| (uri, text.as_str(), line, character))
}

fn source_and_position_from_range<'a>(
    state: &'a ServerState,
    params: Option<&'a Value>,
) -> Option<(&'a str, &'a str, usize, usize)> {
    let params = params?;
    let uri = params
        .pointer("/textDocument/uri")
        .and_then(Value::as_str)?;
    let line = params
        .pointer("/range/start/line")
        .and_then(Value::as_u64)? as usize;
    let character = params
        .pointer("/range/start/character")
        .and_then(Value::as_u64)? as usize;

    state
        .documents
        .get(uri)
        .map(|text| (uri, text.as_str(), line, character))
}

fn initialize_result() -> Value {
    json!({
        "capabilities": {
            "textDocumentSync": {
                "openClose": true,
                "change": 1,
                "save": true
            },
            "completionProvider": {
                "resolveProvider": false,
                "triggerCharacters": [
                    "'", "\"", ".", "(", "@", "[", ",", "$", "<", "-", ">", " ", "_",
                    "a", "b", "c", "d", "e", "f", "g", "h", "i", "j", "k", "l", "m",
                    "n", "o", "p", "q", "r", "s", "t", "u", "v", "w", "x", "y", "z",
                    "A", "B", "C", "D", "E", "F", "G", "H", "I", "J", "K", "L", "M",
                    "N", "O", "P", "Q", "R", "S", "T", "U", "V", "W", "X", "Y", "Z",
                    "0", "1", "2", "3", "4", "5", "6", "7", "8", "9"
                ]
            },
            "definitionProvider": true,
            "hoverProvider": true,
            "codeActionProvider": true,
            "documentOnTypeFormattingProvider": {
                "firstTriggerCharacter": "'",
                "moreTriggerCharacter": ["\""]
            },
            "executeCommandProvider": {
                "commands": ["rust-php.openAssetInZed"]
            },
            "diagnosticProvider": {
                "interFileDependencies": true,
                "workspaceDiagnostics": false
            }
        },
        "serverInfo": {
            "name": "rust-php",
            "version": env!("CARGO_PKG_VERSION")
        }
    })
}

fn success(id: Value, result: Value) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": result,
    })
}

fn error(id: Value, code: i64, message: &str) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": {
            "code": code,
            "message": message,
        },
    })
}

fn read_message(input: &mut impl BufRead) -> Result<Option<Value>, String> {
    let mut content_length = None;
    let mut line = String::new();

    loop {
        line.clear();
        let read = input
            .read_line(&mut line)
            .map_err(|error| error.to_string())?;
        if read == 0 {
            return Ok(None);
        }
        let trimmed = line.trim_end_matches(['\r', '\n']);
        if trimmed.is_empty() {
            break;
        }
        if let Some((name, value)) = trimmed.split_once(':') {
            if name.trim().eq_ignore_ascii_case("Content-Length") {
                content_length = value.trim().parse::<usize>().ok();
            }
        }
    }

    let content_length =
        content_length.ok_or_else(|| "missing Content-Length header".to_string())?;
    let mut body = vec![0u8; content_length];
    input
        .read_exact(&mut body)
        .map_err(|error| error.to_string())?;

    serde_json::from_slice(&body)
        .map(Some)
        .map_err(|error| error.to_string())
}

fn write_message(output: &mut impl Write, message: &Value) -> Result<(), String> {
    let body = serde_json::to_vec(message).map_err(|error| error.to_string())?;
    write!(output, "Content-Length: {}\r\n\r\n", body.len()).map_err(|error| error.to_string())?;
    output.write_all(&body).map_err(|error| error.to_string())?;
    output.flush().map_err(|error| error.to_string())
}

fn file_uri_to_path(uri: &str) -> Option<PathBuf> {
    let path = uri.strip_prefix("file://")?;
    Some(PathBuf::from(percent_decode(path)))
}

fn percent_decode(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut output = String::new();
    let mut index = 0usize;

    while index < bytes.len() {
        if bytes[index] == b'%' && index + 2 < bytes.len() {
            if let Ok(value) = u8::from_str_radix(&input[index + 1..index + 3], 16) {
                output.push(value as char);
                index += 3;
                continue;
            }
        }
        output.push(bytes[index] as char);
        index += 1;
    }

    output
}

fn rebuild_index(state: &ServerState) -> Result<ProjectIndex, String> {
    let Some(project) = &state.project else {
        return Err("no Laravel project resolved".to_string());
    };

    let overrides = collect_overrides(state, &project.root);
    ProjectIndex::build_with_overrides(project, &overrides)
}

fn collect_overrides(state: &ServerState, root: &Path) -> FileOverrides {
    let mut overrides = FileOverrides::default();

    for (uri, text) in &state.documents {
        let Some(path) = file_uri_to_path(uri) else {
            continue;
        };
        if !path_affects_index(root, &path) {
            continue;
        }
        overrides.insert(path, text.clone());
    }

    overrides
}

fn uri_affects_index(state: &ServerState, uri: &str) -> bool {
    let Some(project) = state.project.as_ref() else {
        return false;
    };
    let Some(path) = file_uri_to_path(uri) else {
        return false;
    };

    path_affects_index(project.root.as_path(), &path)
}

fn document_is_reindexable(state: &ServerState, uri: &str) -> bool {
    let Some(source) = state.documents.get(uri) else {
        return true;
    };

    if let Some(reason) = route_reindex_guard_reason(uri, source) {
        log_lsp_event(format!(
            "reindex guard uri={uri} kind=route-parser reason={reason}"
        ));
        return false;
    }

    !php_document_has_parse_errors(uri, source)
}

fn route_reindex_guard_reason(uri: &str, source: &str) -> Option<&'static str> {
    if !uri.ends_with(".php") || !uri.contains("/routes/") {
        return None;
    }

    routes::reindex_guard_reason(source.as_bytes())
}

fn php_document_has_parse_errors(uri: &str, source: &str) -> bool {
    !php_document_parse_errors(uri, source).is_empty()
}

fn php_document_parse_errors(uri: &str, source: &str) -> Vec<String> {
    if !uri.ends_with(".php") || uri.ends_with(".blade.php") {
        return Vec::new();
    }

    let arena = Bump::new();
    let lexer = Lexer::new(source.as_bytes());
    let mut parser = Parser::new(lexer, &arena);
    let program = parser.parse_program();
    let path = file_uri_to_path(uri)
        .unwrap_or_else(|| PathBuf::from(uri))
        .display()
        .to_string();

    program
        .errors
        .iter()
        .map(|error| error.to_human_readable_with_path(source.as_bytes(), Some(&path)))
        .collect()
}

fn log_php_parse_errors(uri: &str, source: &str, phase: &str) {
    let errors = php_document_parse_errors(uri, source);
    if errors.is_empty() {
        return;
    }

    log_lsp_event(format!(
        "parse-errors phase={phase} uri={uri} count={}",
        errors.len()
    ));
    for (index, error) in errors.iter().enumerate() {
        log_lsp_event(format!(
            "parse-error phase={phase} uri={uri} index={} detail=\n{}",
            index + 1,
            error
        ));
    }
}

fn reindex_for_uri(state: &mut ServerState, _uri: &str) {
    let start = Instant::now();
    log_lsp_event(format!("reindex start uri={_uri}"));

    match rebuild_index(state) {
        Ok(index) => {
            let elapsed = start.elapsed().as_millis();
            log_lsp_event(format!("reindex ok uri={_uri} elapsed_ms={elapsed}"));
            state.index = Some(index);
        }
        Err(error) => {
            let elapsed = start.elapsed().as_millis();
            log_lsp_event(format!(
                "reindex error uri={_uri} elapsed_ms={elapsed} error={error}"
            ));
        }
    }
}

fn log_lsp_event(message: String) {
    let path = std::env::var_os("RUST_PHP_LSP_LOG")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/tmp/rust-php-lsp.log"));
    let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) else {
        return;
    };
    let _ = writeln!(file, "[rust-php:lsp] {message}");
}

fn open_in_zed(path: &Path) -> Result<(), String> {
    if !path.exists() {
        log_lsp_event(format!("openInZed missing path={}", path.display()));
        return Err(format!("asset path does not exist: {}", path.display()));
    }

    log_lsp_event(format!(
        "openInZed trying command=zed path={}",
        path.display()
    ));
    match Command::new("zed").arg(path).spawn() {
        Ok(_) => {
            log_lsp_event(format!("openInZed ok command=zed path={}", path.display()));
            return Ok(());
        }
        Err(error) => {
            log_lsp_event(format!(
                "openInZed failed command=zed path={} error={}",
                path.display(),
                error
            ));
        }
    }

    #[cfg(target_os = "macos")]
    {
        log_lsp_event(format!(
            "openInZed trying command=open -a Zed path={}",
            path.display()
        ));
        if Command::new("open")
            .args(["-a", "Zed"])
            .arg(path)
            .spawn()
            .is_ok()
        {
            log_lsp_event(format!(
                "openInZed ok command=open -a Zed path={}",
                path.display()
            ));
            return Ok(());
        }
        log_lsp_event(format!(
            "openInZed failed command=open -a Zed path={}",
            path.display()
        ));
    }

    log_lsp_event(format!("openInZed giving-up path={}", path.display()));
    Err("failed to launch Zed for asset path".to_string())
}

#[cfg(test)]
mod tests {
    use std::collections::{HashMap, HashSet};
    use std::path::{Path, PathBuf};

    use serde_json::json;

    use crate::lsp::index::ProjectIndex;
    use crate::lsp::overrides::FileOverrides;
    use crate::project;

    use super::{
        ServerState, completion_result, diagnostic_result, handle_message, initialize_result,
        path_affects_index, php_document_has_parse_errors, php_document_parse_errors,
        route_reindex_guard_reason,
    };

    fn workspace_root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(Path::parent)
            .expect("lsp crate should be under crates/")
            .to_path_buf()
    }

    fn sandbox_project() -> project::LaravelProject {
        let root = workspace_root().join("laravel-example").join("sandbox-app");
        project::from_root(root).expect("sandbox project should resolve")
    }

    #[test]
    fn skips_diagnostics_for_dirty_documents() {
        let project = sandbox_project();
        let index = ProjectIndex::build_with_overrides(&project, &FileOverrides::default())
            .expect("index should build");
        let uri = format!(
            "file://{}",
            project.root.join("routes/starter.php").display()
        );
        let source = std::fs::read_to_string(project.root.join("routes/starter.php"))
            .expect("starter route file should load");

        let mut state = ServerState {
            project_root: Some(project.root.clone()),
            project: Some(project),
            index: Some(index),
            documents: HashMap::from([(uri.clone(), source)]),
            dirty_documents: HashSet::from([uri.clone()]),
            parse_failed_documents: HashSet::new(),
            shutdown_requested: false,
            exiting: false,
        };

        let result = diagnostic_result(
            &state,
            Some(&json!({
                "textDocument": { "uri": uri }
            })),
        );

        assert_eq!(
            result
                .pointer("/items")
                .and_then(|v| v.as_array())
                .map(Vec::len),
            Some(0)
        );

        state.dirty_documents.clear();
        let saved = diagnostic_result(
            &state,
            Some(&json!({
                "textDocument": { "uri": uri }
            })),
        );
        assert!(
            saved
                .pointer("/items")
                .and_then(|v| v.as_array())
                .map(|items| !items.is_empty())
                .unwrap_or(false)
        );
    }

    #[test]
    fn keeps_symbol_completion_lists_live_for_retriggering() {
        let project = sandbox_project();
        let index = ProjectIndex::build_with_overrides(&project, &FileOverrides::default())
            .expect("index should build");
        let uri = format!(
            "file://{}",
            project
                .root
                .join("app/Http/Controllers/WebsiteController.php")
                .display()
        );
        let source = "<?php\n\nreturn route('health');\n".to_string();
        let character = source
            .lines()
            .nth(2)
            .and_then(|line| line.find("health"))
            .expect("route token should exist")
            + "hea".len();

        let state = ServerState {
            project_root: Some(project.root.clone()),
            project: Some(project),
            index: Some(index),
            documents: HashMap::from([(uri.clone(), source)]),
            dirty_documents: HashSet::new(),
            parse_failed_documents: HashSet::new(),
            shutdown_requested: false,
            exiting: false,
        };

        let result = completion_result(
            &state,
            Some(&json!({
                "textDocument": { "uri": uri },
                "position": { "line": 2, "character": character }
            })),
        );

        assert_eq!(
            result.get("isIncomplete").and_then(|value| value.as_bool()),
            Some(true)
        );
        assert!(
            result
                .pointer("/items")
                .and_then(|value| value.as_array())
                .map(|items| !items.is_empty())
                .unwrap_or(false)
        );
    }

    #[test]
    fn completes_local_view_variables_inside_compact_strings() {
        let project = sandbox_project();
        let index = ProjectIndex::build_with_overrides(&project, &FileOverrides::default())
            .expect("index should build");
        let uri = format!(
            "file://{}",
            project
                .root
                .join("app/Http/Controllers/BladeSandboxController.php")
                .display()
        );
        let source = std::fs::read_to_string(
            project
                .root
                .join("app/Http/Controllers/BladeSandboxController.php"),
        )
        .expect("controller should load");
        let source = source.replace(
            "compact('pageTitle', 'currentUser', 'orders', 'filters')",
            "compact('')",
        );
        let line_index = source
            .lines()
            .position(|line| line.contains("compact('')"))
            .expect("compact line should exist");
        let line_text = source.lines().nth(line_index).expect("line should exist");
        let character = line_text.find("''").expect("compact token should exist") + 1;

        let state = ServerState {
            project_root: Some(project.root.clone()),
            project: Some(project),
            index: Some(index),
            documents: HashMap::from([(uri.clone(), source)]),
            dirty_documents: HashSet::new(),
            parse_failed_documents: HashSet::new(),
            shutdown_requested: false,
            exiting: false,
        };

        let result = completion_result(
            &state,
            Some(&json!({
                "textDocument": { "uri": uri },
                "position": { "line": line_index, "character": character }
            })),
        );

        let labels = result
            .pointer("/items")
            .and_then(|value| value.as_array())
            .into_iter()
            .flatten()
            .filter_map(|item| item.get("label").and_then(|value| value.as_str()))
            .collect::<Vec<_>>();

        assert!(labels.contains(&"currentUser"));
        assert!(labels.contains(&"pageTitle"));
        assert!(labels.contains(&"orders"));
        assert!(labels.contains(&"filters"));
        assert!(labels.contains(&"stats"));
        assert!(labels.contains(&"teamMembers"));
        assert!(labels.contains(&"breadcrumbs"));
        assert!(labels.contains(&"flashMessage"));
        assert!(labels.contains(&"internalAuditLog"));
        assert!(labels.contains(&"draftInvoice"));
    }

    #[test]
    fn completes_local_view_variables_inside_compact_array_strings() {
        let project = sandbox_project();
        let index = ProjectIndex::build_with_overrides(&project, &FileOverrides::default())
            .expect("index should build");
        let uri = format!(
            "file://{}",
            project
                .root
                .join("app/Http/Controllers/BladeSandboxController.php")
                .display()
        );
        let source = std::fs::read_to_string(
            project
                .root
                .join("app/Http/Controllers/BladeSandboxController.php"),
        )
        .expect("controller should load");
        let source = source.replace(
            "compact('pageTitle', 'currentUser', 'orders', 'filters')",
            "compact([''])",
        );
        let line_index = source
            .lines()
            .position(|line| line.contains("compact([''])"))
            .expect("compact line should exist");
        let line_text = source.lines().nth(line_index).expect("line should exist");
        let character = line_text.find("''").expect("compact token should exist") + 1;

        let state = ServerState {
            project_root: Some(project.root.clone()),
            project: Some(project),
            index: Some(index),
            documents: HashMap::from([(uri.clone(), source)]),
            dirty_documents: HashSet::new(),
            parse_failed_documents: HashSet::new(),
            shutdown_requested: false,
            exiting: false,
        };

        let result = completion_result(
            &state,
            Some(&json!({
                "textDocument": { "uri": uri },
                "position": { "line": line_index, "character": character }
            })),
        );

        let labels = result
            .pointer("/items")
            .and_then(|value| value.as_array())
            .into_iter()
            .flatten()
            .filter_map(|item| item.get("label").and_then(|value| value.as_str()))
            .collect::<Vec<_>>();

        assert!(labels.contains(&"currentUser"));
        assert!(labels.contains(&"pageTitle"));
        assert!(labels.contains(&"orders"));
        assert!(labels.contains(&"filters"));
        assert!(labels.contains(&"stats"));
        assert!(labels.contains(&"teamMembers"));
        assert!(labels.contains(&"breadcrumbs"));
        assert!(labels.contains(&"flashMessage"));
        assert!(labels.contains(&"internalAuditLog"));
        assert!(labels.contains(&"draftInvoice"));
    }

    #[test]
    fn completes_blade_view_variables_from_lsp_request() {
        let project = sandbox_project();
        let index = ProjectIndex::build_with_overrides(&project, &FileOverrides::default())
            .expect("index should build");
        let uri = format!(
            "file://{}",
            project
                .root
                .join("resources/views/ide-lab/orders.blade.php")
                .display()
        );
        let source =
            "<div>\n    {{ $ }}\n    @php\n        $ord\n    @endphp\n</div>\n".to_string();
        let line_index = source
            .lines()
            .position(|line| line.contains("{{ $ }}"))
            .expect("blade echo line should exist");
        let line_text = source.lines().nth(line_index).expect("line should exist");
        let character = line_text.find("$ ").expect("dollar should exist") + 1;

        let state = ServerState {
            project_root: Some(project.root.clone()),
            project: Some(project),
            index: Some(index),
            documents: HashMap::from([(uri.clone(), source)]),
            dirty_documents: HashSet::new(),
            parse_failed_documents: HashSet::new(),
            shutdown_requested: false,
            exiting: false,
        };

        let result = completion_result(
            &state,
            Some(&json!({
                "textDocument": { "uri": uri },
                "position": { "line": line_index, "character": character }
            })),
        );

        let labels = result
            .pointer("/items")
            .and_then(|value| value.as_array())
            .into_iter()
            .flatten()
            .filter_map(|item| item.get("label").and_then(|value| value.as_str()))
            .collect::<Vec<_>>();

        assert!(labels.contains(&"$orders"));
        assert!(labels.contains(&"$filters"));
    }

    #[test]
    fn treats_blade_views_as_index_relevant() {
        let root = Path::new("/tmp/example");

        assert!(path_affects_index(
            root,
            &root.join("resources/views/home.blade.php")
        ));
        assert!(path_affects_index(
            root,
            &root.join("resources/views/vendor/app/layout.blade.php")
        ));
        assert!(!path_affects_index(root, &root.join("README.md")));
    }

    #[test]
    fn detects_php_parse_errors_before_reindexing() {
        assert!(php_document_has_parse_errors(
            "file:///tmp/routes/web.php",
            "<?php\nRoute::get('/', fn() => ;\n"
        ));
        let errors = php_document_parse_errors(
            "file:///tmp/routes/web.php",
            "<?php\nRoute::get('/', fn() => ;\n",
        );
        assert!(!errors.is_empty());
        assert!(errors[0].contains("error:"));
        assert!(errors[0].contains("/tmp/routes/web.php:2:"));
        assert!(!php_document_has_parse_errors(
            "file:///tmp/routes/web.php",
            "<?php\nRoute::get('/', fn() => 'ok');\n"
        ));
        assert!(!php_document_has_parse_errors(
            "file:///tmp/resources/views/home.blade.php",
            "@if($x)\n<div>{{ $x }}</div>\n"
        ));
    }

    #[test]
    fn did_save_with_parse_errors_skips_reindex_and_diagnostics() {
        let project = sandbox_project();
        let index = ProjectIndex::build_with_overrides(&project, &FileOverrides::default())
            .expect("index should build");
        let uri = format!(
            "file://{}",
            project.root.join("routes/starter.php").display()
        );
        let broken = "<?php\nRoute::get('/', fn() => ;\n".to_string();

        let mut state = ServerState {
            project_root: Some(project.root.clone()),
            project: Some(project),
            index: Some(index),
            documents: HashMap::from([(uri.clone(), broken)]),
            dirty_documents: HashSet::from([uri.clone()]),
            parse_failed_documents: HashSet::new(),
            shutdown_requested: false,
            exiting: false,
        };

        let response = handle_message(
            &mut state,
            json!({
                "method": "textDocument/didSave",
                "params": { "textDocument": { "uri": uri.clone() } }
            }),
        )
        .expect("didSave should succeed");

        assert!(response.is_none());
        assert!(!state.dirty_documents.contains(&uri));
        assert!(state.parse_failed_documents.contains(&uri));
        assert!(state.index.is_some());

        let diagnostics = diagnostic_result(
            &state,
            Some(&json!({
                "textDocument": { "uri": uri }
            })),
        );

        assert_eq!(
            diagnostics
                .pointer("/items")
                .and_then(|value| value.as_array())
                .map(Vec::len),
            Some(0)
        );
    }

    #[test]
    fn route_action_quote_wedge_is_blocked_before_reindex() {
        let uri = "file:///tmp/project/routes/web.php";
        let broken = "<?php\nRoute::get('/', [ManageOfficeController::class, 'sdsd'sd']);\n";

        assert_eq!(
            route_reindex_guard_reason(uri, broken),
            Some("unsafe-string-adjacency")
        );
        assert!(php_document_has_parse_errors(uri, broken));
    }

    #[test]
    fn completion_triggers_include_arrow_close() {
        let result = initialize_result();
        let triggers = result
            .pointer("/capabilities/completionProvider/triggerCharacters")
            .and_then(|value| value.as_array())
            .expect("trigger characters should exist");

        assert!(
            triggers.iter().any(|value| value.as_str() == Some(">")),
            "completion should trigger when `->` is completed",
        );
    }

    #[test]
    fn completion_triggers_include_identifier_characters() {
        let result = initialize_result();
        let triggers = result
            .pointer("/capabilities/completionProvider/triggerCharacters")
            .and_then(|value| value.as_array())
            .expect("trigger characters should exist");

        for ch in ["a", "Z", "_", "7"] {
            assert!(
                triggers.iter().any(|value| value.as_str() == Some(ch)),
                "completion should retrigger while editing identifier strings: {ch}",
            );
        }
    }
}

fn path_affects_index(root: &Path, path: &Path) -> bool {
    if !path.starts_with(root) {
        return false;
    }

    let Ok(relative) = path.strip_prefix(root) else {
        return false;
    };

    if relative.starts_with("routes")
        || relative.starts_with("config")
        || relative.starts_with("app")
        || relative.starts_with("packages")
    {
        return path.extension().and_then(|ext| ext.to_str()) == Some("php");
    }

    if relative.starts_with(Path::new("resources/views")) {
        return path.extension().and_then(|ext| ext.to_str()) == Some("php");
    }

    if relative == Path::new("bootstrap/app.php")
        || relative == Path::new("bootstrap/providers.php")
        || relative == Path::new("composer.json")
        || relative == Path::new(".env")
        || relative == Path::new(".env.example")
    {
        return true;
    }

    relative.starts_with(Path::new("app/Providers"))
        && path.extension().and_then(|ext| ext.to_str()) == Some("php")
}
