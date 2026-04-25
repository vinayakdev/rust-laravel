use comfy_table::{Cell, Color, Row};
use std::fmt::Write as _;

use super::{header, location_cell, new_table, terminal_width, wrap_cell};
use crate::types::{ControllerMethodEntry, ControllerReport};

struct Widths {
    method: usize,
    visibility: usize,
    source: usize,
    status: usize,
}

pub fn print_controller_report(report: &ControllerReport) {
    println!("{}", render_controller_report(report));
}

pub fn render_controller_report(report: &ControllerReport) -> String {
    if report.controllers.is_empty() {
        return "No controllers found.".to_string();
    }

    let widths = widths();
    let mut output = String::new();

    for controller in &report.controllers {
        let _ = writeln!(
            output,
            "{} [{} callable / {} total]",
            controller.fqn, controller.callable_method_count, controller.method_count
        );
        let _ = writeln!(
            output,
            "  file: {}:{}",
            controller.file.display(),
            controller.line
        );
        if let Some(parent) = &controller.extends {
            let _ = writeln!(output, "  extends: {parent}");
        }
        if !controller.traits.is_empty() {
            let _ = writeln!(output, "  traits: {}", controller.traits.join(", "));
        }

        let mut table = new_table();
        table.set_header(vec![
            header("Line"),
            header("Method"),
            header("Visibility"),
            header("Source"),
            header("Route"),
            header("Notes"),
        ]);

        for method in &controller.methods {
            table.add_row(method_row(method, &widths));
        }

        let _ = writeln!(output, "{table}\n");
    }

    output.trim_end().to_string()
}

fn method_row(method: &ControllerMethodEntry, widths: &Widths) -> Row {
    Row::from(vec![
        location_cell(method.line, 1),
        wrap_cell(&method.name, widths.method),
        visibility_cell(method, widths.visibility),
        wrap_cell(
            &format!("{} ({})", method.source_name, method.source_kind),
            widths.source,
        ),
        route_cell(method),
        wrap_cell(&method.accessibility, widths.status),
    ])
}

fn visibility_cell(method: &ControllerMethodEntry, width: usize) -> Cell {
    let text = if method.is_static {
        format!("{} static", method.visibility)
    } else {
        method.visibility.clone()
    };
    wrap_cell(&text, width)
}

fn route_cell(method: &ControllerMethodEntry) -> Cell {
    let label = if method.accessible_from_route {
        "callable"
    } else {
        "blocked"
    };
    let cell = Cell::new(label);
    if method.accessible_from_route {
        cell.fg(Color::Green)
    } else {
        cell.fg(Color::Red)
    }
}

fn widths() -> Widths {
    let width = terminal_width();
    if width < 110 {
        Widths {
            method: 16,
            visibility: 12,
            source: 22,
            status: 24,
        }
    } else if width < 150 {
        Widths {
            method: 22,
            visibility: 14,
            source: 28,
            status: 34,
        }
    } else {
        Widths {
            method: 28,
            visibility: 16,
            source: 38,
            status: 44,
        }
    }
}
