use std::fs;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::thread;

use crate::project::{self, LaravelProject};

use super::command::DebugCommand;
use super::reports::{error_json, render_json_report};

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
            .map(|project| project.root.to_string_lossy().into_owned()),
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
    let bytes_read = stream
        .read(&mut buffer)
        .map_err(|error| error.to_string())?;
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
                    let location =
                        format!("/?project={}&command=route:list", url_encode(project_root));
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
            write_response(
                &mut stream,
                "200 OK",
                "application/json; charset=utf-8",
                &body,
            )?;
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
            let Some(command_id) = params.get("command") else {
                write_response(
                    &mut stream,
                    "400 Bad Request",
                    "application/json; charset=utf-8",
                    &error_json("missing query param: command"),
                )?;
                return Ok(());
            };

            match render_report(&state.projects, project_id, command_id) {
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

fn render_report(
    projects: &[LaravelProject],
    project_id: &str,
    command_id: &str,
) -> Result<String, String> {
    let project = projects
        .iter()
        .find(|project| project.root.to_string_lossy() == project_id)
        .ok_or_else(|| format!("unknown project id: {project_id}"))?;
    let command =
        DebugCommand::parse(command_id).ok_or_else(|| format!("unknown command: {command_id}"))?;

    render_json_report(project, command)
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
    serde_json::to_string(&items).map_err(|error| error.to_string())
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
    stream
        .write_all(response.as_bytes())
        .map_err(|error| error.to_string())
}

fn split_target(target: &str) -> (&str, &str) {
    target.split_once('?').unwrap_or((target, ""))
}

fn write_redirect(stream: &mut TcpStream, location: &str) -> Result<(), String> {
    let response = format!(
        "HTTP/1.1 302 Found\r\nLocation: {location}\r\nContent-Length: 0\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n"
    );
    stream
        .write_all(response.as_bytes())
        .map_err(|error| error.to_string())
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

    let body = fs::read_to_string(&asset_path).map_err(|error| error.to_string())?;
    write_response(stream, "200 OK", content_type_for(&asset_path), &body)
}

fn web_dist_dir() -> PathBuf {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .ancestors()
        .find(|dir| dir.join("web").is_dir())
        .unwrap_or(manifest_dir)
        .join("web")
        .join("out")
}

fn content_type_for(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
    {
        "html" => "text/html; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "js" => "application/javascript; charset=utf-8",
        "svg" => "image/svg+xml",
        _ => "text/plain; charset=utf-8",
    }
}

fn parse_query(query: &str) -> VecMap {
    let mut params = VecMap::default();
    for pair in query.split('&').filter(|segment| !segment.is_empty()) {
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
