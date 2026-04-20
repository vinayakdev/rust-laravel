use std::fmt::Write as _;
use std::fs;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Command as ProcessCommand;
use std::sync::Arc;
use std::thread;
use std::time::Instant;

use crate::analyzers;
use crate::project::{self, LaravelProject};
use crate::types::RouteEntry;
use serde::Serialize;

const HOST: &str = "127.0.0.1";
const PORT_START: u16 = 4318;
const PORT_END: u16 = 4328;

pub fn run(initial_project: Option<&str>) -> Result<(), String> {
    let projects = project::discover_projects()?;
    if projects.is_empty() {
        return Err(
            "no Laravel projects found. put one under ./laravel-example/<project>, ./test/<project>, or run from a Laravel app"
                .to_string(),
        );
    }

    let state = Arc::new(State {
        projects,
        initial_project_root: initial_project
            .map(|value| project::resolve(Some(value)))
            .transpose()?
            .map(|p| p.root.to_string_lossy().into_owned()),
    });

    let (listener, address) = bind_listener()?;

    println!("Debug web UI running at http://{address}");
    println!("Press Ctrl+C to stop.");

    for stream in listener.incoming() {
        let Ok(stream) = stream else {
            continue;
        };
        let state = Arc::clone(&state);
        let _ = thread::Builder::new()
            .name("debug-web-request".to_string())
            .stack_size(16 * 1024 * 1024)
            .spawn(move || {
                let _ = handle_connection(stream, state);
            });
    }

    Ok(())
}

fn bind_listener() -> Result<(TcpListener, String), String> {
    let mut last_error = None;

    for port in PORT_START..=PORT_END {
        let address = format!("{HOST}:{port}");
        match TcpListener::bind(&address) {
            Ok(listener) => return Ok((listener, address)),
            Err(error) => last_error = Some(format!("{address}: {error}")),
        }
    }

    Err(format!(
        "failed to bind http server on any localhost port from {PORT_START} to {PORT_END}: {}",
        last_error.unwrap_or_else(|| "unknown bind error".to_string())
    ))
}

struct State {
    projects: Vec<LaravelProject>,
    initial_project_root: Option<String>,
}

fn handle_connection(mut stream: TcpStream, state: Arc<State>) -> Result<(), String> {
    let mut buffer = [0u8; 16 * 1024];
    let bytes_read = stream.read(&mut buffer).map_err(|e| e.to_string())?;
    if bytes_read == 0 {
        return Ok(());
    }

    let request = String::from_utf8_lossy(&buffer[..bytes_read]);
    let Some(request_line) = request.lines().next() else {
        return Ok(());
    };

    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or_default();
    let target = parts.next().unwrap_or("/");

    if method != "GET" {
        write_response(
            &mut stream,
            "405 Method Not Allowed",
            "text/plain; charset=utf-8",
            "Only GET is supported.",
        )?;
        return Ok(());
    }

    let (path, query) = split_target(target);

    match path {
        "/" => {
            if query.is_empty() {
                if let Some(project_root) = state.initial_project_root.as_deref() {
                    let location = format!("/?project={}&command=route:list", url_encode(project_root));
                    write_redirect(&mut stream, &location)?;
                } else {
                    serve_web_asset(&mut stream, "index.html")?;
                }
            } else {
                serve_web_asset(&mut stream, "index.html")?;
            }
        }
        asset_path if asset_path.starts_with("/assets/") || asset_path.starts_with("/_next/") => {
            serve_web_asset(&mut stream, asset_path.trim_start_matches('/'))?
        }
        "/api/projects" => {
            let body = projects_payload(&state.projects)?;
            write_response(&mut stream, "200 OK", "application/json; charset=utf-8", &body)?;
        }
        "/api/report" => {
            let params = parse_query(query);
            let Some(project_id) = params.get("project") else {
                write_response(
                    &mut stream,
                    "400 Bad Request",
                    "application/json; charset=utf-8",
                    &error_json("missing query param: project"),
                )?;
                return Ok(());
            };
            let Some(command) = params.get("command") else {
                write_response(
                    &mut stream,
                    "400 Bad Request",
                    "application/json; charset=utf-8",
                    &error_json("missing query param: command"),
                )?;
                return Ok(());
            };

            match render_report(&state.projects, project_id, command) {
                Ok(body) => write_response(
                    &mut stream,
                    "200 OK",
                    "application/json; charset=utf-8",
                    &body,
                )?,
                Err(error) => write_response(
                    &mut stream,
                    "400 Bad Request",
                    "application/json; charset=utf-8",
                    &error_json(&error),
                )?,
            }
        }
        _ => write_response(
            &mut stream,
            "404 Not Found",
            "text/plain; charset=utf-8",
            "Not found.",
        )?,
    }

    Ok(())
}

fn projects_payload(projects: &[LaravelProject]) -> Result<String, String> {
    let items = projects
        .iter()
        .map(|project| {
            serde_json::json!({
                "id": project.root.to_string_lossy(),
                "name": project.name,
                "root": project.root,
            })
        })
        .collect::<Vec<_>>();
    serde_json::to_string(&items).map_err(|e| e.to_string())
}

fn render_report(
    projects: &[LaravelProject],
    project_id: &str,
    command: &str,
) -> Result<String, String> {
    let project = projects
        .iter()
        .find(|project| project.root.to_string_lossy() == project_id)
        .ok_or_else(|| format!("unknown project id: {project_id}"))?;

    let started_at = Instant::now();
    let rss_before_kb = current_rss_kb();

    let payload = match command {
        "route:list" => {
            let report = analyzers::routes::analyze(project)?;
            serde_json::json!({
                "project": project.name,
                "root": project.root,
                "command": command,
                "report": report,
            })
        }
        "route:sources" => {
            let report = analyzers::routes::analyze(project)?;
            serde_json::json!({
                "project": project.name,
                "root": project.root,
                "command": command,
                "report": report,
            })
        }
        "middleware:list" => {
            let report = analyzers::middleware::analyze(project)?;
            serde_json::json!({
                "project": project.name,
                "root": project.root,
                "command": command,
                "report": report,
            })
        }
        "config:list" => {
            let report = analyzers::configs::analyze(project)?;
            serde_json::json!({
                "project": project.name,
                "root": project.root,
                "command": command,
                "report": report,
            })
        }
        "config:sources" => {
            let report = analyzers::configs::analyze(project)?;
            serde_json::json!({
                "project": project.name,
                "root": project.root,
                "command": command,
                "report": report,
            })
        }
        "provider:list" => {
            let report = analyzers::providers::analyze(project)?;
            serde_json::json!({
                "project": project.name,
                "root": project.root,
                "command": command,
                "report": report,
            })
        }
        "view:list" => {
            let report = analyzers::views::analyze(project)?;
            serde_json::json!({
                "project": project.name,
                "root": project.root,
                "command": command,
                "report": report,
            })
        }
        "route:compare" => {
            let rust_report = analyzers::routes::analyze(project)?;
            let comparison = compare_routes(project, &rust_report.routes)?;
            serde_json::json!({
                "project": project.name,
                "root": project.root,
                "command": command,
                "comparison": comparison,
            })
        }
        other => return Err(format!("unknown command: {other}")),
    };

    let debug = DebugInfo {
        duration_ms: started_at.elapsed().as_millis(),
        parsed_file_count: collect_debug_paths(&payload).len(),
        rss_before_kb,
        rss_after_kb: current_rss_kb(),
    };

    let payload = attach_debug(payload, debug);
    serde_json::to_string(&payload).map_err(|e| e.to_string())
}

#[derive(Debug, Serialize)]
struct DebugInfo {
    duration_ms: u128,
    parsed_file_count: usize,
    rss_before_kb: Option<u64>,
    rss_after_kb: Option<u64>,
}

fn attach_debug(mut payload: serde_json::Value, debug: DebugInfo) -> serde_json::Value {
    if let serde_json::Value::Object(ref mut object) = payload {
        object.insert(
            "debug".to_string(),
            serde_json::to_value(debug).unwrap_or(serde_json::Value::Null),
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

fn collect_debug_paths(value: &serde_json::Value) -> BTreeSet<String> {
    let mut paths = BTreeSet::new();
    collect_debug_paths_inner(None, value, &mut paths);
    paths
}

fn collect_debug_paths_inner(
    key: Option<&str>,
    value: &serde_json::Value,
    paths: &mut BTreeSet<String>,
) {
    match value {
        serde_json::Value::Object(object) => {
            for (child_key, child_value) in object {
                collect_debug_paths_inner(Some(child_key.as_str()), child_value, paths);
            }
        }
        serde_json::Value::Array(items) => {
            for item in items {
                collect_debug_paths_inner(key, item, paths);
            }
        }
        serde_json::Value::String(text) => {
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
                | "artisan_path"
        )
    )
}

fn error_json(message: &str) -> String {
    serde_json::to_string(&serde_json::json!({ "error": message }))
        .unwrap_or_else(|_| "{\"error\":\"internal serialization failure\"}".to_string())
}

fn write_response(
    stream: &mut TcpStream,
    status: &str,
    content_type: &str,
    body: &str,
) -> Result<(), String> {
    let response = format!(
        "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n{}",
        body.len(),
        body
    );
    stream.write_all(response.as_bytes()).map_err(|e| e.to_string())
}

fn split_target(target: &str) -> (&str, &str) {
    target.split_once('?').unwrap_or((target, ""))
}

fn write_redirect(stream: &mut TcpStream, location: &str) -> Result<(), String> {
    let response = format!(
        "HTTP/1.1 302 Found\r\nLocation: {location}\r\nContent-Length: 0\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n"
    );
    stream.write_all(response.as_bytes()).map_err(|e| e.to_string())
}

fn serve_web_asset(stream: &mut TcpStream, relative_path: &str) -> Result<(), String> {
    let asset_path = web_dist_dir().join(relative_path);
    if !asset_path.is_file() {
        let (status, message) = if relative_path == "index.html" {
            (
                "500 Internal Server Error",
                "web UI assets are missing. run `pnpm build` inside ./web first.",
            )
        } else {
            ("404 Not Found", "Not found.")
        };
        write_response(stream, status, "text/plain; charset=utf-8", message)?;
        return Ok(());
    }

    let body = fs::read_to_string(&asset_path).map_err(|e| e.to_string())?;
    write_response(stream, "200 OK", content_type_for(&asset_path), &body)
}

fn web_dist_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("web").join("out")
}

fn content_type_for(path: &Path) -> &'static str {
    match path.extension().and_then(|value| value.to_str()).unwrap_or_default() {
        "html" => "text/html; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "js" => "application/javascript; charset=utf-8",
        "svg" => "image/svg+xml",
        _ => "text/plain; charset=utf-8",
    }
}

fn parse_query(query: &str) -> VecMap {
    let mut params = VecMap::default();
    for pair in query.split('&').filter(|s| !s.is_empty()) {
        let (key, value) = pair.split_once('=').unwrap_or((pair, ""));
        params.insert(url_decode(key), url_decode(value));
    }
    params
}

#[derive(Default)]
struct VecMap(Vec<(String, String)>);

impl VecMap {
    fn insert(&mut self, key: String, value: String) {
        self.0.push((key, value));
    }

    fn get(&self, key: &str) -> Option<&str> {
        self.0
            .iter()
            .find(|(candidate, _)| candidate == key)
            .map(|(_, value)| value.as_str())
    }
}

fn url_decode(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut output = Vec::with_capacity(bytes.len());
    let mut index = 0;

    while index < bytes.len() {
        match bytes[index] {
            b'+' => {
                output.push(b' ');
                index += 1;
            }
            b'%' if index + 2 < bytes.len() => {
                let hex = &input[index + 1..index + 3];
                if let Ok(value) = u8::from_str_radix(hex, 16) {
                    output.push(value);
                    index += 3;
                } else {
                    output.push(bytes[index]);
                    index += 1;
                }
            }
            byte => {
                output.push(byte);
                index += 1;
            }
        }
    }

    String::from_utf8_lossy(&output).into_owned()
}

fn url_encode(input: &str) -> String {
    input
        .bytes()
        .flat_map(|byte| match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' | b'/' => {
                vec![byte as char]
            }
            b' ' => vec!['+'],
            other => format!("%{:02X}", other).chars().collect(),
        })
        .collect()
}

#[allow(dead_code)]
fn index_html(initial_project_root: Option<&str>) -> String {
    let mut html = String::with_capacity(INDEX_HTML_HEAD.len() + INDEX_HTML_TAIL.len() + 128);
    let _ = write!(
        html,
        "{}<script>window.__INITIAL_PROJECT__ = {};</script>{}",
        INDEX_HTML_HEAD,
        serde_json::to_string(&initial_project_root.unwrap_or("")).unwrap_or_else(|_| "\"\"".to_string()),
        INDEX_HTML_TAIL
    );
    html
}

#[allow(dead_code)]
const INDEX_HTML_HEAD: &str = r#"<!doctype html>
<html lang="en">
  <head>
    <meta charset="utf-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1" />
    <title>Rust PHP Debug</title>
    <link rel="stylesheet" href="/app.css" />
  </head>
  <body>
    <div id="app" class="shell">
      <aside class="sidebar">
        <div class="brand">
          <p class="eyebrow">Laravel Static Debugger</p>
          <h1>rust-php</h1>
          <p class="lede">Use the sidebar to switch projects and analyzers. The selected project and command stay in the URL, so refresh keeps your place.</p>
        </div>
        <div class="sidebar-section">
          <div class="section-head">
            <span>Project</span>
            <button id="refresh-projects" class="ghost-button" type="button">Refresh</button>
          </div>
          <label class="control-label" for="project-select">Laravel project</label>
          <select id="project-select" class="project-select"></select>
        </div>
        <div class="sidebar-section sidebar-grow">
          <div class="section-head">
            <span>Parameters</span>
          </div>
          <nav id="command-list" class="command-list"></nav>
        </div>
      </aside>
      <main class="workspace">
        <header class="toolbar">
          <div class="toolbar-copy">
            <p class="viewer-label">Selected Project</p>
            <h2 id="project-name">None</h2>
            <p id="project-root" class="project-root"></p>
          </div>
          <div class="toolbar-meta">
            <div id="debug-bar" class="debug-bar"></div>
            <span id="status-pill" class="status-pill">Idle</span>
          </div>
        </header>
        <section class="viewer-card">
          <div class="viewer-head">
            <div>
              <p class="viewer-label">Active Analyzer</p>
              <h2 id="command-name">Routes</h2>
            </div>
            <p id="route-state" class="project-root"></p>
          </div>
          <div id="report-output" class="viewer-body">
            <div class="empty-state">Loading projects…</div>
          </div>
        </section>
      </main>
    </div>
"#;

#[allow(dead_code)]
const INDEX_HTML_TAIL: &str = r#"
    <script src="/app.js"></script>
  </body>
</html>
"#;

#[allow(dead_code)]
const APP_CSS: &str = r#":root {
  --bg: #f4f0e7;
  --bg-soft: #ebe3d4;
  --panel: rgba(252, 249, 242, 0.94);
  --panel-strong: #fffdf8;
  --line: rgba(67, 50, 32, 0.14);
  --line-strong: rgba(67, 50, 32, 0.2);
  --text: #2b2218;
  --muted: #6a5d49;
  --accent: #1b6f6a;
  --accent-soft: rgba(27, 111, 106, 0.1);
  --accent-strong: #114f4b;
  --shadow: 0 22px 64px rgba(56, 41, 19, 0.12);
  --radius: 26px;
  --radius-sm: 18px;
}

* { box-sizing: border-box; }

html, body {
  margin: 0;
  min-height: 100%;
  color: var(--text);
  background:
    radial-gradient(circle at top left, rgba(17, 94, 89, 0.12), transparent 24rem),
    radial-gradient(circle at bottom right, rgba(182, 127, 51, 0.18), transparent 28rem),
    linear-gradient(180deg, var(--bg), #efe6d8);
  font-family: "Iowan Old Style", "Palatino Linotype", "Book Antiqua", Georgia, serif;
}

body {
  min-height: 100vh;
}

.shell {
  display: grid;
  grid-template-columns: 288px minmax(0, 1fr);
  min-height: 100vh;
  gap: 20px;
  padding: 20px;
}

.sidebar,
.viewer-card,
.toolbar {
  background: var(--panel);
  backdrop-filter: blur(16px);
  border: 1px solid var(--line);
  box-shadow: var(--shadow);
}

.sidebar {
  border-radius: calc(var(--radius) + 4px);
  padding: 22px;
  display: flex;
  flex-direction: column;
  gap: 22px;
}

.brand h1,
.viewer-head h2,
.toolbar-copy h2 {
  margin: 0;
  font-size: clamp(1.9rem, 2.4vw, 2.35rem);
  line-height: 0.95;
  letter-spacing: -0.04em;
}

.eyebrow,
.viewer-label {
  margin: 0 0 8px;
  text-transform: uppercase;
  letter-spacing: 0.18em;
  font-size: 0.72rem;
  color: var(--muted);
}

.lede,
.project-root {
  margin: 12px 0 0;
  color: var(--muted);
  line-height: 1.55;
}

.sidebar-section {
  min-height: 0;
  display: flex;
  flex-direction: column;
  gap: 14px;
}

.sidebar-grow {
  flex: 1;
}

.section-head,
.viewer-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 16px;
}

.workspace {
  min-width: 0;
  display: grid;
  grid-template-rows: auto minmax(0, 1fr);
  gap: 18px;
}

.toolbar {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 18px;
  border-radius: 999px;
  padding: 18px 20px;
}

.toolbar-copy {
  min-width: 0;
}

.toolbar-copy .project-root {
  margin-top: 10px;
}

.toolbar-meta {
  display: flex;
  align-items: center;
  gap: 12px;
  flex-wrap: wrap;
  justify-content: flex-end;
}

.ghost-button,
.command-button,
.project-select {
  appearance: none;
  border: 1px solid var(--line);
  background: rgba(255, 255, 255, 0.64);
  color: inherit;
  transition: 160ms ease;
}

.ghost-button {
  border-radius: 999px;
  padding: 8px 12px;
  color: var(--muted);
  background: transparent;
  cursor: pointer;
}

.ghost-button:hover,
.command-button:hover,
.project-select:hover,
.project-select:focus {
  border-color: rgba(14, 118, 110, 0.35);
  background: rgba(255, 255, 255, 0.66);
}

.control-label {
  font-size: 0.78rem;
  text-transform: uppercase;
  letter-spacing: 0.14em;
  color: var(--muted);
}

.project-select {
  width: 100%;
  border-radius: 16px;
  padding: 12px 14px;
  font: inherit;
  cursor: pointer;
}

.command-list {
  display: flex;
  flex-direction: column;
  gap: 10px;
  overflow: auto;
  padding-right: 2px;
}

.command-button {
  width: 100%;
  text-align: left;
  border-radius: 18px;
  padding: 14px 16px;
  cursor: pointer;
}

.command-button strong,
.command-button span {
  display: block;
}

.command-button strong {
  font-weight: 700;
  font-size: 0.98rem;
  color: var(--text);
}

.command-button span {
  margin-top: 5px;
  color: var(--muted);
  font-size: 0.82rem;
  line-height: 1.4;
}

.command-button.is-active {
  background: var(--accent);
  color: white;
  border-color: var(--accent);
}

.command-button.is-active strong,
.command-button.is-active span {
  color: white;
}

.status-pill {
  border-radius: 999px;
  padding: 10px 14px;
  background: var(--accent-soft);
  color: var(--accent-strong);
  font-weight: 700;
  font-size: 0.9rem;
}

.viewer-card {
  border-radius: calc(var(--radius) + 6px);
  padding: 26px;
  display: grid;
  grid-template-rows: auto minmax(0, 1fr);
  gap: 18px;
  min-height: 0;
}

.viewer-body {
  min-height: 0;
  overflow: auto;
  padding: 22px;
  border-radius: var(--radius-sm);
  background:
    linear-gradient(180deg, rgba(252, 249, 243, 0.96), rgba(245, 238, 226, 0.98)),
    repeating-linear-gradient(
      180deg,
      rgba(47, 36, 23, 0.02) 0,
      rgba(47, 36, 23, 0.02) 28px,
      transparent 28px,
      transparent 56px
    );
  border: 1px solid rgba(47, 36, 23, 0.08);
  color: #241a10;
  box-shadow: inset 0 1px 0 rgba(255, 255, 255, 0.7);
}

.empty-state {
  display: grid;
  place-items: center;
  min-height: 260px;
  color: var(--muted);
  font-size: 1rem;
}

.report-stack {
  display: flex;
  flex-direction: column;
  gap: 18px;
}

.summary-row {
  display: flex;
  flex-wrap: wrap;
  gap: 12px;
}

.summary-chip {
  border-radius: 999px;
  padding: 10px 14px;
  border: 1px solid rgba(70, 56, 34, 0.12);
  background: rgba(255, 255, 255, 0.72);
  color: var(--text);
  font-weight: 700;
  font-size: 0.9rem;
}

.debug-bar {
  display: flex;
  align-items: center;
  justify-content: flex-end;
  flex-wrap: wrap;
  gap: 8px;
  min-height: 42px;
}

.debug-chip {
  border-radius: 999px;
  padding: 8px 12px;
  border: 1px solid rgba(27, 111, 106, 0.16);
  background: rgba(27, 111, 106, 0.08);
  color: var(--accent-strong);
  font-size: 0.78rem;
  font-weight: 700;
  white-space: nowrap;
}

.section-card {
  border-radius: 20px;
  border: 1px solid rgba(47, 36, 23, 0.08);
  background: rgba(255, 253, 248, 0.76);
  overflow: hidden;
}

.section-headline {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  padding: 14px 18px;
  border-bottom: 1px solid rgba(47, 36, 23, 0.08);
}

.section-headline h3 {
  margin: 0;
  font-size: 1.05rem;
}

.count-badge {
  border-radius: 999px;
  padding: 7px 10px;
  background: var(--bg-accent);
  color: var(--text);
  font-size: 0.8rem;
  font-weight: 700;
}

.table-scroll {
  overflow: auto;
}

.data-table {
  width: 100%;
  border-collapse: collapse;
  min-width: 720px;
}

.data-table thead th {
  position: sticky;
  top: 0;
  z-index: 1;
  background: #f3ebde;
  color: var(--muted);
  text-transform: uppercase;
  letter-spacing: 0.08em;
  font-size: 0.72rem;
  text-align: left;
}

.data-table th,
.data-table td {
  padding: 12px 14px;
  border-bottom: 1px solid rgba(47, 36, 23, 0.08);
  vertical-align: top;
}

.data-table tbody tr:hover {
  background: rgba(17, 94, 89, 0.05);
}

.mono {
  font-family: "SFMono-Regular", "Cascadia Code", "Source Code Pro", Menlo, Consolas, monospace;
  font-size: 0.88rem;
}

.soft {
  color: var(--muted);
}

.badge-line {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
}

.mini-badge {
  border-radius: 999px;
  padding: 5px 9px;
  background: rgba(47, 36, 23, 0.07);
  color: var(--text);
  font-size: 0.74rem;
  font-weight: 700;
}

.mini-badge.success {
  background: rgba(14, 118, 110, 0.12);
  color: var(--accent-strong);
}

.mini-badge.warn {
  background: rgba(180, 114, 37, 0.14);
  color: #8c4b0f;
}

.mini-badge.danger {
  background: rgba(161, 46, 28, 0.14);
  color: #962b16;
}

.legend {
  padding: 0 18px 18px;
  color: var(--muted);
  line-height: 1.55;
}

@media (max-width: 960px) {
  .shell {
    grid-template-columns: 1fr;
  }

  .sidebar {
    order: 2;
  }

  .workspace {
    order: 1;
  }

  .toolbar,
  .viewer-head {
    flex-direction: column;
    align-items: flex-start;
  }

  .toolbar-meta,
  .debug-bar {
    justify-content: flex-start;
  }
}
"#;

#[derive(serde::Serialize)]
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

#[derive(Clone, serde::Serialize)]
struct ComparedRoute {
    key: String,
    methods: Vec<String>,
    uri: String,
    name: Option<String>,
    action: Option<String>,
    source: Option<String>,
    middleware: Vec<String>,
}

fn compare_routes(project: &LaravelProject, analyzer_routes: &[RouteEntry]) -> Result<RouteComparisonPayload, String> {
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
            note: "This project does not have an artisan file, so runtime comparison is unavailable.".to_string(),
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
        .map_err(|e| format!("failed to run php artisan route:list --json: {e}"))?;

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
        .map_err(|e| format!("failed to parse artisan route:list --json output: {e}"))?;

    use std::collections::{BTreeMap, BTreeSet};
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

    let matched_keys = runtime_keys.intersection(&analyzer_keys).cloned().collect::<Vec<_>>();
    let runtime_only_keys = runtime_keys.difference(&analyzer_keys).cloned().collect::<Vec<_>>();
    let analyzer_only_keys = analyzer_keys.difference(&runtime_keys).cloned().collect::<Vec<_>>();

    Ok(RouteComparisonPayload {
        runtime_count: runtime_map.len(),
        analyzer_count: analyzer_map.len(),
        matched_count: matched_keys.len(),
        runtime_only_count: runtime_only_keys.len(),
        analyzer_only_count: analyzer_only_keys.len(),
        runnable: true,
        artisan_path: Some(artisan_path.display().to_string()),
        note: "Runtime route list comes from `php artisan route:list --json` and is compared against normalized analyzer routes by method + URI + name.".to_string(),
        matched: matched_keys.into_iter().filter_map(|key| runtime_map.get(&key).cloned()).collect(),
        runtime_only: runtime_only_keys.into_iter().filter_map(|key| runtime_map.get(&key).cloned()).collect(),
        analyzer_only: analyzer_only_keys.into_iter().filter_map(|key| analyzer_map.get(&key).cloned()).collect(),
    })
}

#[derive(serde::Deserialize)]
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
        source: Some(format!("{}:{}:{}", route.file.display(), route.line, route.column)),
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
        let trimmed = uri.trim_matches('/');
        format!("/{}", trimmed)
    }
}

fn route_compare_key(methods: &[String], uri: &str, name: Option<&str>) -> String {
    format!("{} {} {}", methods.join("|"), uri, name.unwrap_or("-"))
}

#[allow(dead_code)]
const APP_JS: &str = r#"const COMMANDS = [
  { id: "route:list", label: "Routes" },
  { id: "route:compare", label: "Route Compare" },
  { id: "route:sources", label: "Route Sources" },
  { id: "middleware:list", label: "Middleware" },
  { id: "config:list", label: "Config" },
  { id: "config:sources", label: "Config Sources" },
  { id: "provider:list", label: "Providers" },
  { id: "view:list", label: "Views & Components" },
];

const state = {
  projects: [],
  selectedProject: "",
  selectedCommand: "route:list",
};

const elements = {
  projectList: document.getElementById("project-list"),
  commandTabs: document.getElementById("command-tabs"),
  projectName: document.getElementById("project-name"),
  projectRoot: document.getElementById("project-root"),
  reportOutput: document.getElementById("report-output"),
  statusPill: document.getElementById("status-pill"),
  refreshProjects: document.getElementById("refresh-projects"),
};

function setStatus(message) {
  elements.statusPill.textContent = message;
}

function escapeHtml(value) {
  return value
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;");
}

function renderProjects() {
  elements.projectList.innerHTML = state.projects
    .map((project) => {
      const active = project.id === state.selectedProject ? " is-active" : "";
      return `
        <button class="project-button${active}" data-project-id="${encodeURIComponent(project.id)}" type="button">
          <strong>${escapeHtml(project.name)}</strong>
          <span>${escapeHtml(project.root)}</span>
        </button>
      `;
    })
    .join("");

  elements.projectList.querySelectorAll("[data-project-id]").forEach((button) => {
    button.addEventListener("click", () => {
      state.selectedProject = decodeURIComponent(button.dataset.projectId);
      renderProjects();
      updateProjectMeta();
      loadReport();
    });
  });
}

function renderCommands() {
  elements.commandTabs.innerHTML = COMMANDS.map((command) => {
    const active = command.id === state.selectedCommand ? " is-active" : "";
    return `
      <button class="command-tab${active}" data-command-id="${command.id}" type="button">
        ${command.label}
      </button>
    `;
  }).join("");

  elements.commandTabs.querySelectorAll("[data-command-id]").forEach((button) => {
    button.addEventListener("click", () => {
      state.selectedCommand = button.dataset.commandId;
      renderCommands();
      loadReport();
    });
  });
}

function updateProjectMeta() {
  const project = state.projects.find((candidate) => candidate.id === state.selectedProject);
  elements.projectName.textContent = project ? project.name : "None";
  elements.projectRoot.textContent = project ? project.root : "";
}

function setViewerHtml(html) {
  elements.reportOutput.innerHTML = html;
}

function emptyState(message) {
  return `<div class="empty-state">${escapeHtml(message)}</div>`;
}

function badge(text, tone = "") {
  const klass = tone ? `mini-badge ${tone}` : "mini-badge";
  return `<span class="${klass}">${escapeHtml(String(text))}</span>`;
}

function summaryChip(label, value) {
  return `<span class="summary-chip">${escapeHtml(label)}: ${escapeHtml(String(value))}</span>`;
}

function formatKilobytes(value) {
  const numeric = Number(value);
  if (!Number.isFinite(numeric)) {
    return null;
  }
  const absolute = Math.abs(numeric);
  if (absolute >= 1024 * 1024) {
    return `${numeric < 0 ? "-" : ""}${(absolute / (1024 * 1024)).toFixed(2)} GB`;
  }
  if (absolute >= 1024) {
    return `${numeric < 0 ? "-" : ""}${(absolute / 1024).toFixed(1)} MB`;
  }
  return `${numeric} KB`;
}

function renderDebugSummaryChips(payload) {
  const debug = payload.debug;
  if (!debug) {
    return "";
  }

  const chips = [
    summaryChip("Files Parsed", debug.parsed_file_count ?? 0),
    summaryChip("Time", `${debug.duration_ms ?? 0} ms`),
  ];

  if (debug.rss_after_kb != null) {
    chips.push(summaryChip("RSS", formatKilobytes(debug.rss_after_kb) || `${debug.rss_after_kb} KB`));
  }

  if (debug.rss_before_kb != null && debug.rss_after_kb != null) {
    const delta = debug.rss_after_kb - debug.rss_before_kb;
    const prefix = delta > 0 ? "+" : "";
    chips.push(summaryChip("RSS Delta", `${prefix}${formatKilobytes(delta) || `${delta} KB`}`));
  }

  return chips.join("");
}

function dataCell(value, klass = "") {
  const classAttr = klass ? ` class="${klass}"` : "";
  return `<td${classAttr}>${value}</td>`;
}

function sectionTable(title, count, headers, rows, options = {}) {
  const minWidth = options.minWidth || 720;
  const legend = options.legend ? `<div class="legend">${options.legend}</div>` : "";
  return `
    <section class="section-card">
      <div class="section-headline">
        <h3>${escapeHtml(title)}</h3>
        <span class="count-badge">${escapeHtml(String(count))}</span>
      </div>
      <div class="table-scroll">
        <table class="data-table" style="min-width:${minWidth}px">
          <thead>
            <tr>${headers.map((header) => `<th>${escapeHtml(header)}</th>`).join("")}</tr>
          </thead>
          <tbody>${rows.join("")}</tbody>
        </table>
      </div>
      ${legend}
    </section>
  `;
}

function renderRoutes(payload, sourceMode) {
  const report = payload.report;
  const rows = report.routes.map((route) => {
    const middleware = route.resolved_middleware.length ? route.resolved_middleware : route.middleware;
    const patterns = Object.entries(route.parameter_patterns || {});
    const registeredVia = `
      <div>${escapeHtml(route.registration.kind)}</div>
      <div class="soft mono">${escapeHtml(route.registration.declared_in)}:${route.registration.line}:${route.registration.column}</div>
      ${route.registration.provider_class ? `<div class="soft">${escapeHtml(route.registration.provider_class)}</div>` : ""}
    `;

    return `
      <tr>
        ${dataCell(`<div><strong>${escapeHtml(route.methods.join("|"))}</strong></div><div class="soft mono">${escapeHtml(route.uri)}</div>`)}
        ${dataCell(`<div class="mono">${escapeHtml(route.file)}:${route.line}:${route.column}</div>`)}
        ${dataCell(escapeHtml(route.name || "-"))}
        ${dataCell(escapeHtml(route.action || "-"), "mono")}
        ${dataCell(middleware.length ? `<div class="badge-line">${middleware.map((item) => badge(item)).join("")}</div>` : `<span class="soft">-</span>`)}
        ${dataCell(patterns.length ? `<div class="badge-line">${patterns.map(([key, value]) => badge(`${key}=${value}`)).join("")}</div>` : `<span class="soft">-</span>`)}
        ${dataCell(registeredVia)}
      </tr>
    `;
  });

  const chips = `
    <div class="summary-row">
      ${summaryChip("Project", payload.project)}
      ${summaryChip("Routes", report.route_count)}
      ${summaryChip("Mode", sourceMode ? "Source Attribution" : "Route Table")}
      ${renderDebugSummaryChips(payload)}
    </div>
  `;

  return `
    <div class="report-stack">
      ${chips}
      ${sectionTable(
        sourceMode ? "Registered Route Sources" : "Effective Routes",
        report.route_count,
        ["Route", "Location", "Name", "Action", "Middleware", "Patterns", "Registered Via"],
        rows,
        { minWidth: 1180 }
      )}
    </div>
  `;
}

function renderCompareRows(routes) {
  return routes.map((route) => `
    <tr>
      ${dataCell(`<div><strong>${escapeHtml(route.methods.join("|"))}</strong></div><div class="soft mono">${escapeHtml(route.uri)}</div>`)}
      ${dataCell(escapeHtml(route.name || "-"))}
      ${dataCell(escapeHtml(route.action || "-"), "mono")}
      ${dataCell(route.middleware.length ? `<div class="badge-line">${route.middleware.map((item) => badge(item)).join("")}</div>` : `<span class="soft">-</span>`)}
      ${dataCell(route.source ? `<span class="mono">${escapeHtml(route.source)}</span>` : `<span class="soft">-</span>`)}
    </tr>
  `);
}

function renderRouteCompare(payload) {
  const comparison = payload.comparison;

  const chips = `
    <div class="summary-row">
      ${summaryChip("Project", payload.project)}
      ${summaryChip("Runtime Routes", comparison.runtime_count)}
      ${summaryChip("Analyzer Routes", comparison.analyzer_count)}
      ${summaryChip("Matched", comparison.matched_count)}
      ${summaryChip("Missed by Rust", comparison.runtime_only_count)}
      ${summaryChip("Analyzer Only", comparison.analyzer_only_count)}
      ${renderDebugSummaryChips(payload)}
    </div>
  `;

  const note = `
    <section class="section-card">
      <div class="section-headline">
        <h3>Comparison Notes</h3>
        <span class="count-badge">${comparison.runnable ? "runtime available" : "runtime unavailable"}</span>
      </div>
      <div class="legend">
        <p>${escapeHtml(comparison.note)}</p>
        ${comparison.artisan_path ? `<p class="mono">${escapeHtml(comparison.artisan_path)}</p>` : ""}
      </div>
    </section>
  `;

  const matchedTable = sectionTable(
    "Matched Routes",
    comparison.matched_count,
    ["Route", "Name", "Action", "Middleware", "Source"],
    renderCompareRows(comparison.matched),
    { minWidth: 1080 }
  );

  const runtimeOnlyTable = sectionTable(
    "Runtime Only: Missing From Rust",
    comparison.runtime_only_count,
    ["Route", "Name", "Action", "Middleware", "Source"],
    renderCompareRows(comparison.runtime_only),
    { minWidth: 1080 }
  );

  const analyzerOnlyTable = sectionTable(
    "Analyzer Only",
    comparison.analyzer_only_count,
    ["Route", "Name", "Action", "Middleware", "Source"],
    renderCompareRows(comparison.analyzer_only),
    { minWidth: 1080 }
  );

  return `
    <div class="report-stack">
      ${chips}
      ${note}
      ${runtimeOnlyTable}
      ${matchedTable}
      ${analyzerOnlyTable}
    </div>
  `;
}

function renderConfigs(payload, sourceMode) {
  const report = payload.report;
  const rows = report.items.map((item) => `
    <tr>
      ${dataCell(`<div><strong>${escapeHtml(item.key)}</strong></div><div class="soft mono">${escapeHtml(item.file)}:${item.line}:${item.column}</div>`)}
      ${dataCell(escapeHtml(item.env_key || "-"), "mono")}
      ${dataCell(escapeHtml(item.default_value || "-"), "mono")}
      ${dataCell(escapeHtml(item.env_value || "-"), "mono")}
      ${dataCell(`
        <div>${escapeHtml(item.source.kind)}</div>
        ${item.source.provider_class ? `<div class="soft">${escapeHtml(item.source.provider_class)}</div>` : ""}
        <div class="soft mono">${escapeHtml(item.source.declared_in)}:${item.source.line}:${item.source.column}</div>
      `)}
    </tr>
  `);

  const chips = `
    <div class="summary-row">
      ${summaryChip("Project", payload.project)}
      ${summaryChip("Items", report.item_count)}
      ${summaryChip("Mode", sourceMode ? "Source Attribution" : "Config Table")}
      ${renderDebugSummaryChips(payload)}
    </div>
  `;

  return `
    <div class="report-stack">
      ${chips}
      ${sectionTable(
        sourceMode ? "Config Sources" : "Effective Config",
        report.item_count,
        ["Config Item", "Env Key", "Default", "Env Value", "Registered Via"],
        rows,
        {
          minWidth: 1080,
          legend: !sourceMode ? "The web UI shows raw values directly; the terminal color legend is no longer needed here." : ""
        }
      )}
    </div>
  `;
}

function renderProviders(payload) {
  const report = payload.report;
  const rows = report.providers.map((provider) => `
    <tr>
      ${dataCell(`<div><strong>${escapeHtml(provider.provider_class)}</strong></div><div class="soft mono">${escapeHtml(provider.declared_in)}:${provider.line}:${provider.column}</div>`)}
      ${dataCell(escapeHtml(provider.registration_kind))}
      ${dataCell(escapeHtml(provider.package_name || "-"))}
      ${dataCell(provider.source_file ? `<div class="mono">${escapeHtml(provider.source_file)}</div>` : `<span class="soft">-</span>`)}
      ${dataCell(`
        <div class="badge-line">
          ${badge(provider.status, provider.source_available ? "success" : "danger")}
          ${badge(provider.source_available ? "source available" : "source missing", provider.source_available ? "success" : "danger")}
        </div>
      `)}
    </tr>
  `);

  return `
    <div class="report-stack">
      <div class="summary-row">
        ${summaryChip("Project", payload.project)}
        ${summaryChip("Providers", report.provider_count)}
        ${renderDebugSummaryChips(payload)}
      </div>
      ${sectionTable(
        "Providers",
        report.provider_count,
        ["Provider", "Registration Kind", "Package", "Source File", "Status"],
        rows,
        { minWidth: 1100 }
      )}
    </div>
  `;
}

function renderMiddleware(payload) {
  const report = payload.report;

  const aliasRows = report.aliases.map((alias) => `
    <tr>
      ${dataCell(`<strong>${escapeHtml(alias.name)}</strong>`)}
      ${dataCell(escapeHtml(alias.target), "mono")}
      ${dataCell(`<div>${escapeHtml(alias.source.provider_class)}</div><div class="soft mono">${escapeHtml(alias.source.declared_in)}:${alias.source.line}:${alias.source.column}</div>`)}
    </tr>
  `);

  const groupRows = report.groups.map((group) => `
    <tr>
      ${dataCell(`<strong>${escapeHtml(group.name)}</strong>`)}
      ${dataCell(group.members.length ? `<div class="badge-line">${group.members.map((item) => badge(item)).join("")}</div>` : `<span class="soft">-</span>`)}
      ${dataCell(`<div>${escapeHtml(group.source.provider_class)}</div><div class="soft mono">${escapeHtml(group.source.declared_in)}:${group.source.line}:${group.source.column}</div>`)}
    </tr>
  `);

  const patternRows = report.patterns.map((pattern) => `
    <tr>
      ${dataCell(`<strong>${escapeHtml(pattern.parameter)}</strong>`)}
      ${dataCell(escapeHtml(pattern.pattern), "mono")}
      ${dataCell(`<div>${escapeHtml(pattern.source.provider_class)}</div><div class="soft mono">${escapeHtml(pattern.source.declared_in)}:${pattern.source.line}:${pattern.source.column}</div>`)}
    </tr>
  `);

  return `
    <div class="report-stack">
      <div class="summary-row">
        ${summaryChip("Project", payload.project)}
        ${summaryChip("Aliases", report.alias_count)}
        ${summaryChip("Groups", report.group_count)}
        ${summaryChip("Patterns", report.pattern_count)}
        ${renderDebugSummaryChips(payload)}
      </div>
      ${sectionTable("Middleware Aliases", report.alias_count, ["Alias", "Target", "Declared In"], aliasRows, { minWidth: 900 })}
      ${sectionTable("Middleware Groups", report.group_count, ["Group", "Members", "Declared In"], groupRows, { minWidth: 980 })}
      ${sectionTable("Route Patterns", report.pattern_count, ["Parameter", "Pattern", "Declared In"], patternRows, { minWidth: 900 })}
    </div>
  `;
}

function renderViewInventory(payload) {
  const report = payload.report;
  const renderVariableBadges = (items) =>
    items && items.length
      ? `<div class="badge-line">${items
          .map((item) => badge(item.default_value == null ? item.name : `${item.name}=${item.default_value}`))
          .join("")}</div>`
      : `<span class="soft">-</span>`;

  const viewRows = report.views.map((view) => `
    <tr>
      ${dataCell(`<div><strong>${escapeHtml(view.name)}</strong></div><div class="soft mono">${escapeHtml(view.file)}</div>`)}
      ${dataCell(escapeHtml(view.kind))}
      ${dataCell(renderVariableBadges(view.props))}
      ${dataCell(renderVariableBadges(view.variables))}
      ${dataCell(`
        <div class="mono">${escapeHtml(view.source.declared_in)}:${view.source.line}:${view.source.column}</div>
        ${view.source.provider_class ? `<div class="soft">${escapeHtml(view.source.provider_class)}</div>` : ""}
      `)}
    </tr>
  `);

  const bladeRows = report.blade_components.map((component) => `
    <tr>
      ${dataCell(`<div><strong>${escapeHtml(component.component)}</strong></div><div class="soft">${escapeHtml(component.kind)}</div>`)}
      ${dataCell(component.class_name ? `<div class="mono">${escapeHtml(component.class_name)}</div>${component.class_file ? `<div class="soft mono">${escapeHtml(component.class_file)}</div>` : ""}` : `<span class="soft">-</span>`)}
      ${dataCell(component.view_name ? `<div class="mono">${escapeHtml(component.view_name)}</div>${component.view_file ? `<div class="soft mono">${escapeHtml(component.view_file)}</div>` : ""}` : component.view_file ? `<div class="mono">${escapeHtml(component.view_file)}</div>` : `<span class="soft">-</span>`)}
      ${dataCell(renderVariableBadges(component.props))}
      ${dataCell(`<div class="mono">${escapeHtml(component.source.declared_in)}:${component.source.line}:${component.source.column}</div>${component.source.provider_class ? `<div class="soft">${escapeHtml(component.source.provider_class)}</div>` : ""}`)}
    </tr>
  `);

  const livewireRows = report.livewire_components.map((component) => `
    <tr>
      ${dataCell(`<div><strong>${escapeHtml(component.component)}</strong></div><div class="soft">${escapeHtml(component.kind)}</div>`)}
      ${dataCell(component.class_name ? `<div class="mono">${escapeHtml(component.class_name)}</div>${component.class_file ? `<div class="soft mono">${escapeHtml(component.class_file)}</div>` : ""}` : `<span class="soft">-</span>`)}
      ${dataCell(component.view_name ? `<div class="mono">${escapeHtml(component.view_name)}</div>${component.view_file ? `<div class="soft mono">${escapeHtml(component.view_file)}</div>` : ""}` : component.view_file ? `<div class="mono">${escapeHtml(component.view_file)}</div>` : `<span class="soft">-</span>`)}
      ${dataCell(renderVariableBadges(component.state))}
      ${dataCell(`<div class="mono">${escapeHtml(component.source.declared_in)}:${component.source.line}:${component.source.column}</div>${component.source.provider_class ? `<div class="soft">${escapeHtml(component.source.provider_class)}</div>` : ""}`)}
    </tr>
  `);

  return `
    <div class="report-stack">
      <div class="summary-row">
        ${summaryChip("Project", payload.project)}
        ${summaryChip("Views", report.view_count)}
        ${summaryChip("Blade Components", report.blade_component_count)}
        ${summaryChip("Livewire Components", report.livewire_component_count)}
        ${renderDebugSummaryChips(payload)}
      </div>
      ${sectionTable("Views", report.view_count, ["View", "Kind", "Blade Props", "Passed Variables", "Declared In"], viewRows, { minWidth: 1220 })}
      ${sectionTable("Blade Components", report.blade_component_count, ["Component", "Class", "View", "Props", "Declared In"], bladeRows, { minWidth: 1320 })}
      ${sectionTable("Livewire Components", report.livewire_component_count, ["Component", "Class", "View", "Public State", "Declared In"], livewireRows, { minWidth: 1320 })}
    </div>
  `;
}

function renderPayload(payload) {
  switch (payload.command) {
    case "route:list":
      return renderRoutes(payload, false);
    case "route:compare":
      return renderRouteCompare(payload);
    case "route:sources":
      return renderRoutes(payload, true);
    case "config:list":
      return renderConfigs(payload, false);
    case "config:sources":
      return renderConfigs(payload, true);
    case "provider:list":
      return renderProviders(payload);
    case "middleware:list":
      return renderMiddleware(payload);
    case "view:list":
      return renderViewInventory(payload);
    default:
      return emptyState(`No renderer for ${payload.command}`);
  }
}

async function loadProjects() {
  setStatus("Loading projects");
  setViewerHtml(emptyState("Loading projects…"));

  const response = await fetch("/api/projects");
  const projects = await response.json();
  state.projects = projects;

  const initial = window.__INITIAL_PROJECT__ || "";
  if (!state.selectedProject) {
    state.selectedProject =
      projects.find((project) => project.id === initial)?.id ||
      projects[0]?.id ||
      "";
  }

  renderProjects();
  renderCommands();
  updateProjectMeta();

  if (state.selectedProject) {
    await loadReport();
  } else {
    setViewerHtml(emptyState("No projects found."));
    setStatus("No projects");
  }
}

async function loadReport() {
  if (!state.selectedProject) {
    setViewerHtml(emptyState("No project selected."));
    return;
  }

  const params = new URLSearchParams({
    project: state.selectedProject,
    command: state.selectedCommand,
  });

  setStatus("Running analyzer");
  setViewerHtml(emptyState("Loading report…"));

  const response = await fetch(`/api/report?${params.toString()}`);
  const payload = await response.json();

  if (!response.ok) {
    setViewerHtml(emptyState(payload.error || "Unknown server error."));
    setStatus("Analyzer failed");
    return;
  }

  setViewerHtml(renderPayload(payload));
  setStatus("Loaded");
}

elements.refreshProjects.addEventListener("click", () => {
  state.projects = [];
  state.selectedProject = "";
  loadProjects().catch((error) => {
    elements.reportOutput.textContent = error.message;
    setStatus("Refresh failed");
  });
});

loadProjects().catch((error) => {
  setViewerHtml(emptyState(error.message));
  setStatus("Boot failed");
});
"#;
