use crate::types::{
    ConfigReport, ControllerReport, MiddlewareReport, MigrationReport, ModelReport, OutputMode,
    ProviderReport, PublicAssetReport, RouteReport, ViewReport,
};

fn dispatch(
    mode: OutputMode,
    json: impl FnOnce() -> Result<(), String>,
    text: impl FnOnce(),
) -> Result<(), String> {
    match mode {
        OutputMode::Json => json(),
        OutputMode::Text => {
            text();
            Ok(())
        }
    }
}

pub fn print_routes(report: &RouteReport, mode: OutputMode) -> Result<(), String> {
    dispatch(mode, || rust_php_output::json::print_routes(report), || rust_php_output::text::routes::print_route_table(&report.routes))
}

pub fn print_route_sources(report: &RouteReport, mode: OutputMode) -> Result<(), String> {
    dispatch(mode, || rust_php_output::json::print_routes(report), || rust_php_output::text::routes::print_route_source_table(&report.routes))
}

pub fn print_configs(report: &ConfigReport, mode: OutputMode) -> Result<(), String> {
    dispatch(mode, || rust_php_output::json::print_configs(report), || rust_php_output::text::configs::print_config_table(report))
}

pub fn print_config_sources(report: &ConfigReport, mode: OutputMode) -> Result<(), String> {
    dispatch(mode, || rust_php_output::json::print_configs(report), || rust_php_output::text::configs::print_config_source_table(report))
}

pub fn print_controllers(report: &ControllerReport, mode: OutputMode) -> Result<(), String> {
    dispatch(mode, || rust_php_output::json::print_controllers(report), || rust_php_output::text::controllers::print_controller_report(report))
}

pub fn print_providers(report: &ProviderReport, mode: OutputMode) -> Result<(), String> {
    dispatch(mode, || rust_php_output::json::print_providers(report), || rust_php_output::text::providers::print_provider_table(report))
}

pub fn print_middlewares(report: &MiddlewareReport, mode: OutputMode) -> Result<(), String> {
    dispatch(mode, || rust_php_output::json::print_middlewares(report), || rust_php_output::text::middleware::print_middleware_tables(report))
}

pub fn print_views(report: &ViewReport, mode: OutputMode) -> Result<(), String> {
    dispatch(mode, || rust_php_output::json::print_views(report), || rust_php_output::text::views::print_view_report(report))
}

pub fn print_livewire(report: &ViewReport, mode: OutputMode) -> Result<(), String> {
    dispatch(mode, || rust_php_output::json::print_livewire(report), || rust_php_output::text::views::print_livewire_report(report))
}

pub fn print_models(report: &ModelReport, mode: OutputMode) -> Result<(), String> {
    dispatch(mode, || rust_php_output::json::print_models(report), || rust_php_output::text::models::print_model_report(report))
}

pub fn print_migrations(report: &MigrationReport, mode: OutputMode) -> Result<(), String> {
    dispatch(mode, || rust_php_output::json::print_migrations(report), || rust_php_output::text::models::print_migration_report(report))
}

pub fn print_public_assets(report: &PublicAssetReport, mode: OutputMode) -> Result<(), String> {
    dispatch(mode, || rust_php_output::json::print_public_assets(report), || rust_php_output::text::public_assets::print_public_asset_report(report))
}
