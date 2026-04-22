use super::{header, new_table, terminal_width, wrap_cell};
use crate::types::{MiddlewareAlias, MiddlewareGroup, MiddlewareReport, RoutePattern};
use std::fmt::Write as _;

struct MiddlewareWidths {
    name: usize,
    detail: usize,
    declared_at: usize,
    provider: usize,
}

pub fn print_middleware_tables(report: &MiddlewareReport) {
    println!("{}", render_middleware_tables(report));
}

pub fn render_middleware_tables(report: &MiddlewareReport) -> String {
    let mut output = String::new();
    let _ = writeln!(output, "Project: {}", report.project_name);

    if report.aliases.is_empty() && report.groups.is_empty() && report.patterns.is_empty() {
        let _ = write!(output, "No middleware or route patterns found.");
        return output;
    }

    let widths = middleware_widths();

    if !report.aliases.is_empty() {
        let mut table = new_table();
        table.set_header(vec![
            header("Alias"),
            header("Target"),
            header("Declared At"),
            header("Provider"),
        ]);
        for alias in &report.aliases {
            table.add_row(alias_row(alias, &widths));
        }
        let _ = writeln!(output, "Aliases");
        let _ = writeln!(output, "{table}\n");
    }

    if !report.groups.is_empty() {
        let mut table = new_table();
        table.set_header(vec![
            header("Group"),
            header("Members"),
            header("Declared At"),
            header("Provider"),
        ]);
        for group in &report.groups {
            table.add_row(group_row(group, &widths));
        }
        let _ = writeln!(output, "Groups");
        let _ = writeln!(output, "{table}\n");
    }

    if !report.patterns.is_empty() {
        let mut table = new_table();
        table.set_header(vec![
            header("Param"),
            header("Pattern"),
            header("Declared At"),
            header("Provider"),
        ]);
        for pattern in &report.patterns {
            table.add_row(pattern_row(pattern, &widths));
        }
        let _ = writeln!(output, "Patterns");
        let _ = write!(output, "{table}");
    }
    output
}

fn alias_row(alias: &MiddlewareAlias, w: &MiddlewareWidths) -> comfy_table::Row {
    comfy_table::Row::from(vec![
        wrap_cell(&alias.name, w.name),
        wrap_cell(&alias.target, w.detail),
        wrap_cell(
            &format!(
                "{}:{}:{}",
                alias.source.declared_in.display(),
                alias.source.line,
                alias.source.column
            ),
            w.declared_at,
        ),
        wrap_cell(&alias.source.provider_class, w.provider),
    ])
}

fn group_row(group: &MiddlewareGroup, w: &MiddlewareWidths) -> comfy_table::Row {
    comfy_table::Row::from(vec![
        wrap_cell(&group.name, w.name),
        wrap_cell(&group.members.join(","), w.detail),
        wrap_cell(
            &format!(
                "{}:{}:{}",
                group.source.declared_in.display(),
                group.source.line,
                group.source.column
            ),
            w.declared_at,
        ),
        wrap_cell(&group.source.provider_class, w.provider),
    ])
}

fn pattern_row(pattern: &RoutePattern, w: &MiddlewareWidths) -> comfy_table::Row {
    comfy_table::Row::from(vec![
        wrap_cell(&pattern.parameter, w.name),
        wrap_cell(&pattern.pattern, w.detail),
        wrap_cell(
            &format!(
                "{}:{}:{}",
                pattern.source.declared_in.display(),
                pattern.source.line,
                pattern.source.column
            ),
            w.declared_at,
        ),
        wrap_cell(&pattern.source.provider_class, w.provider),
    ])
}

fn middleware_widths() -> MiddlewareWidths {
    let t = terminal_width();
    if t < 110 {
        MiddlewareWidths {
            name: 14,
            detail: 22,
            declared_at: 18,
            provider: 18,
        }
    } else if t < 150 {
        MiddlewareWidths {
            name: 18,
            detail: 34,
            declared_at: 24,
            provider: 24,
        }
    } else {
        MiddlewareWidths {
            name: 22,
            detail: 44,
            declared_at: 32,
            provider: 30,
        }
    }
}
