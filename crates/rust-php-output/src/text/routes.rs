use comfy_table::{Cell, Color, Row};
use std::fmt::Write as _;

use super::{header, join_or_dash, location_cell, new_table, terminal_width, wrap_cell};
use crate::types::{RouteEntry, RouteRegistration};

struct RouteWidths {
    uri: usize,
    name: usize,
    action: usize,
    middleware: usize,
    patterns: usize,
    registration: usize,
}
struct RouteSourceWidths {
    route: usize,
    uri: usize,
    provider: usize,
    declared_at: usize,
    kind: usize,
}

pub fn print_route_table(routes: &[RouteEntry]) {
    println!("{}", render_route_table(routes));
}

pub fn render_route_table(routes: &[RouteEntry]) -> String {
    if routes.is_empty() {
        return "No routes found.".to_string();
    }

    let mut output = String::new();
    let widths = route_widths();
    let mut current_file = None;
    let mut table = new_table();

    for route in routes {
        let file = route.file.as_path();
        if current_file != Some(file) {
            if current_file.is_some() {
                let _ = writeln!(output, "{table}\n");
                table = new_table();
            }
            let _ = writeln!(output, "{}", file.display());
            table.set_header(vec![
                header("Line:Col"),
                header("Method"),
                header("Uri"),
                header("Name"),
                header("Action"),
                header("Middleware"),
                header("Patterns"),
                header("Registered Via"),
            ]);
            current_file = Some(file);
        }
        table.add_row(Row::from(vec![
            location_cell(route.line, route.column),
            Cell::new(route.methods.join("|")),
            wrap_cell(&route.uri, widths.uri),
            wrap_cell(route.name.as_deref().unwrap_or("-"), widths.name),
            wrap_cell(&display_action(route), widths.action),
            wrap_cell(&display_middleware(route), widths.middleware),
            wrap_cell(&display_patterns(route), widths.patterns),
            wrap_cell(
                &registration_summary(&route.registration),
                widths.registration,
            ),
        ]));
    }
    let _ = write!(output, "{table}");
    output
}

pub fn print_route_source_table(routes: &[RouteEntry]) {
    println!("{}", render_route_source_table(routes));
}

pub fn render_route_source_table(routes: &[RouteEntry]) -> String {
    if routes.is_empty() {
        return "No routes found.".to_string();
    }

    let mut output = String::new();
    let widths = route_source_widths();
    let mut table = new_table();
    table.set_header(vec![
        header("Route"),
        header("Method"),
        header("Uri"),
        header("Provider"),
        header("Declared At"),
        header("Kind"),
    ]);

    for route in routes {
        table.add_row(Row::from(vec![
            wrap_cell(
                &format!("{}:{}:{}", route.file.display(), route.line, route.column),
                widths.route,
            ),
            Cell::new(route.methods.join("|")),
            wrap_cell(&route.uri, widths.uri),
            provider_cell(&route.registration, widths.provider),
            wrap_cell(
                &format!(
                    "{}:{}:{}",
                    route.registration.declared_in.display(),
                    route.registration.line,
                    route.registration.column
                ),
                widths.declared_at,
            ),
            kind_cell(&route.registration, widths.kind),
        ]));
    }
    let _ = write!(output, "{table}");
    output
}

fn display_middleware(route: &RouteEntry) -> String {
    let values = if route.resolved_middleware.is_empty() {
        &route.middleware
    } else {
        &route.resolved_middleware
    };
    join_or_dash(values)
}

fn display_action(route: &RouteEntry) -> String {
    let Some(action) = route.action.as_deref() else {
        return "-".to_string();
    };
    match route.controller_target.as_ref() {
        Some(target) if target.status != "ok" => format!("{action} [{}]", target.status),
        _ => action.to_string(),
    }
}

fn display_patterns(route: &RouteEntry) -> String {
    if route.parameter_patterns.is_empty() {
        return "-".to_string();
    }
    route
        .parameter_patterns
        .iter()
        .map(|(k, v)| format!("{k}={v}"))
        .collect::<Vec<_>>()
        .join(",")
}

fn registration_summary(reg: &RouteRegistration) -> String {
    match &reg.provider_class {
        Some(p) => format!(
            "{p} @ {}:{}:{}",
            reg.declared_in.display(),
            reg.line,
            reg.column
        ),
        None => reg.kind.clone(),
    }
}

fn provider_cell(reg: &RouteRegistration, width: usize) -> Cell {
    let text = reg.provider_class.as_deref().unwrap_or("-");
    let cell = wrap_cell(text, width);
    if reg.provider_class.is_some() {
        cell.fg(Color::Cyan)
    } else {
        cell.fg(Color::DarkGrey)
    }
}

fn kind_cell(reg: &RouteRegistration, width: usize) -> Cell {
    let cell = wrap_cell(&reg.kind, width);
    if reg.provider_class.is_some() {
        cell.fg(Color::Green)
    } else {
        cell.fg(Color::DarkGrey)
    }
}

fn route_widths() -> RouteWidths {
    let t = terminal_width();
    if t < 110 {
        RouteWidths {
            uri: 18,
            name: 16,
            action: 20,
            middleware: 14,
            patterns: 16,
            registration: 18,
        }
    } else if t < 150 {
        RouteWidths {
            uri: 24,
            name: 20,
            action: 28,
            middleware: 18,
            patterns: 18,
            registration: 24,
        }
    } else {
        RouteWidths {
            uri: 34,
            name: 26,
            action: 42,
            middleware: 24,
            patterns: 22,
            registration: 32,
        }
    }
}

fn route_source_widths() -> RouteSourceWidths {
    let t = terminal_width();
    if t < 110 {
        RouteSourceWidths {
            route: 18,
            uri: 18,
            provider: 18,
            declared_at: 18,
            kind: 14,
        }
    } else if t < 150 {
        RouteSourceWidths {
            route: 28,
            uri: 24,
            provider: 24,
            declared_at: 24,
            kind: 18,
        }
    } else {
        RouteSourceWidths {
            route: 38,
            uri: 30,
            provider: 30,
            declared_at: 34,
            kind: 20,
        }
    }
}
