use crate::types::{
    ConfigReport, ControllerReport, MiddlewareReport, MigrationReport, ModelReport, OutputMode,
    ProviderReport, PublicAssetReport, RouteReport, ViewReport,
};

#[allow(dead_code)]
/// Shared contract for rendering a report. Each report type gets one text
/// renderer module and one JSON renderer function.
pub trait Reporter<T> {
    fn render_text(data: &T) -> Result<(), String>;
    fn render_json(data: &T) -> Result<(), String>;
}

pub fn print_routes(report: &RouteReport, mode: OutputMode) -> Result<(), String> {
    match mode {
        OutputMode::Json => rust_php_output::json::print_routes(report),
        OutputMode::Text => {
            rust_php_output::text::routes::print_route_table(&report.routes);
            Ok(())
        }
    }
}

pub fn print_route_sources(report: &RouteReport, mode: OutputMode) -> Result<(), String> {
    match mode {
        OutputMode::Json => rust_php_output::json::print_routes(report),
        OutputMode::Text => {
            rust_php_output::text::routes::print_route_source_table(&report.routes);
            Ok(())
        }
    }
}

pub fn print_configs(report: &ConfigReport, mode: OutputMode) -> Result<(), String> {
    match mode {
        OutputMode::Json => rust_php_output::json::print_configs(report),
        OutputMode::Text => {
            rust_php_output::text::configs::print_config_table(report);
            Ok(())
        }
    }
}

pub fn print_config_sources(report: &ConfigReport, mode: OutputMode) -> Result<(), String> {
    match mode {
        OutputMode::Json => rust_php_output::json::print_configs(report),
        OutputMode::Text => {
            rust_php_output::text::configs::print_config_source_table(report);
            Ok(())
        }
    }
}

pub fn print_controllers(report: &ControllerReport, mode: OutputMode) -> Result<(), String> {
    match mode {
        OutputMode::Json => rust_php_output::json::print_controllers(report),
        OutputMode::Text => {
            rust_php_output::text::controllers::print_controller_report(report);
            Ok(())
        }
    }
}

pub fn print_providers(report: &ProviderReport, mode: OutputMode) -> Result<(), String> {
    match mode {
        OutputMode::Json => rust_php_output::json::print_providers(report),
        OutputMode::Text => {
            rust_php_output::text::providers::print_provider_table(report);
            Ok(())
        }
    }
}

pub fn print_middlewares(report: &MiddlewareReport, mode: OutputMode) -> Result<(), String> {
    match mode {
        OutputMode::Json => rust_php_output::json::print_middlewares(report),
        OutputMode::Text => {
            rust_php_output::text::middleware::print_middleware_tables(report);
            Ok(())
        }
    }
}

pub fn print_views(report: &ViewReport, mode: OutputMode) -> Result<(), String> {
    match mode {
        OutputMode::Json => rust_php_output::json::print_views(report),
        OutputMode::Text => {
            rust_php_output::text::views::print_view_report(report);
            Ok(())
        }
    }
}

pub fn print_models(report: &ModelReport, mode: OutputMode) -> Result<(), String> {
    match mode {
        OutputMode::Json => rust_php_output::json::print_models(report),
        OutputMode::Text => {
            rust_php_output::text::models::print_model_report(report);
            Ok(())
        }
    }
}

pub fn print_migrations(report: &MigrationReport, mode: OutputMode) -> Result<(), String> {
    match mode {
        OutputMode::Json => rust_php_output::json::print_migrations(report),
        OutputMode::Text => {
            rust_php_output::text::models::print_migration_report(report);
            Ok(())
        }
    }
}

pub fn print_public_assets(report: &PublicAssetReport, mode: OutputMode) -> Result<(), String> {
    match mode {
        OutputMode::Json => rust_php_output::json::print_public_assets(report),
        OutputMode::Text => {
            rust_php_output::text::public_assets::print_public_asset_report(report);
            Ok(())
        }
    }
}
