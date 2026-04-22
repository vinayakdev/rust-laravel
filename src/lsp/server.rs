use std::collections::{HashMap, HashSet};
use std::fs::OpenOptions;
use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};
use std::time::Instant;

use serde_json::{Value, json};

use super::context::{
    detect_blade_variable_context, detect_helper_context, detect_route_action_context,
    detect_symbol_context, detect_view_data_context,
};
use super::index::ProjectIndex;
use super::overrides::FileOverrides;
use super::query;
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
                    log_lsp_event(format!("didOpen uri={uri} bytes={}", text.len()));
                    reindex_for_uri(state, uri);
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
                reindex_for_uri(state, uri);
            }
            Ok(None)
        }
        Some("textDocument/didClose") => {
            if let Some(uri) = message
                .pointer("/params/textDocument/uri")
                .and_then(Value::as_str)
            {
                state.documents.remove(uri);
                state.dirty_documents.remove(uri);
                log_lsp_event(format!("didClose uri={uri}"));
                reindex_for_uri(state, uri);
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
        detect_view_data_context(uri, source, line, character)
    {
        log_lsp_event(format!(
            "completion uri={uri} line={} char={} context=view-data prefix={:?}",
            line, character, context.prefix
        ));
        (
            query::complete_view_data_variables(source, &context, line),
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

    let definitions = if let Some(context) = detect_symbol_context(source, line, character) {
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

    if let Some(context) = detect_symbol_context(source, line, character) {
        return query::hover(index, &context, line).unwrap_or(Value::Null);
    }
    if let Some(context) = detect_route_action_context(uri, source, line, character) {
        return query::route_action_hover(index, &context, line).unwrap_or(Value::Null);
    }

    Value::Null
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
    let Some(index) = &state.index else {
        return Value::Array(Vec::new());
    };
    let diagnostics = params
        .pointer("/context/diagnostics")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();

    Value::Array(query::route_action_code_actions(index, &diagnostics))
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
                "triggerCharacters": ["'", "\"", ".", "(", "@", "[", ",", "$"]
            },
            "definitionProvider": true,
            "hoverProvider": true,
            "codeActionProvider": true,
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

#[cfg(test)]
mod tests {
    use std::collections::{HashMap, HashSet};
    use std::path::Path;

    use serde_json::json;

    use crate::lsp::index::ProjectIndex;
    use crate::lsp::overrides::FileOverrides;
    use crate::project;

    use super::{ServerState, completion_result, diagnostic_result};

    fn sandbox_project() -> project::LaravelProject {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("laravel-example")
            .join("sandbox-app");
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
