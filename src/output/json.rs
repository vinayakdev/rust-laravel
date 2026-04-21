use crate::types::{
    ConfigReport, ControllerReport, MiddlewareReport, MigrationReport, ModelReport, ProviderReport,
    RouteReport, ViewReport,
};

pub fn print_routes(report: &RouteReport) -> Result<(), String> {
    println!(
        "{}",
        serde_json::to_string_pretty(report).map_err(|e| e.to_string())?
    );
    Ok(())
}

pub fn print_configs(report: &ConfigReport) -> Result<(), String> {
    println!(
        "{}",
        serde_json::to_string_pretty(report).map_err(|e| e.to_string())?
    );
    Ok(())
}

pub fn print_controllers(report: &ControllerReport) -> Result<(), String> {
    println!(
        "{}",
        serde_json::to_string_pretty(report).map_err(|e| e.to_string())?
    );
    Ok(())
}

pub fn print_providers(report: &ProviderReport) -> Result<(), String> {
    println!(
        "{}",
        serde_json::to_string_pretty(report).map_err(|e| e.to_string())?
    );
    Ok(())
}

pub fn print_middlewares(report: &MiddlewareReport) -> Result<(), String> {
    println!(
        "{}",
        serde_json::to_string_pretty(report).map_err(|e| e.to_string())?
    );
    Ok(())
}

pub fn print_views(report: &ViewReport) -> Result<(), String> {
    println!(
        "{}",
        serde_json::to_string_pretty(report).map_err(|e| e.to_string())?
    );
    Ok(())
}

pub fn print_models(report: &ModelReport) -> Result<(), String> {
    println!(
        "{}",
        serde_json::to_string_pretty(report).map_err(|e| e.to_string())?
    );
    Ok(())
}

pub fn print_migrations(report: &MigrationReport) -> Result<(), String> {
    println!(
        "{}",
        serde_json::to_string_pretty(report).map_err(|e| e.to_string())?
    );
    Ok(())
}
