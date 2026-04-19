use comfy_table::{Cell, Color, Row};

use crate::types::{ConfigItem, ConfigReport, ConfigSource};
use super::{header, location_cell, new_table, terminal_width, wrap_cell};

struct ConfigWidths { key: usize, env_key: usize, default: usize, env_value: usize, registration: usize }
struct ConfigSourceWidths { config: usize, env_key: usize, provider: usize, declared_at: usize, kind: usize }

pub fn print_config_table(report: &ConfigReport) {
    if report.items.is_empty() { println!("No config items found."); return; }

    let widths = config_widths();
    let mut current_file = None;
    let mut table = new_table();

    for item in &report.items {
        let file = item.file.as_path();
        if current_file != Some(file) {
            if current_file.is_some() { println!("{table}"); println!(); table = new_table(); }
            println!("{}", file.display());
            table.set_header(vec![
                header("Line:Col"), header("Key"), header("Env Key"),
                header("Default"), header("Env Value"), header("Registered Via"),
            ]);
            current_file = Some(file);
        }
        table.add_row(config_row(item, &widths));
    }

    println!("Legend: green = env value present, yellow = default-only, red = env key missing from .env");
    println!("{table}");
}

pub fn print_config_source_table(report: &ConfigReport) {
    if report.items.is_empty() { println!("No config items found."); return; }

    let widths = config_source_widths();
    let mut table = new_table();
    table.set_header(vec![
        header("Config"), header("Env Key"), header("Provider"),
        header("Declared At"), header("Kind"),
    ]);

    for item in &report.items {
        table.add_row(Row::from(vec![
            wrap_cell(&format!("{}:{}:{} ({})", item.file.display(), item.line, item.column, item.key), widths.config),
            env_key_cell(item, widths.env_key),
            provider_cell(&item.source, widths.provider),
            wrap_cell(&format!("{}:{}:{}", item.source.declared_in.display(), item.source.line, item.source.column), widths.declared_at),
            source_kind_cell(&item.source, widths.kind),
        ]));
    }
    println!("{table}");
}

fn config_row(item: &ConfigItem, widths: &ConfigWidths) -> Row {
    Row::from(vec![
        location_cell(item.line, item.column),
        wrap_cell(&item.key, widths.key),
        env_key_cell(item, widths.env_key),
        default_cell(item, widths.default),
        env_value_cell(item, widths.env_value),
        wrap_cell(&source_summary(&item.source), widths.registration),
    ])
}

fn env_key_cell(item: &ConfigItem, width: usize) -> Cell {
    let text = item.env_key.as_deref().unwrap_or("-");
    let cell = wrap_cell(text, width);
    if item.env_key.is_some() && item.env_value.is_none() { cell.fg(Color::Red) }
    else if item.env_key.is_some() { cell.fg(Color::Green) }
    else { cell.fg(Color::DarkGrey) }
}

fn default_cell(item: &ConfigItem, width: usize) -> Cell {
    let text = item.default_value.as_deref().unwrap_or("-");
    let cell = wrap_cell(text, width);
    if item.env_value.is_none() && item.default_value.is_some() { cell.fg(Color::Yellow) }
    else if item.default_value.is_none() { cell.fg(Color::DarkGrey) }
    else { cell }
}

fn env_value_cell(item: &ConfigItem, width: usize) -> Cell {
    let text = item.env_value.as_deref().unwrap_or("-");
    let cell = wrap_cell(text, width);
    if item.env_value.is_some() { cell.fg(Color::Green) }
    else if item.env_key.is_some() { cell.fg(Color::Red) }
    else { cell.fg(Color::DarkGrey) }
}

fn provider_cell(source: &ConfigSource, width: usize) -> Cell {
    let text = source.provider_class.as_deref().unwrap_or("-");
    let cell = wrap_cell(text, width);
    if source.provider_class.is_some() { cell.fg(Color::Cyan) } else { cell.fg(Color::DarkGrey) }
}

fn source_kind_cell(source: &ConfigSource, width: usize) -> Cell {
    let cell = wrap_cell(&source.kind, width);
    if source.provider_class.is_some() { cell.fg(Color::Green) } else { cell.fg(Color::DarkGrey) }
}

fn source_summary(source: &ConfigSource) -> String {
    match &source.provider_class {
        Some(p) => format!("{p} @ {}:{}:{}", source.declared_in.display(), source.line, source.column),
        None => source.kind.clone(),
    }
}

fn config_widths() -> ConfigWidths {
    let t = terminal_width();
    if t < 110 { ConfigWidths { key: 24, env_key: 16, default: 14, env_value: 16, registration: 18 } }
    else if t < 150 { ConfigWidths { key: 32, env_key: 20, default: 18, env_value: 20, registration: 24 } }
    else { ConfigWidths { key: 42, env_key: 26, default: 26, env_value: 26, registration: 32 } }
}

fn config_source_widths() -> ConfigSourceWidths {
    let t = terminal_width();
    if t < 110 { ConfigSourceWidths { config: 22, env_key: 16, provider: 18, declared_at: 18, kind: 14 } }
    else if t < 150 { ConfigSourceWidths { config: 34, env_key: 20, provider: 24, declared_at: 24, kind: 18 } }
    else { ConfigSourceWidths { config: 46, env_key: 26, provider: 30, declared_at: 34, kind: 22 } }
}
