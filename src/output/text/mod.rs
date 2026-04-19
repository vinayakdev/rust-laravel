pub mod configs;
pub mod middleware;
pub mod providers;
pub mod routes;

use comfy_table::{
    Cell, CellAlignment, ColumnConstraint, ContentArrangement, Table,
    modifiers::UTF8_ROUND_CORNERS, presets::UTF8_FULL_CONDENSED,
};

pub(super) fn new_table() -> Table {
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

pub(super) fn header(text: &str) -> Cell {
    Cell::new(text).set_alignment(CellAlignment::Center)
}

pub(super) fn location_cell(line: usize, column: usize) -> Cell {
    Cell::new(format!("{line}:{column}")).set_alignment(CellAlignment::Right)
}

pub(super) fn wrap_cell(text: &str, width: usize) -> Cell {
    Cell::new(truncate(text, width))
}

pub(super) fn join_or_dash(values: &[String]) -> String {
    if values.is_empty() { "-".to_string() } else { values.join(",") }
}

pub(super) fn terminal_width() -> usize {
    std::env::var("COLUMNS")
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .filter(|&v| v > 40)
        .unwrap_or(160)
}

fn truncate(text: &str, width: usize) -> String {
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
