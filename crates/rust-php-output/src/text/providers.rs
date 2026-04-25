use comfy_table::{Cell, Color};
use std::fmt::Write as _;

use super::{header, location_cell, new_table, terminal_width, wrap_cell};
use crate::types::{ProviderEntry, ProviderReport};

struct ProviderWidths {
    declared_in: usize,
    provider: usize,
    kind: usize,
    package: usize,
    source: usize,
    status: usize,
}

pub fn print_provider_table(report: &ProviderReport) {
    println!("{}", render_provider_table(report));
}

pub fn render_provider_table(report: &ProviderReport) -> String {
    if report.providers.is_empty() {
        return "No providers found.".to_string();
    }

    let mut output = String::new();
    let widths = provider_widths();
    let mut table = new_table();
    table.set_header(vec![
        header("Line:Col"),
        header("Declared In"),
        header("Provider"),
        header("Kind"),
        header("Package"),
        header("Source"),
        header("Status"),
    ]);

    for provider in &report.providers {
        table.add_row(provider_row(provider, &widths));
    }

    let _ = writeln!(output, "Project: {}", report.project_name);
    let _ = writeln!(output, "Declared providers: {}", report.provider_count);
    let _ = writeln!(output, "{table}");
    let _ = write!(
        output,
        "Legend: green = source resolved, red = source missing, grey = not package-backed"
    );
    output
}

fn provider_row(provider: &ProviderEntry, widths: &ProviderWidths) -> comfy_table::Row {
    comfy_table::Row::from(vec![
        location_cell(provider.line, provider.column),
        wrap_cell(
            &provider.declared_in.display().to_string(),
            widths.declared_in,
        ),
        wrap_cell(&provider.provider_class, widths.provider),
        wrap_cell(&provider.registration_kind, widths.kind),
        package_cell(provider, widths.package),
        source_cell(provider, widths.source),
        status_cell(provider, widths.status),
    ])
}

fn package_cell(provider: &ProviderEntry, width: usize) -> Cell {
    let text = provider.package_name.as_deref().unwrap_or("-");
    let cell = wrap_cell(text, width);
    if provider.package_name.is_some() {
        cell.fg(Color::Cyan)
    } else {
        cell.fg(Color::DarkGrey)
    }
}

fn source_cell(provider: &ProviderEntry, width: usize) -> Cell {
    let text = provider
        .source_file
        .as_ref()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|| "-".to_string());
    let cell = wrap_cell(&text, width);
    if provider.source_available {
        cell.fg(Color::Green)
    } else {
        cell.fg(Color::Red)
    }
}

fn status_cell(provider: &ProviderEntry, width: usize) -> Cell {
    let cell = wrap_cell(&provider.status, width);
    if provider.source_available {
        cell.fg(Color::Green)
    } else {
        cell.fg(Color::Red)
    }
}

fn provider_widths() -> ProviderWidths {
    let t = terminal_width();
    if t < 110 {
        ProviderWidths {
            declared_in: 18,
            provider: 20,
            kind: 12,
            package: 16,
            source: 18,
            status: 14,
        }
    } else if t < 150 {
        ProviderWidths {
            declared_in: 24,
            provider: 28,
            kind: 18,
            package: 22,
            source: 28,
            status: 14,
        }
    } else {
        ProviderWidths {
            declared_in: 32,
            provider: 38,
            kind: 22,
            package: 28,
            source: 40,
            status: 14,
        }
    }
}
