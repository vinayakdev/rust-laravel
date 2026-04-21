use std::collections::{BTreeMap, BTreeSet};
use std::process::Command as ProcessCommand;
use std::time::Instant;

use serde::Deserialize;
use serde::Serialize;
use serde_json::{Value, json};

use crate::analyzers;
use crate::output::text;
use crate::project::LaravelProject;
use crate::types::RouteEntry;

use super::command::DebugCommand;

pub(crate) fn render_text_report(
    project: &LaravelProject,
    command: DebugCommand,
) -> Result<String, String> {
    match command {
        DebugCommand::RouteList => {
            let report = analyzers::routes::analyze(project)?;
            Ok(text::routes::render_route_table(&report.routes))
        }
        DebugCommand::RouteSources => {
            let report = analyzers::routes::analyze(project)?;
            Ok(text::routes::render_route_source_table(&report.routes))
        }
        DebugCommand::MiddlewareList => {
            let report = analyzers::middleware::analyze(project)?;
            Ok(text::middleware::render_middleware_tables(&report))
        }
        DebugCommand::ConfigList => {
            let report = analyzers::configs::analyze(project)?;
            Ok(text::configs::render_config_table(&report))
        }
        DebugCommand::ConfigSources => {
            let report = analyzers::configs::analyze(project)?;
            Ok(text::configs::render_config_source_table(&report))
        }
        DebugCommand::ControllerList => {
            let report = analyzers::controllers::analyze(project)?;
            Ok(text::controllers::render_controller_report(&report))
        }
        DebugCommand::ProviderList => {
            let report = analyzers::providers::analyze(project)?;
            Ok(text::providers::render_provider_table(&report))
        }
        DebugCommand::ViewList => {
            let report = analyzers::views::analyze(project)?;
            Ok(text::views::render_view_report(&report))
        }
        DebugCommand::ModelList => {
            let report = analyzers::models::analyze(project)?;
            Ok(text::models::render_model_report(&report))
        }
        DebugCommand::MigrationList => {
            let report = analyzers::migrations::analyze(project)?;
            Ok(text::models::render_migration_report(&report))
        }
        DebugCommand::RouteCompare => Err(
            "route:compare is only available in the web debugger because it needs structured JSON output"
                .to_string(),
        ),
    }
}

pub(crate) fn render_json_report(
    project: &LaravelProject,
    command: DebugCommand,
) -> Result<String, String> {
    let started_at = Instant::now();
    let rss_before_kb = current_rss_kb();

    let payload = match command {
        DebugCommand::RouteList => {
            let report = analyzers::routes::analyze(project)?;
            json_payload(project, command, json!({ "report": report }))
        }
        DebugCommand::RouteSources => {
            let report = analyzers::routes::analyze(project)?;
            json_payload(project, command, json!({ "report": report }))
        }
        DebugCommand::MiddlewareList => {
            let report = analyzers::middleware::analyze(project)?;
            json_payload(project, command, json!({ "report": report }))
        }
        DebugCommand::ConfigList => {
            let report = analyzers::configs::analyze(project)?;
            json_payload(project, command, json!({ "report": report }))
        }
        DebugCommand::ConfigSources => {
            let report = analyzers::configs::analyze(project)?;
            json_payload(project, command, json!({ "report": report }))
        }
        DebugCommand::ControllerList => {
            let report = analyzers::controllers::analyze(project)?;
            json_payload(project, command, json!({ "report": report }))
        }
        DebugCommand::ProviderList => {
            let report = analyzers::providers::analyze(project)?;
            json_payload(project, command, json!({ "report": report }))
        }
        DebugCommand::ViewList => {
            let report = analyzers::views::analyze(project)?;
            json_payload(project, command, json!({ "report": report }))
        }
        DebugCommand::ModelList => {
            let report = analyzers::models::analyze(project)?;
            json_payload(project, command, json!({ "report": report }))
        }
        DebugCommand::MigrationList => {
            let report = analyzers::migrations::analyze(project)?;
            json_payload(project, command, json!({ "report": report }))
        }
        DebugCommand::RouteCompare => {
            let report = analyzers::routes::analyze(project)?;
            json_payload(
                project,
                command,
                json!({ "comparison": compare_routes(project, &report.routes)? }),
            )
        }
    };

    let debug = DebugInfo {
        duration_ms: started_at.elapsed().as_millis(),
        parsed_file_count: collect_debug_paths(&payload).len(),
        rss_before_kb,
        rss_after_kb: current_rss_kb(),
    };

    serde_json::to_string(&attach_debug(payload, debug)).map_err(|error| error.to_string())
}

fn json_payload(project: &LaravelProject, command: DebugCommand, body: Value) -> Value {
    let mut payload = json!({
        "project": project.name,
        "root": project.root,
        "command": command.label(),
    });

    if let Value::Object(object) = body {
        for (key, value) in object {
            payload[key] = value;
        }
    }

    payload
}

#[derive(Debug, Serialize)]
struct DebugInfo {
    duration_ms: u128,
    parsed_file_count: usize,
    rss_before_kb: Option<u64>,
    rss_after_kb: Option<u64>,
}

fn attach_debug(mut payload: Value, debug: DebugInfo) -> Value {
    if let Value::Object(ref mut object) = payload {
        object.insert(
            "debug".to_string(),
            serde_json::to_value(debug).unwrap_or(Value::Null),
        );
    }
    payload
}

fn current_rss_kb() -> Option<u64> {
    let output = ProcessCommand::new("ps")
        .args(["-o", "rss=", "-p", &std::process::id().to_string()])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    String::from_utf8(output.stdout)
        .ok()?
        .trim()
        .parse::<u64>()
        .ok()
}

fn collect_debug_paths(value: &Value) -> BTreeSet<String> {
    let mut paths = BTreeSet::new();
    collect_debug_paths_inner(None, value, &mut paths);
    paths
}

fn collect_debug_paths_inner(key: Option<&str>, value: &Value, paths: &mut BTreeSet<String>) {
    match value {
        Value::Object(object) => {
            for (child_key, child_value) in object {
                collect_debug_paths_inner(Some(child_key.as_str()), child_value, paths);
            }
        }
        Value::Array(items) => {
            for item in items {
                collect_debug_paths_inner(key, item, paths);
            }
        }
        Value::String(text) => {
            if is_debug_path_key(key) {
                let trimmed = text.trim();
                if !trimmed.is_empty() {
                    paths.insert(trimmed.to_string());
                }
            }
        }
        _ => {}
    }
}

fn is_debug_path_key(key: Option<&str>) -> bool {
    matches!(
        key,
        Some(
            "root"
                | "file"
                | "declared_in"
                | "source_file"
                | "class_file"
                | "view_file"
                | "expected_file"
                | "artisan_path"
        )
    )
}

#[derive(Serialize)]
struct RouteComparisonPayload {
    runtime_count: usize,
    analyzer_count: usize,
    matched_count: usize,
    runtime_only_count: usize,
    analyzer_only_count: usize,
    runnable: bool,
    artisan_path: Option<String>,
    note: String,
    matched: Vec<ComparedRoute>,
    runtime_only: Vec<ComparedRoute>,
    analyzer_only: Vec<ComparedRoute>,
}

#[derive(Clone, Serialize)]
struct ComparedRoute {
    key: String,
    methods: Vec<String>,
    uri: String,
    name: Option<String>,
    action: Option<String>,
    source: Option<String>,
    middleware: Vec<String>,
}

fn compare_routes(
    project: &LaravelProject,
    analyzer_routes: &[RouteEntry],
) -> Result<RouteComparisonPayload, String> {
    let artisan_path = project.root.join("artisan");
    if !artisan_path.is_file() {
        return Ok(RouteComparisonPayload {
            runtime_count: 0,
            analyzer_count: analyzer_routes.len(),
            matched_count: 0,
            runtime_only_count: 0,
            analyzer_only_count: analyzer_routes.len(),
            runnable: false,
            artisan_path: None,
            note:
                "This project does not have an artisan file, so runtime comparison is unavailable."
                    .to_string(),
            matched: Vec::new(),
            runtime_only: analyzer_routes.iter().map(compared_from_analyzer).collect(),
            analyzer_only: Vec::new(),
        });
    }

    let output = ProcessCommand::new("php")
        .arg("artisan")
        .arg("route:list")
        .arg("--json")
        .current_dir(&project.root)
        .output()
        .map_err(|error| format!("failed to run php artisan route:list --json: {error}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Ok(RouteComparisonPayload {
            runtime_count: 0,
            analyzer_count: analyzer_routes.len(),
            matched_count: 0,
            runtime_only_count: 0,
            analyzer_only_count: analyzer_routes.len(),
            runnable: false,
            artisan_path: Some(artisan_path.display().to_string()),
            note: if stderr.is_empty() {
                "Artisan route:list failed for this project.".to_string()
            } else {
                format!("Artisan route:list failed: {stderr}")
            },
            matched: Vec::new(),
            runtime_only: Vec::new(),
            analyzer_only: analyzer_routes.iter().map(compared_from_analyzer).collect(),
        });
    }

    let runtime_routes: Vec<ArtisanRoute> = serde_json::from_slice(&output.stdout)
        .map_err(|error| format!("failed to parse artisan route:list --json output: {error}"))?;

    let mut runtime_map: BTreeMap<String, ComparedRoute> = BTreeMap::new();
    let mut analyzer_map: BTreeMap<String, ComparedRoute> = BTreeMap::new();

    for route in runtime_routes {
        let compared = compared_from_runtime(route);
        runtime_map.entry(compared.key.clone()).or_insert(compared);
    }

    for route in analyzer_routes {
        let compared = compared_from_analyzer(route);
        analyzer_map.entry(compared.key.clone()).or_insert(compared);
    }

    let runtime_keys = runtime_map.keys().cloned().collect::<BTreeSet<_>>();
    let analyzer_keys = analyzer_map.keys().cloned().collect::<BTreeSet<_>>();

    let matched_keys = runtime_keys
        .intersection(&analyzer_keys)
        .cloned()
        .collect::<Vec<_>>();
    let runtime_only_keys = runtime_keys
        .difference(&analyzer_keys)
        .cloned()
        .collect::<Vec<_>>();
    let analyzer_only_keys = analyzer_keys
        .difference(&runtime_keys)
        .cloned()
        .collect::<Vec<_>>();

    Ok(RouteComparisonPayload {
        runtime_count: runtime_map.len(),
        analyzer_count: analyzer_map.len(),
        matched_count: matched_keys.len(),
        runtime_only_count: runtime_only_keys.len(),
        analyzer_only_count: analyzer_only_keys.len(),
        runnable: true,
        artisan_path: Some(artisan_path.display().to_string()),
        note: "Runtime route list comes from `php artisan route:list --json` and is compared against normalized analyzer routes by method + URI + name.".to_string(),
        matched: matched_keys
            .into_iter()
            .filter_map(|key| runtime_map.get(&key).cloned())
            .collect(),
        runtime_only: runtime_only_keys
            .into_iter()
            .filter_map(|key| runtime_map.get(&key).cloned())
            .collect(),
        analyzer_only: analyzer_only_keys
            .into_iter()
            .filter_map(|key| analyzer_map.get(&key).cloned())
            .collect(),
    })
}

#[derive(Deserialize)]
struct ArtisanRoute {
    method: String,
    uri: String,
    name: Option<String>,
    action: Option<String>,
    middleware: Vec<String>,
    path: Option<String>,
}

fn compared_from_runtime(route: ArtisanRoute) -> ComparedRoute {
    let methods = normalize_runtime_methods(&route.method);
    let uri = normalize_uri(&route.uri);
    let name = route.name.filter(|value| !value.is_empty());

    ComparedRoute {
        key: route_compare_key(&methods, &uri, name.as_deref()),
        methods,
        uri,
        name,
        action: route.action.filter(|value| !value.is_empty()),
        source: route.path,
        middleware: route.middleware,
    }
}

fn compared_from_analyzer(route: &RouteEntry) -> ComparedRoute {
    let methods = normalize_methods(&route.methods);
    let uri = normalize_uri(&route.uri);
    let name = route.name.clone().filter(|value| !value.is_empty());

    ComparedRoute {
        key: route_compare_key(&methods, &uri, name.as_deref()),
        methods,
        uri,
        name,
        action: route.action.clone().filter(|value| !value.is_empty()),
        source: Some(format!(
            "{}:{}:{}",
            route.file.display(),
            route.line,
            route.column
        )),
        middleware: if route.resolved_middleware.is_empty() {
            route.middleware.clone()
        } else {
            route.resolved_middleware.clone()
        },
    }
}

fn normalize_runtime_methods(methods: &str) -> Vec<String> {
    methods
        .split('|')
        .map(str::trim)
        .filter(|value| !value.is_empty() && *value != "HEAD")
        .map(|value| value.to_ascii_uppercase())
        .collect()
}

fn normalize_methods(methods: &[String]) -> Vec<String> {
    methods
        .iter()
        .map(|value| value.trim().to_ascii_uppercase())
        .filter(|value| value != "HEAD")
        .collect()
}

fn normalize_uri(uri: &str) -> String {
    if uri == "/" {
        "/".to_string()
    } else {
        format!("/{}", uri.trim_matches('/'))
    }
}

fn route_compare_key(methods: &[String], uri: &str, name: Option<&str>) -> String {
    format!("{} {} {}", methods.join("|"), uri, name.unwrap_or("-"))
}

pub(crate) fn error_json(message: &str) -> String {
    serde_json::to_string(&json!({ "error": message }))
        .unwrap_or_else(|_| "{\"error\":\"internal serialization failure\"}".to_string())
}
