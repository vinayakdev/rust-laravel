use crate::types::{ConfigItem, ConfigReport, OutputMode, RouteEntry, RouteReport};
use comfy_table::{
    Cell, CellAlignment, Color, ColumnConstraint, ContentArrangement, Row, Table,
    modifiers::UTF8_ROUND_CORNERS, presets::UTF8_FULL_CONDENSED,
};

pub fn print_routes(report: &RouteReport, mode: OutputMode) -> Result<(), String> {
    match mode {
        OutputMode::Json => {
            let json = serde_json::to_string_pretty(report).map_err(|error| error.to_string())?;
            println!("{json}");
        }
        OutputMode::Text => print_route_table(&report.routes),
    }

    Ok(())
}

pub fn print_configs(report: &ConfigReport, mode: OutputMode) -> Result<(), String> {
    match mode {
        OutputMode::Json => {
            let json = serde_json::to_string_pretty(report).map_err(|error| error.to_string())?;
            println!("{json}");
        }
        OutputMode::Text => print_config_table(report),
    }

    Ok(())
}

fn print_route_table(routes: &[RouteEntry]) {
    if routes.is_empty() {
        println!("No routes found.");
        return;
    }

    let widths = route_widths();
    let mut current_file = None;
    let mut table = new_table();

    for route in routes {
        let file = route.file.as_path();
        if current_file != Some(file) {
            if current_file.is_some() {
                println!("{table}");
                println!();
                table = new_table();
            }

            println!("{}", file.display());
            table.set_header(vec![
                header("Line:Col"),
                header("Method"),
                header("Uri"),
                header("Name"),
                header("Action"),
                header("Middleware"),
            ]);
            current_file = Some(file);
        }

        table.add_row(Row::from(vec![
            location_cell(route.line, route.column),
            Cell::new(route.methods.join("|")),
            wrap_cell(&route.uri, widths.uri),
            wrap_cell(route.name.as_deref().unwrap_or("-"), widths.name),
            wrap_cell(route.action.as_deref().unwrap_or("-"), widths.action),
            wrap_cell(&join_or_dash(&route.middleware), widths.middleware),
        ]));
    }

    println!("{table}");
}

fn print_config_table(report: &ConfigReport) {
    if report.items.is_empty() {
        println!("No config items found.");
        return;
    }

    let widths = config_widths();
    let mut current_file = None;
    let mut table = new_table();

    for item in &report.items {
        let file = item.file.as_path();
        if current_file != Some(file) {
            if current_file.is_some() {
                println!("{table}");
                println!();
                table = new_table();
            }

            println!("{}", file.display());
            table.set_header(vec![
                header("Line:Col"),
                header("Key"),
                header("Env Key"),
                header("Default"),
                header("Env Value"),
            ]);
            current_file = Some(file);
        }

        table.add_row(config_row(item, &widths));
    }

    println!(
        "Legend: green = env value present, yellow = default-only, red = env key missing from .env"
    );
    println!("{table}");
}

fn config_row(item: &ConfigItem, widths: &ConfigWidths) -> Row {
    Row::from(vec![
        location_cell(item.line, item.column),
        wrap_cell(&item.key, widths.key),
        env_key_cell(item, widths.env_key),
        default_cell(item, widths.default),
        env_value_cell(item, widths.env_value),
    ])
}

fn new_table() -> Table {
    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL_CONDENSED)
        .apply_modifier(UTF8_ROUND_CORNERS)
        .set_content_arrangement(ContentArrangement::Dynamic);
    table.set_constraints(vec![
        ColumnConstraint::UpperBoundary(comfy_table::Width::Fixed(10)),
        ColumnConstraint::LowerBoundary(comfy_table::Width::Fixed(8)),
        ColumnConstraint::LowerBoundary(comfy_table::Width::Fixed(18)),
        ColumnConstraint::LowerBoundary(comfy_table::Width::Fixed(14)),
        ColumnConstraint::LowerBoundary(comfy_table::Width::Fixed(16)),
    ]);
    table
}

fn header(text: &str) -> Cell {
    Cell::new(text).set_alignment(CellAlignment::Center)
}

fn location_cell(line: usize, column: usize) -> Cell {
    Cell::new(format!("{line}:{column}")).set_alignment(CellAlignment::Right)
}

fn wrap_cell(text: &str, width: usize) -> Cell {
    Cell::new(truncate_for_terminal(text, width))
}

fn join_or_dash(values: &[String]) -> String {
    if values.is_empty() {
        "-".to_string()
    } else {
        values.join(",")
    }
}

fn env_key_cell(item: &ConfigItem, width: usize) -> Cell {
    let text = item.env_key.as_deref().unwrap_or("-");
    let mut cell = wrap_cell(text, width);
    if item.env_key.is_some() && item.env_value.is_none() {
        cell = cell.fg(Color::Red);
    } else if item.env_key.is_some() && item.env_value.is_some() {
        cell = cell.fg(Color::Green);
    } else {
        cell = cell.fg(Color::DarkGrey);
    }
    cell
}

fn default_cell(item: &ConfigItem, width: usize) -> Cell {
    let text = item.default_value.as_deref().unwrap_or("-");
    let mut cell = wrap_cell(text, width);
    if item.env_value.is_none() && item.default_value.is_some() {
        cell = cell.fg(Color::Yellow);
    } else if item.default_value.is_none() {
        cell = cell.fg(Color::DarkGrey);
    }
    cell
}

fn env_value_cell(item: &ConfigItem, width: usize) -> Cell {
    let text = item.env_value.as_deref().unwrap_or("-");
    let mut cell = wrap_cell(text, width);
    if item.env_value.is_some() {
        cell = cell.fg(Color::Green);
    } else if item.env_key.is_some() {
        cell = cell.fg(Color::Red);
    } else {
        cell = cell.fg(Color::DarkGrey);
    }
    cell
}

fn truncate_for_terminal(text: &str, width: usize) -> String {
    let count = text.chars().count();
    if count <= width {
        return text.to_string();
    }
    if width <= 1 {
        return "…".to_string();
    }

    let mut output = String::new();
    for ch in text.chars().take(width.saturating_sub(1)) {
        output.push(ch);
    }
    output.push('…');
    output
}

struct RouteWidths {
    uri: usize,
    name: usize,
    action: usize,
    middleware: usize,
}

struct ConfigWidths {
    key: usize,
    env_key: usize,
    default: usize,
    env_value: usize,
}

fn route_widths() -> RouteWidths {
    let terminal = terminal_width();
    if terminal < 110 {
        RouteWidths {
            uri: 18,
            name: 16,
            action: 20,
            middleware: 14,
        }
    } else if terminal < 150 {
        RouteWidths {
            uri: 24,
            name: 20,
            action: 28,
            middleware: 18,
        }
    } else {
        RouteWidths {
            uri: 34,
            name: 26,
            action: 42,
            middleware: 24,
        }
    }
}

fn config_widths() -> ConfigWidths {
    let terminal = terminal_width();
    if terminal < 110 {
        ConfigWidths {
            key: 24,
            env_key: 16,
            default: 14,
            env_value: 16,
        }
    } else if terminal < 150 {
        ConfigWidths {
            key: 32,
            env_key: 20,
            default: 18,
            env_value: 20,
        }
    } else {
        ConfigWidths {
            key: 42,
            env_key: 26,
            default: 26,
            env_value: 26,
        }
    }
}

fn terminal_width() -> usize {
    std::env::var("COLUMNS")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .filter(|value| *value > 40)
        .unwrap_or(160)
}
