use std::fmt::Write as _;

use super::{header, join_or_dash, new_table, terminal_width, wrap_cell};
use crate::types::{MigrationReport, ModelReport};

pub fn print_model_report(report: &ModelReport) {
    println!("{}", render_model_report(report));
}

pub fn print_migration_report(report: &MigrationReport) {
    println!("{}", render_migration_report(report));
}

pub fn render_model_report(report: &ModelReport) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "Project: {}", report.project_name);
    let _ = writeln!(out, "Models: {}", report.model_count);
    let _ = writeln!(out);
    let _ = write!(out, "{}", render_models_table(report));
    out
}

pub fn render_migration_report(report: &MigrationReport) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "Project: {}", report.project_name);
    let _ = writeln!(out, "Migrations: {}", report.migration_count);
    let _ = writeln!(out);
    let _ = write!(out, "{}", render_migrations_table(report));
    out
}

fn render_models_table(report: &ModelReport) -> String {
    if report.models.is_empty() {
        return "No models found.".to_string();
    }

    let t = terminal_width();
    let (cn, tb, tr, tc, tf) = if t < 120 {
        (20, 18, 20, 8, 20)
    } else if t < 160 {
        (26, 22, 28, 10, 28)
    } else {
        (32, 26, 36, 12, 34)
    };

    let mut table = new_table();
    table.set_header(vec![
        header("Class"),
        header("Table"),
        header("Traits"),
        header("Cols"),
        header("Relations"),
        header("File"),
    ]);

    for m in &report.models {
        let table_display = if m.table_inferred {
            format!("{}*", m.table)
        } else {
            m.table.clone()
        };

        let traits_str = join_or_dash(&m.traits);

        let relations_str = if m.relations.is_empty() {
            "-".to_string()
        } else {
            m.relations
                .iter()
                .map(|r| format!("{}:{}", abbrev_relation(&r.relation_type), r.method))
                .collect::<Vec<_>>()
                .join(", ")
        };

        let cols_count = m.columns.len().to_string();

        table.add_row(vec![
            wrap_cell(&m.class_name, cn),
            wrap_cell(&table_display, tb),
            wrap_cell(&traits_str, tr),
            wrap_cell(&cols_count, tc),
            wrap_cell(&relations_str, tf),
            wrap_cell(&m.file.display().to_string(), tf),
        ]);
    }

    table.to_string()
}

fn render_migrations_table(report: &MigrationReport) -> String {
    if report.migrations.is_empty() {
        return "No migrations found.".to_string();
    }

    let t = terminal_width();
    let (ts, tb, op, nc, nd) = if t < 120 {
        (18, 18, 8, 5, 5)
    } else if t < 160 {
        (20, 22, 8, 6, 6)
    } else {
        (24, 28, 8, 6, 6)
    };

    let mut table = new_table();
    table.set_header(vec![
        header("Timestamp"),
        header("Table"),
        header("Op"),
        header("+Cols"),
        header("-Cols"),
        header("File"),
    ]);

    for m in &report.migrations {
        let table_display = if m.table.is_empty() {
            "-".to_string()
        } else {
            m.table.clone()
        };
        table.add_row(vec![
            wrap_cell(&m.timestamp, ts),
            wrap_cell(&table_display, tb),
            wrap_cell(&m.operation, op),
            wrap_cell(&m.columns.len().to_string(), nc),
            wrap_cell(&m.dropped_columns.len().to_string(), nd),
            wrap_cell(&m.file.display().to_string(), 40),
        ]);
    }

    table.to_string()
}

fn abbrev_relation(rel: &str) -> &str {
    match rel {
        "hasOne" => "1:1",
        "hasMany" => "1:N",
        "belongsTo" => "N:1",
        "belongsToMany" => "N:N",
        "hasManyThrough" => "1:N>",
        "hasOneThrough" => "1:1>",
        "morphTo" => "m→",
        "morphOne" => "m1",
        "morphMany" => "mN",
        "morphToMany" => "mNN",
        "morphedByMany" => "←mNN",
        _ => rel,
    }
}
