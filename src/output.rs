use crate::types::{ConfigReport, OutputMode, RouteEntry, RouteReport};

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

    let location_width = routes
        .iter()
        .map(|route| format!("{}:{}", route.line, route.column).len())
        .max()
        .unwrap_or(8)
        .max("LINE:COL".len());
    let method_width = routes
        .iter()
        .map(|route| route.methods.join("|").len())
        .max()
        .unwrap_or(6)
        .max("METHOD".len());
    let uri_width = routes
        .iter()
        .map(|route| route.uri.len())
        .max()
        .unwrap_or(3)
        .max("URI".len());
    let name_width = routes
        .iter()
        .map(|route| route.name.as_deref().unwrap_or("-").len())
        .max()
        .unwrap_or(4)
        .max("NAME".len());
    let action_width = routes
        .iter()
        .map(|route| route.action.as_deref().unwrap_or("-").len())
        .max()
        .unwrap_or(6)
        .max("ACTION".len());

    let mut current_file = None;
    for route in routes {
        let file = route.file.as_path();
        if current_file != Some(file) {
            if current_file.is_some() {
                println!();
            }
            println!("{}", file.display());
            println!(
                "  {location:<location_width$}  {method:<method_width$}  {uri:<uri_width$}  {name:<name_width$}  {action:<action_width$}  {middleware}",
                location = "LINE:COL",
                method = "METHOD",
                uri = "URI",
                name = "NAME",
                action = "ACTION",
                middleware = "MIDDLEWARE",
                location_width = location_width,
                method_width = method_width,
                uri_width = uri_width,
                name_width = name_width,
                action_width = action_width,
            );
            println!(
                "  {location:-<location_width$}  {method:-<method_width$}  {uri:-<uri_width$}  {name:-<name_width$}  {action:-<action_width$}  ----------",
                location = "",
                location_width = location_width,
                method = "",
                uri = "",
                name = "",
                action = "",
                method_width = method_width,
                uri_width = uri_width,
                name_width = name_width,
                action_width = action_width,
            );
            current_file = Some(file);
        }

        let methods = route.methods.join("|");
        let name = route.name.as_deref().unwrap_or("-");
        let action = route.action.as_deref().unwrap_or("-");
        let middleware = if route.middleware.is_empty() {
            "-".to_string()
        } else {
            route.middleware.join(",")
        };

        println!(
            "  {location:<location_width$}  {method:<method_width$}  {uri:<uri_width$}  {name:<name_width$}  {action:<action_width$}  {middleware}",
            location = format!("{}:{}", route.line, route.column),
            method = methods,
            uri = route.uri,
            name = name,
            action = action,
            middleware = middleware,
            location_width = location_width,
            method_width = method_width,
            uri_width = uri_width,
            name_width = name_width,
            action_width = action_width,
        );
    }
}

fn print_config_table(report: &ConfigReport) {
    if report.references.is_empty() {
        println!("No config items found.");
        return;
    }

    let location_width = report
        .references
        .iter()
        .map(|reference| format!("{}:{}", reference.line, reference.column).len())
        .max()
        .unwrap_or(8)
        .max("LINE:COL".len());
    let kind_width = report
        .references
        .iter()
        .map(|reference| reference.kind.len())
        .max()
        .unwrap_or(4)
        .max("KIND".len());
    let key_width = report
        .references
        .iter()
        .map(|reference| reference.key.len())
        .max()
        .unwrap_or(3)
        .max("KEY".len());

    let mut current_file = None;
    for reference in &report.references {
        let file = reference.file.as_path();
        if current_file != Some(file) {
            if current_file.is_some() {
                println!();
            }
            println!("{}", file.display());
            println!(
                "  {location:<location_width$}  {kind:<kind_width$}  {key:<key_width$}",
                location = "LINE:COL",
                kind = "KIND",
                key = "KEY",
                location_width = location_width,
                kind_width = kind_width,
                key_width = key_width
            );
            println!(
                "  {location:-<location_width$}  {kind:-<kind_width$}  {key:-<key_width$}",
                location = "",
                kind = "",
                key = "",
                location_width = location_width,
                kind_width = kind_width,
                key_width = key_width
            );
            current_file = Some(file);
        }

        println!(
            "  {location:<location_width$}  {kind:<kind_width$}  {key:<key_width$}",
            location = format!("{}:{}", reference.line, reference.column),
            kind = reference.kind,
            key = reference.key,
            location_width = location_width,
            kind_width = kind_width,
            key_width = key_width,
        );
    }
}
