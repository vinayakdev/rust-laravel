use std::collections::BTreeSet;
use std::path::Path;
use std::process::Command as ProcessCommand;
use std::time::Instant;

use rust_php_configs::types::ConfigReport;
use rust_php_controllers::types::ControllerReport;
use rust_php_foundation::types::ProviderReport;
use rust_php_middleware::types::MiddlewareReport;
use rust_php_migrations::types::MigrationReport;
use rust_php_models::types::ModelReport;
use rust_php_public::types::PublicAssetReport;
use rust_php_routes::types::RouteReport;
use rust_php_views::types::ViewReport;
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
        DebugCommand::Dashboard => Err(
            "dashboard is only available in the web debugger because it needs structured JSON output"
                .to_string(),
        ),
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
        DebugCommand::PublicList => {
            let report = analyzers::public_assets::analyze(project)?;
            Ok(text::public_assets::render_public_asset_report(&report))
        }
        DebugCommand::RouteCompare => Err(
            "route:compare is only available in the web debugger because it needs structured JSON output"
                .to_string(),
        ),
        DebugCommand::VendorList => Err(
            "vendor:list is only available in the web debugger".to_string(),
        ),
    }
}

pub(crate) fn render_json_report(
    project: &LaravelProject,
    command: DebugCommand,
) -> Result<String, String> {
    let started_at = Instant::now();
    let rss_before_kb = current_rss_kb();

    let (payload, parsed_file_count_override) = match command {
        DebugCommand::Dashboard => {
            let report = build_dashboard_report(project)?;
            let parsed_file_count = report.summary.total_files_scanned;
            (
                json_payload(project, command, json!({ "report": report })),
                Some(parsed_file_count),
            )
        }
        DebugCommand::RouteList => {
            let report = analyzers::routes::analyze(project)?;
            (
                json_payload(project, command, json!({ "report": report })),
                None,
            )
        }
        DebugCommand::RouteSources => {
            let report = analyzers::routes::analyze(project)?;
            (
                json_payload(project, command, json!({ "report": report })),
                None,
            )
        }
        DebugCommand::MiddlewareList => {
            let report = analyzers::middleware::analyze(project)?;
            (
                json_payload(project, command, json!({ "report": report })),
                None,
            )
        }
        DebugCommand::ConfigList => {
            let report = analyzers::configs::analyze(project)?;
            (
                json_payload(project, command, json!({ "report": report })),
                None,
            )
        }
        DebugCommand::ConfigSources => {
            let report = analyzers::configs::analyze(project)?;
            (
                json_payload(project, command, json!({ "report": report })),
                None,
            )
        }
        DebugCommand::ControllerList => {
            let report = analyzers::controllers::analyze(project)?;
            (
                json_payload(project, command, json!({ "report": report })),
                None,
            )
        }
        DebugCommand::ProviderList => {
            let report = analyzers::providers::analyze(project)?;
            (
                json_payload(project, command, json!({ "report": report })),
                None,
            )
        }
        DebugCommand::ViewList => {
            let report = analyzers::views::analyze(project)?;
            (
                json_payload(project, command, json!({ "report": report })),
                None,
            )
        }
        DebugCommand::ModelList => {
            let report = analyzers::models::analyze(project)?;
            (
                json_payload(project, command, json!({ "report": report })),
                None,
            )
        }
        DebugCommand::MigrationList => {
            let report = analyzers::migrations::analyze(project)?;
            (
                json_payload(project, command, json!({ "report": report })),
                None,
            )
        }
        DebugCommand::PublicList => {
            let report = analyzers::public_assets::analyze(project)?;
            (
                json_payload(project, command, json!({ "report": report })),
                None,
            )
        }
        DebugCommand::RouteCompare => {
            let report = analyzers::routes::analyze(project)?;
            (
                json_payload(
                    project,
                    command,
                    json!({ "comparison": compare_routes(project, &report.routes)? }),
                ),
                None,
            )
        }
        DebugCommand::VendorList => {
            let classes = crate::vendor::list_vendor_classes(&project.root);
            let count = classes.len();
            (
                json_payload(
                    project,
                    command,
                    json!({ "report": { "class_count": count, "classes": classes } }),
                ),
                Some(count),
            )
        }
    };

    let debug = DebugInfo {
        duration_ms: started_at.elapsed().as_millis(),
        parsed_file_count: parsed_file_count_override
            .unwrap_or_else(|| collect_debug_paths(&payload).len()),
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

#[derive(Debug, Serialize)]
struct DashboardReport {
    summary: DashboardSummary,
    features: Vec<DashboardFeature>,
}

#[derive(Debug, Serialize)]
struct DashboardSummary {
    feature_count: usize,
    total_files_scanned: usize,
    total_items_found: usize,
    total_autocomplete_suggestions: usize,
}

#[derive(Debug, Serialize)]
struct DashboardFeature {
    id: &'static str,
    label: &'static str,
    files_scanned: usize,
    items_found: usize,
    autocomplete_suggestions: usize,
    scan_time_ms: u128,
    rss_delta_kb: Option<i64>,
}

fn build_dashboard_report(project: &LaravelProject) -> Result<DashboardReport, String> {
    let mut total_files = BTreeSet::new();
    let features = vec![
        measure_feature("routes", "Routes", || {
            analyzers::routes::analyze(project)
                .map(|report| summarize_routes(&report, &mut total_files))
        })?,
        measure_feature("views", "Views", || {
            analyzers::views::analyze(project)
                .map(|report| summarize_views(&report, &mut total_files))
        })?,
        measure_feature("controllers", "Controllers", || {
            analyzers::controllers::analyze(project)
                .map(|report| summarize_controllers(&report, &mut total_files))
        })?,
        measure_feature("models", "Models", || {
            analyzers::models::analyze(project)
                .map(|report| summarize_models(&report, &mut total_files))
        })?,
        measure_feature("migrations", "Migrations", || {
            analyzers::migrations::analyze(project)
                .map(|report| summarize_migrations(&report, &mut total_files))
        })?,
        measure_feature("config", "Config", || {
            analyzers::configs::analyze(project)
                .map(|report| summarize_configs(&report, &mut total_files))
        })?,
        measure_feature("providers", "Providers", || {
            analyzers::providers::analyze(project)
                .map(|report| summarize_providers(&report, &mut total_files))
        })?,
        measure_feature("middleware", "Middleware", || {
            analyzers::middleware::analyze(project)
                .map(|report| summarize_middleware(&report, &mut total_files))
        })?,
        measure_feature("public", "Public Files", || {
            analyzers::public_assets::analyze(project)
                .map(|report| summarize_public_assets(&report, &mut total_files))
        })?,
        measure_feature("vendor", "Vendor Classes", || {
            Ok(summarize_vendor_classes(
                &crate::vendor::list_vendor_classes(&project.root),
                &mut total_files,
            ))
        })?,
    ];

    let summary = DashboardSummary {
        feature_count: features.len(),
        total_files_scanned: total_files.len(),
        total_items_found: features.iter().map(|feature| feature.items_found).sum(),
        total_autocomplete_suggestions: features
            .iter()
            .map(|feature| feature.autocomplete_suggestions)
            .sum(),
    };

    Ok(DashboardReport { summary, features })
}

fn measure_feature(
    id: &'static str,
    label: &'static str,
    build: impl FnOnce() -> Result<DashboardFeatureCounts, String>,
) -> Result<DashboardFeature, String> {
    let rss_before_kb = current_rss_kb();
    let started_at = Instant::now();
    let counts = build()?;
    let scan_time_ms = started_at.elapsed().as_millis();
    let rss_after_kb = current_rss_kb();

    Ok(DashboardFeature {
        id,
        label,
        files_scanned: counts.files_scanned,
        items_found: counts.items_found,
        autocomplete_suggestions: counts.autocomplete_suggestions,
        scan_time_ms,
        rss_delta_kb: rss_after_kb
            .zip(rss_before_kb)
            .map(|(after, before)| after as i64 - before as i64),
    })
}

struct DashboardFeatureCounts {
    files_scanned: usize,
    items_found: usize,
    autocomplete_suggestions: usize,
}

fn summarize_routes(
    report: &RouteReport,
    total_files: &mut BTreeSet<String>,
) -> DashboardFeatureCounts {
    let mut files = BTreeSet::new();
    let mut suggestions = 0usize;

    for route in &report.routes {
        add_path(&mut files, &route.file);
        add_path(&mut files, &route.registration.declared_in);
        if let Some(target) = &route.controller_target {
            if let Some(path) = target.declared_in.as_ref() {
                add_path(&mut files, path);
            }
            if let Some(path) = target.method_declared_in.as_ref() {
                add_path(&mut files, path);
            }
        }

        suggestions += 1;
        suggestions += usize::from(route.name.is_some());
        suggestions += usize::from(route.action.is_some());
        suggestions += route.parameter_patterns.len();
    }

    total_files.extend(files.iter().cloned());

    DashboardFeatureCounts {
        files_scanned: files.len(),
        items_found: report.route_count,
        autocomplete_suggestions: suggestions,
    }
}

fn summarize_views(
    report: &ViewReport,
    total_files: &mut BTreeSet<String>,
) -> DashboardFeatureCounts {
    let mut files = BTreeSet::new();
    let mut suggestions = 0usize;

    for view in &report.views {
        add_path(&mut files, &view.file);
        add_path(&mut files, &view.source.declared_in);
        for usage in &view.usages {
            add_path(&mut files, &usage.source.declared_in);
        }

        suggestions += 1 + view.props.len() + view.variables.len();
    }

    for component in &report.blade_components {
        add_path(&mut files, &component.source.declared_in);
        if let Some(path) = component.class_file.as_ref() {
            add_path(&mut files, path);
        }
        if let Some(path) = component.view_file.as_ref() {
            add_path(&mut files, path);
        }

        suggestions += 1 + component.props.len();
    }

    for component in &report.livewire_components {
        add_path(&mut files, &component.source.declared_in);
        if let Some(path) = component.class_file.as_ref() {
            add_path(&mut files, path);
        }
        if let Some(path) = component.view_file.as_ref() {
            add_path(&mut files, path);
        }

        suggestions += 1 + component.state.len();
    }

    for missing in &report.missing_views {
        for usage in &missing.usages {
            add_path(&mut files, &usage.source.declared_in);
        }
    }

    total_files.extend(files.iter().cloned());

    DashboardFeatureCounts {
        files_scanned: files.len(),
        items_found: report.view_count
            + report.blade_component_count
            + report.livewire_component_count,
        autocomplete_suggestions: suggestions,
    }
}

fn summarize_controllers(
    report: &ControllerReport,
    total_files: &mut BTreeSet<String>,
) -> DashboardFeatureCounts {
    let mut files = BTreeSet::new();
    let mut suggestions = 0usize;

    for controller in &report.controllers {
        add_path(&mut files, &controller.file);
        suggestions += 1;

        for method in &controller.methods {
            add_path(&mut files, &method.declared_in);
            suggestions += 1 + method.variables.len();
        }
    }

    total_files.extend(files.iter().cloned());

    DashboardFeatureCounts {
        files_scanned: files.len(),
        items_found: report.controller_count,
        autocomplete_suggestions: suggestions,
    }
}

fn summarize_models(
    report: &ModelReport,
    total_files: &mut BTreeSet<String>,
) -> DashboardFeatureCounts {
    let mut files = BTreeSet::new();
    let mut suggestions = 0usize;

    for model in &report.models {
        add_path(&mut files, &model.file);
        suggestions += 1
            + model.columns.len()
            + model.relations.len()
            + model.scopes.len()
            + model.accessors.len()
            + model.mutators.len();

        for relation in &model.relations {
            if let Some(path) = relation.related_model_file.as_ref() {
                add_path(&mut files, path);
            }
        }
    }

    total_files.extend(files.iter().cloned());

    DashboardFeatureCounts {
        files_scanned: files.len(),
        items_found: report.model_count,
        autocomplete_suggestions: suggestions,
    }
}

fn summarize_migrations(
    report: &MigrationReport,
    total_files: &mut BTreeSet<String>,
) -> DashboardFeatureCounts {
    let mut files = BTreeSet::new();
    let mut suggestions = 0usize;

    for migration in &report.migrations {
        add_path(&mut files, &migration.file);
        suggestions += usize::from(!migration.table.is_empty())
            + migration.columns.len()
            + migration.indexes.len();
    }

    total_files.extend(files.iter().cloned());

    DashboardFeatureCounts {
        files_scanned: files.len(),
        items_found: report.migration_count,
        autocomplete_suggestions: suggestions,
    }
}

fn summarize_configs(
    report: &ConfigReport,
    total_files: &mut BTreeSet<String>,
) -> DashboardFeatureCounts {
    let mut files = BTreeSet::new();
    let mut suggestions = 0usize;

    for item in &report.items {
        add_path(&mut files, &item.file);
        add_path(&mut files, &item.source.declared_in);
        suggestions += 1 + usize::from(item.env_key.is_some());
    }

    total_files.extend(files.iter().cloned());

    DashboardFeatureCounts {
        files_scanned: files.len(),
        items_found: report.item_count,
        autocomplete_suggestions: suggestions,
    }
}

fn summarize_providers(
    report: &ProviderReport,
    total_files: &mut BTreeSet<String>,
) -> DashboardFeatureCounts {
    let mut files = BTreeSet::new();

    for provider in &report.providers {
        add_path(&mut files, &provider.declared_in);
        if let Some(path) = provider.source_file.as_ref() {
            add_path(&mut files, path);
        }
    }

    total_files.extend(files.iter().cloned());

    DashboardFeatureCounts {
        files_scanned: files.len(),
        items_found: report.provider_count,
        autocomplete_suggestions: report.providers.len(),
    }
}

fn summarize_middleware(
    report: &MiddlewareReport,
    total_files: &mut BTreeSet<String>,
) -> DashboardFeatureCounts {
    let mut files = BTreeSet::new();

    for alias in &report.aliases {
        add_path(&mut files, &alias.source.declared_in);
    }
    for group in &report.groups {
        add_path(&mut files, &group.source.declared_in);
    }
    for pattern in &report.patterns {
        add_path(&mut files, &pattern.source.declared_in);
    }

    total_files.extend(files.iter().cloned());

    DashboardFeatureCounts {
        files_scanned: files.len(),
        items_found: report.alias_count + report.group_count + report.pattern_count,
        autocomplete_suggestions: report.alias_count + report.group_count + report.pattern_count,
    }
}

fn summarize_public_assets(
    report: &PublicAssetReport,
    total_files: &mut BTreeSet<String>,
) -> DashboardFeatureCounts {
    let mut files = BTreeSet::new();

    for asset in &report.assets {
        add_path(&mut files, &asset.file);
        for usage in &asset.usages {
            add_path(&mut files, &usage.file);
        }
    }

    total_files.extend(files.iter().cloned());

    DashboardFeatureCounts {
        files_scanned: files.len(),
        items_found: report.file_count,
        autocomplete_suggestions: report.file_count + report.usage_count,
    }
}

fn summarize_vendor_classes(
    classes: &[crate::vendor::VendorClass],
    total_files: &mut BTreeSet<String>,
) -> DashboardFeatureCounts {
    let mut files = BTreeSet::new();

    for class in classes {
        add_path(&mut files, Path::new(&class.file));
    }

    total_files.extend(files.iter().cloned());

    DashboardFeatureCounts {
        files_scanned: files.len(),
        items_found: classes.len(),
        autocomplete_suggestions: classes.len(),
    }
}

fn add_path(paths: &mut BTreeSet<String>, path: &Path) {
    let text = path.display().to_string();
    if !text.is_empty() {
        paths.insert(text);
    }
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

    let mut runtime_map: std::collections::BTreeMap<String, ComparedRoute> =
        std::collections::BTreeMap::new();
    let mut analyzer_map: std::collections::BTreeMap<String, ComparedRoute> =
        std::collections::BTreeMap::new();

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
