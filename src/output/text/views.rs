use std::fmt::Write as _;

use super::{header, new_table, terminal_width, wrap_cell};
use crate::types::{
    BladeComponentEntry, LivewireComponentEntry, MissingViewEntry, ViewEntry, ViewReport,
};

struct ViewWidths {
    name: usize,
    file: usize,
    kind: usize,
    vars: usize,
    source: usize,
}

struct ComponentWidths {
    component: usize,
    kind: usize,
    class_name: usize,
    view_name: usize,
    vars: usize,
    source: usize,
}

pub fn print_view_report(report: &ViewReport) {
    println!("{}", render_view_report(report));
}

pub fn render_view_report(report: &ViewReport) -> String {
    let mut output = String::new();
    let _ = writeln!(output, "Project: {}", report.project_name);
    let _ = writeln!(output, "Views: {}", report.view_count);
    let _ = writeln!(output, "Blade components: {}", report.blade_component_count);
    let _ = writeln!(
        output,
        "Livewire components: {}",
        report.livewire_component_count
    );
    let _ = writeln!(output, "Missing view refs: {}", report.missing_view_count);
    let _ = writeln!(output);

    let _ = writeln!(output, "Views");
    let _ = writeln!(output, "{}", render_views_table(&report.views));
    let _ = writeln!(output);
    let _ = writeln!(output, "Blade Components");
    let _ = writeln!(
        output,
        "{}",
        render_blade_components_table(&report.blade_components)
    );
    let _ = writeln!(output);
    let _ = writeln!(output, "Livewire Components");
    let _ = writeln!(
        output,
        "{}",
        render_livewire_components_table(&report.livewire_components)
    );
    let _ = writeln!(output);
    let _ = writeln!(output, "Missing View References");
    let _ = write!(
        output,
        "{}",
        render_missing_views_table(&report.missing_views)
    );
    output
}

fn render_views_table(views: &[ViewEntry]) -> String {
    if views.is_empty() {
        return "No views found.".to_string();
    }

    let widths = view_widths();
    let mut table = new_table();
    table.set_header(vec![
        header("View"),
        header("File"),
        header("Kind"),
        header("Inputs"),
        header("Declared In"),
    ]);

    for view in views {
        table.add_row(vec![
            wrap_cell(&view.name, widths.name),
            wrap_cell(&view.file.display().to_string(), widths.file),
            wrap_cell(&view.kind, widths.kind),
            wrap_cell(
                &display_variables(&view.props, &view.variables),
                widths.vars,
            ),
            wrap_cell(
                &format!(
                    "{}:{}:{}",
                    view.source.declared_in.display(),
                    view.source.line,
                    view.source.column
                ),
                widths.source,
            ),
        ]);
    }

    table.to_string()
}

fn render_blade_components_table(components: &[BladeComponentEntry]) -> String {
    if components.is_empty() {
        return "No Blade components found.".to_string();
    }

    let widths = component_widths();
    let mut table = new_table();
    table.set_header(vec![
        header("Component"),
        header("Kind"),
        header("Class"),
        header("View"),
        header("Props"),
        header("Declared In"),
    ]);

    for component in components {
        table.add_row(vec![
            wrap_cell(&component.component, widths.component),
            wrap_cell(&component.kind, widths.kind),
            wrap_cell(
                component.class_name.as_deref().unwrap_or("-"),
                widths.class_name,
            ),
            wrap_cell(
                component.view_name.as_deref().unwrap_or("-"),
                widths.view_name,
            ),
            wrap_cell(&display_list(&component.props), widths.vars),
            wrap_cell(
                &format!(
                    "{}:{}:{}",
                    component.source.declared_in.display(),
                    component.source.line,
                    component.source.column
                ),
                widths.source,
            ),
        ]);
    }

    table.to_string()
}

fn render_livewire_components_table(components: &[LivewireComponentEntry]) -> String {
    if components.is_empty() {
        return "No Livewire components found.".to_string();
    }

    let widths = component_widths();
    let mut table = new_table();
    table.set_header(vec![
        header("Component"),
        header("Kind"),
        header("Class"),
        header("View"),
        header("State"),
        header("Declared In"),
    ]);

    for component in components {
        table.add_row(vec![
            wrap_cell(&component.component, widths.component),
            wrap_cell(&component.kind, widths.kind),
            wrap_cell(
                component.class_name.as_deref().unwrap_or("-"),
                widths.class_name,
            ),
            wrap_cell(
                component.view_name.as_deref().unwrap_or("-"),
                widths.view_name,
            ),
            wrap_cell(&display_list(&component.state), widths.vars),
            wrap_cell(
                &format!(
                    "{}:{}:{}",
                    component.source.declared_in.display(),
                    component.source.line,
                    component.source.column
                ),
                widths.source,
            ),
        ]);
    }

    table.to_string()
}

fn render_missing_views_table(missing_views: &[MissingViewEntry]) -> String {
    if missing_views.is_empty() {
        return "No missing view references found.".to_string();
    }

    let widths = view_widths();
    let mut table = new_table();
    table.set_header(vec![
        header("View"),
        header("Expected File"),
        header("Inputs"),
        header("Referenced In"),
    ]);

    for missing in missing_views {
        let variables = missing
            .usages
            .iter()
            .flat_map(|usage| usage.variables.clone())
            .collect::<Vec<_>>();
        let referenced_in = missing
            .usages
            .iter()
            .map(|usage| {
                format!(
                    "{} [{}:{}:{}]",
                    usage.kind,
                    usage.source.declared_in.display(),
                    usage.source.line,
                    usage.source.column
                )
            })
            .collect::<Vec<_>>()
            .join(" | ");
        table.add_row(vec![
            wrap_cell(&missing.name, widths.name),
            wrap_cell(&missing.expected_file.display().to_string(), widths.file),
            wrap_cell(&display_list(&variables), widths.vars),
            wrap_cell(&referenced_in, widths.source),
        ]);
    }

    table.to_string()
}

fn view_widths() -> ViewWidths {
    let t = terminal_width();
    if t < 110 {
        ViewWidths {
            name: 18,
            file: 18,
            kind: 14,
            vars: 18,
            source: 18,
        }
    } else if t < 150 {
        ViewWidths {
            name: 24,
            file: 24,
            kind: 18,
            vars: 24,
            source: 24,
        }
    } else {
        ViewWidths {
            name: 30,
            file: 34,
            kind: 20,
            vars: 32,
            source: 30,
        }
    }
}

fn component_widths() -> ComponentWidths {
    let t = terminal_width();
    if t < 110 {
        ComponentWidths {
            component: 18,
            kind: 14,
            class_name: 16,
            view_name: 16,
            vars: 18,
            source: 18,
        }
    } else if t < 150 {
        ComponentWidths {
            component: 24,
            kind: 18,
            class_name: 20,
            view_name: 20,
            vars: 22,
            source: 22,
        }
    } else {
        ComponentWidths {
            component: 28,
            kind: 20,
            class_name: 24,
            view_name: 24,
            vars: 28,
            source: 26,
        }
    }
}

fn display_variables(
    props: &[crate::types::ViewVariable],
    variables: &[crate::types::ViewVariable],
) -> String {
    let mut parts = Vec::new();
    if !props.is_empty() {
        parts.push(format!("props={}", display_list(props)));
    }
    if !variables.is_empty() {
        parts.push(format!("vars={}", display_list(variables)));
    }
    if parts.is_empty() {
        "-".to_string()
    } else {
        parts.join(" | ")
    }
}

fn display_list(items: &[crate::types::ViewVariable]) -> String {
    if items.is_empty() {
        return "-".to_string();
    }
    items
        .iter()
        .map(|item| match &item.default_value {
            Some(default) => format!("{}={}", item.name, default),
            None => item.name.clone(),
        })
        .collect::<Vec<_>>()
        .join(", ")
}
