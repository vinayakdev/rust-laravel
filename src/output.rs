use crate::types::{
    ConfigItem, ConfigReport, ConfigSource, MiddlewareAlias, MiddlewareGroup, MiddlewareReport,
    OutputMode, ProviderEntry, ProviderReport, RouteEntry, RoutePattern, RouteRegistration,
    RouteReport,
};
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

pub fn print_route_sources(report: &RouteReport, mode: OutputMode) -> Result<(), String> {
    match mode {
        OutputMode::Json => {
            let json = serde_json::to_string_pretty(report).map_err(|error| error.to_string())?;
            println!("{json}");
        }
        OutputMode::Text => print_route_source_table(&report.routes),
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

pub fn print_config_sources(report: &ConfigReport, mode: OutputMode) -> Result<(), String> {
    match mode {
        OutputMode::Json => {
            let json = serde_json::to_string_pretty(report).map_err(|error| error.to_string())?;
            println!("{json}");
        }
        OutputMode::Text => print_config_source_table(report),
    }

    Ok(())
}

pub fn print_providers(report: &ProviderReport, mode: OutputMode) -> Result<(), String> {
    match mode {
        OutputMode::Json => {
            let json = serde_json::to_string_pretty(report).map_err(|error| error.to_string())?;
            println!("{json}");
        }
        OutputMode::Text => print_provider_table(report),
    }

    Ok(())
}

pub fn print_middlewares(report: &MiddlewareReport, mode: OutputMode) -> Result<(), String> {
    match mode {
        OutputMode::Json => {
            let json = serde_json::to_string_pretty(report).map_err(|error| error.to_string())?;
            println!("{json}");
        }
        OutputMode::Text => print_middleware_tables(report),
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
                header("Patterns"),
                header("Registered Via"),
            ]);
            current_file = Some(file);
        }

        table.add_row(Row::from(vec![
            location_cell(route.line, route.column),
            Cell::new(route.methods.join("|")),
            wrap_cell(&route.uri, widths.uri),
            wrap_cell(route.name.as_deref().unwrap_or("-"), widths.name),
            wrap_cell(route.action.as_deref().unwrap_or("-"), widths.action),
            wrap_cell(&display_middleware(route), widths.middleware),
            wrap_cell(&display_patterns(route), widths.patterns),
            wrap_cell(
                &route_registration_summary(&route.registration),
                widths.registration,
            ),
        ]));
    }

    println!("{table}");
}

fn print_route_source_table(routes: &[RouteEntry]) {
    if routes.is_empty() {
        println!("No routes found.");
        return;
    }

    let widths = route_source_widths();
    let mut table = new_table();
    table.set_header(vec![
        header("Route"),
        header("Method"),
        header("Uri"),
        header("Provider"),
        header("Declared At"),
        header("Kind"),
    ]);

    for route in routes {
        table.add_row(Row::from(vec![
            wrap_cell(
                &format!("{}:{}:{}", route.file.display(), route.line, route.column),
                widths.route,
            ),
            Cell::new(route.methods.join("|")),
            wrap_cell(&route.uri, widths.uri),
            provider_registration_cell(&route.registration, widths.provider),
            wrap_cell(
                &format!(
                    "{}:{}:{}",
                    route.registration.declared_in.display(),
                    route.registration.line,
                    route.registration.column
                ),
                widths.declared_at,
            ),
            registration_kind_cell(&route.registration, widths.kind),
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
                header("Registered Via"),
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

fn print_provider_table(report: &ProviderReport) {
    if report.providers.is_empty() {
        println!("No providers found.");
        return;
    }

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

    println!("Project: {}", report.project_name);
    println!("Declared providers: {}", report.provider_count);
    println!("{table}");
    println!("Legend: green = source resolved, red = source missing, grey = not package-backed");
}

fn print_middleware_tables(report: &MiddlewareReport) {
    println!("Project: {}", report.project_name);

    if report.aliases.is_empty() && report.groups.is_empty() && report.patterns.is_empty() {
        println!("No middleware or route patterns found.");
        return;
    }

    if !report.aliases.is_empty() {
        let widths = middleware_widths();
        let mut table = new_table();
        table.set_header(vec![
            header("Alias"),
            header("Target"),
            header("Declared At"),
            header("Provider"),
        ]);
        for alias in &report.aliases {
            table.add_row(middleware_alias_row(alias, &widths));
        }
        println!("Aliases");
        println!("{table}");
        println!();
    }

    if !report.groups.is_empty() {
        let widths = middleware_widths();
        let mut table = new_table();
        table.set_header(vec![
            header("Group"),
            header("Members"),
            header("Declared At"),
            header("Provider"),
        ]);
        for group in &report.groups {
            table.add_row(middleware_group_row(group, &widths));
        }
        println!("Groups");
        println!("{table}");
        println!();
    }

    if !report.patterns.is_empty() {
        let widths = middleware_widths();
        let mut table = new_table();
        table.set_header(vec![
            header("Param"),
            header("Pattern"),
            header("Declared At"),
            header("Provider"),
        ]);
        for pattern in &report.patterns {
            table.add_row(route_pattern_row(pattern, &widths));
        }
        println!("Patterns");
        println!("{table}");
    }
}

fn print_config_source_table(report: &ConfigReport) {
    if report.items.is_empty() {
        println!("No config items found.");
        return;
    }

    let widths = config_source_widths();
    let mut table = new_table();
    table.set_header(vec![
        header("Config"),
        header("Env Key"),
        header("Provider"),
        header("Declared At"),
        header("Kind"),
    ]);

    for item in &report.items {
        table.add_row(Row::from(vec![
            wrap_cell(
                &format!(
                    "{}:{}:{} ({})",
                    item.file.display(),
                    item.line,
                    item.column,
                    item.key
                ),
                widths.config,
            ),
            env_key_cell(item, widths.env_key),
            config_provider_cell(&item.source, widths.provider),
            wrap_cell(
                &format!(
                    "{}:{}:{}",
                    item.source.declared_in.display(),
                    item.source.line,
                    item.source.column
                ),
                widths.declared_at,
            ),
            config_source_kind_cell(&item.source, widths.kind),
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
        wrap_cell(&config_source_summary(&item.source), widths.registration),
    ])
}

fn provider_row(provider: &ProviderEntry, widths: &ProviderWidths) -> Row {
    Row::from(vec![
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
    patterns: usize,
    registration: usize,
}

struct ConfigWidths {
    key: usize,
    env_key: usize,
    default: usize,
    env_value: usize,
    registration: usize,
}

struct ProviderWidths {
    declared_in: usize,
    provider: usize,
    kind: usize,
    package: usize,
    source: usize,
    status: usize,
}

struct RouteSourceWidths {
    route: usize,
    uri: usize,
    provider: usize,
    declared_at: usize,
    kind: usize,
}

struct ConfigSourceWidths {
    config: usize,
    env_key: usize,
    provider: usize,
    declared_at: usize,
    kind: usize,
}

struct MiddlewareWidths {
    name: usize,
    detail: usize,
    declared_at: usize,
    provider: usize,
}

fn route_widths() -> RouteWidths {
    let terminal = terminal_width();
    if terminal < 110 {
        RouteWidths {
            uri: 18,
            name: 16,
            action: 20,
            middleware: 14,
            patterns: 16,
            registration: 18,
        }
    } else if terminal < 150 {
        RouteWidths {
            uri: 24,
            name: 20,
            action: 28,
            middleware: 18,
            patterns: 18,
            registration: 24,
        }
    } else {
        RouteWidths {
            uri: 34,
            name: 26,
            action: 42,
            middleware: 24,
            patterns: 22,
            registration: 32,
        }
    }
}

fn middleware_widths() -> MiddlewareWidths {
    let terminal = terminal_width();
    if terminal < 110 {
        MiddlewareWidths {
            name: 14,
            detail: 22,
            declared_at: 18,
            provider: 18,
        }
    } else if terminal < 150 {
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

fn route_source_widths() -> RouteSourceWidths {
    let terminal = terminal_width();
    if terminal < 110 {
        RouteSourceWidths {
            route: 18,
            uri: 18,
            provider: 18,
            declared_at: 18,
            kind: 14,
        }
    } else if terminal < 150 {
        RouteSourceWidths {
            route: 28,
            uri: 24,
            provider: 24,
            declared_at: 24,
            kind: 18,
        }
    } else {
        RouteSourceWidths {
            route: 38,
            uri: 30,
            provider: 30,
            declared_at: 34,
            kind: 20,
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
            registration: 18,
        }
    } else if terminal < 150 {
        ConfigWidths {
            key: 32,
            env_key: 20,
            default: 18,
            env_value: 20,
            registration: 24,
        }
    } else {
        ConfigWidths {
            key: 42,
            env_key: 26,
            default: 26,
            env_value: 26,
            registration: 32,
        }
    }
}

fn config_source_widths() -> ConfigSourceWidths {
    let terminal = terminal_width();
    if terminal < 110 {
        ConfigSourceWidths {
            config: 22,
            env_key: 16,
            provider: 18,
            declared_at: 18,
            kind: 14,
        }
    } else if terminal < 150 {
        ConfigSourceWidths {
            config: 34,
            env_key: 20,
            provider: 24,
            declared_at: 24,
            kind: 18,
        }
    } else {
        ConfigSourceWidths {
            config: 46,
            env_key: 26,
            provider: 30,
            declared_at: 34,
            kind: 22,
        }
    }
}

fn provider_widths() -> ProviderWidths {
    let terminal = terminal_width();
    if terminal < 110 {
        ProviderWidths {
            declared_in: 18,
            provider: 20,
            kind: 12,
            package: 16,
            source: 18,
            status: 14,
        }
    } else if terminal < 150 {
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

fn terminal_width() -> usize {
    std::env::var("COLUMNS")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .filter(|value| *value > 40)
        .unwrap_or(160)
}

fn package_cell(provider: &ProviderEntry, width: usize) -> Cell {
    let text = provider.package_name.as_deref().unwrap_or("-");
    let mut cell = wrap_cell(text, width);
    if provider.package_name.is_some() {
        cell = cell.fg(Color::Cyan);
    } else {
        cell = cell.fg(Color::DarkGrey);
    }
    cell
}

fn source_cell(provider: &ProviderEntry, width: usize) -> Cell {
    let text = provider
        .source_file
        .as_ref()
        .map(|path| path.display().to_string())
        .unwrap_or_else(|| "-".to_string());
    let mut cell = wrap_cell(&text, width);
    if provider.source_available {
        cell = cell.fg(Color::Green);
    } else {
        cell = cell.fg(Color::Red);
    }
    cell
}

fn status_cell(provider: &ProviderEntry, width: usize) -> Cell {
    let mut cell = wrap_cell(&provider.status, width);
    if provider.source_available {
        cell = cell.fg(Color::Green);
    } else {
        cell = cell.fg(Color::Red);
    }
    cell
}

fn route_registration_summary(registration: &RouteRegistration) -> String {
    match &registration.provider_class {
        Some(provider) => format!(
            "{provider} @ {}:{}:{}",
            registration.declared_in.display(),
            registration.line,
            registration.column
        ),
        None => registration.kind.clone(),
    }
}

fn provider_registration_cell(registration: &RouteRegistration, width: usize) -> Cell {
    let text = registration.provider_class.as_deref().unwrap_or("-");
    let mut cell = wrap_cell(text, width);
    if registration.provider_class.is_some() {
        cell = cell.fg(Color::Cyan);
    } else {
        cell = cell.fg(Color::DarkGrey);
    }
    cell
}

fn registration_kind_cell(registration: &RouteRegistration, width: usize) -> Cell {
    let mut cell = wrap_cell(&registration.kind, width);
    if registration.provider_class.is_some() {
        cell = cell.fg(Color::Green);
    } else {
        cell = cell.fg(Color::DarkGrey);
    }
    cell
}

fn config_source_summary(source: &ConfigSource) -> String {
    match &source.provider_class {
        Some(provider) => format!(
            "{provider} @ {}:{}:{}",
            source.declared_in.display(),
            source.line,
            source.column
        ),
        None => source.kind.clone(),
    }
}

fn config_provider_cell(source: &ConfigSource, width: usize) -> Cell {
    let text = source.provider_class.as_deref().unwrap_or("-");
    let mut cell = wrap_cell(text, width);
    if source.provider_class.is_some() {
        cell = cell.fg(Color::Cyan);
    } else {
        cell = cell.fg(Color::DarkGrey);
    }
    cell
}

fn config_source_kind_cell(source: &ConfigSource, width: usize) -> Cell {
    let mut cell = wrap_cell(&source.kind, width);
    if source.provider_class.is_some() {
        cell = cell.fg(Color::Green);
    } else {
        cell = cell.fg(Color::DarkGrey);
    }
    cell
}

fn display_middleware(route: &RouteEntry) -> String {
    let values = if route.resolved_middleware.is_empty() {
        &route.middleware
    } else {
        &route.resolved_middleware
    };
    join_or_dash(values)
}

fn display_patterns(route: &RouteEntry) -> String {
    if route.parameter_patterns.is_empty() {
        return "-".to_string();
    }

    route
        .parameter_patterns
        .iter()
        .map(|(name, pattern)| format!("{name}={pattern}"))
        .collect::<Vec<_>>()
        .join(",")
}

fn middleware_alias_row(alias: &MiddlewareAlias, widths: &MiddlewareWidths) -> Row {
    Row::from(vec![
        wrap_cell(&alias.name, widths.name),
        wrap_cell(&alias.target, widths.detail),
        wrap_cell(
            &format!(
                "{}:{}:{}",
                alias.source.declared_in.display(),
                alias.source.line,
                alias.source.column
            ),
            widths.declared_at,
        ),
        wrap_cell(&alias.source.provider_class, widths.provider),
    ])
}

fn middleware_group_row(group: &MiddlewareGroup, widths: &MiddlewareWidths) -> Row {
    Row::from(vec![
        wrap_cell(&group.name, widths.name),
        wrap_cell(&group.members.join(","), widths.detail),
        wrap_cell(
            &format!(
                "{}:{}:{}",
                group.source.declared_in.display(),
                group.source.line,
                group.source.column
            ),
            widths.declared_at,
        ),
        wrap_cell(&group.source.provider_class, widths.provider),
    ])
}

fn route_pattern_row(pattern: &RoutePattern, widths: &MiddlewareWidths) -> Row {
    Row::from(vec![
        wrap_cell(&pattern.parameter, widths.name),
        wrap_cell(&pattern.pattern, widths.detail),
        wrap_cell(
            &format!(
                "{}:{}:{}",
                pattern.source.declared_in.display(),
                pattern.source.line,
                pattern.source.column
            ),
            widths.declared_at,
        ),
        wrap_cell(&pattern.source.provider_class, widths.provider),
    ])
}
