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
                "  {line:>4}  {method:<method_width$}  {uri:<uri_width$}  {name:<name_width$}  {action:<action_width$}  {middleware}",
                line = "LINE",
                method = "METHOD",
                uri = "URI",
                name = "NAME",
                action = "ACTION",
                middleware = "MIDDLEWARE",
                method_width = method_width,
                uri_width = uri_width,
                name_width = name_width,
                action_width = action_width,
            );
            println!(
                "  {line:>4}  {method:-<method_width$}  {uri:-<uri_width$}  {name:-<name_width$}  {action:-<action_width$}  ----------",
                line = "",
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
            "  {line:>4}  {method:<method_width$}  {uri:<uri_width$}  {name:<name_width$}  {action:<action_width$}  {middleware}",
            line = route.line,
            method = methods,
            uri = route.uri,
            name = name,
            action = action,
            middleware = middleware,
            method_width = method_width,
            uri_width = uri_width,
            name_width = name_width,
            action_width = action_width,
        );
    }
}

fn print_config_table(report: &ConfigReport) {
    if report.references.is_empty() {
        println!("No config(...) references found.");
        return;
    }

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
                "  LINE  {key:<key_width$}",
                key = "KEY",
                key_width = key_width
            );
            println!(
                "  ----  {key:-<key_width$}",
                key = "",
                key_width = key_width
            );
            current_file = Some(file);
        }

        println!(
            "  {line:>4}  {key:<key_width$}",
            line = reference.line,
            key = reference.key,
            key_width = key_width,
        );
    }
}
