mod json;
pub mod text;

use crate::types::{
    ConfigReport, MiddlewareReport, OutputMode, ProviderReport, RouteReport,
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
        OutputMode::Json => json::print_routes(report),
        OutputMode::Text => { text::routes::print_route_table(&report.routes); Ok(()) }
    }
}

pub fn print_route_sources(report: &RouteReport, mode: OutputMode) -> Result<(), String> {
    match mode {
        OutputMode::Json => json::print_routes(report),
        OutputMode::Text => { text::routes::print_route_source_table(&report.routes); Ok(()) }
    }
}

pub fn print_configs(report: &ConfigReport, mode: OutputMode) -> Result<(), String> {
    match mode {
        OutputMode::Json => json::print_configs(report),
        OutputMode::Text => { text::configs::print_config_table(report); Ok(()) }
    }
}

pub fn print_config_sources(report: &ConfigReport, mode: OutputMode) -> Result<(), String> {
    match mode {
        OutputMode::Json => json::print_configs(report),
        OutputMode::Text => { text::configs::print_config_source_table(report); Ok(()) }
    }
}

pub fn print_providers(report: &ProviderReport, mode: OutputMode) -> Result<(), String> {
    match mode {
        OutputMode::Json => json::print_providers(report),
        OutputMode::Text => { text::providers::print_provider_table(report); Ok(()) }
    }
}

pub fn print_middlewares(report: &MiddlewareReport, mode: OutputMode) -> Result<(), String> {
    match mode {
        OutputMode::Json => json::print_middlewares(report),
        OutputMode::Text => { text::middleware::print_middleware_tables(report); Ok(()) }
    }
}
