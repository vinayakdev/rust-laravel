mod analyzers;
mod cli;
mod debug;
mod debug_web;
mod output;
mod php;
mod project;
mod types;

use cli::Command;

pub fn run() -> Result<(), String> {
    let options = cli::parse(std::env::args().skip(1))?;

    match options.command {
        Command::RouteList => {
            let project = project::resolve(options.project.as_deref())?;
            let report = analyzers::routes::analyze(&project)?;
            output::print_routes(&report, options.json)?;
        }
        Command::RouteSources => {
            let project = project::resolve(options.project.as_deref())?;
            let report = analyzers::routes::analyze(&project)?;
            output::print_route_sources(&report, options.json)?;
        }
        Command::MiddlewareList => {
            let project = project::resolve(options.project.as_deref())?;
            let report = analyzers::middleware::analyze(&project)?;
            output::print_middlewares(&report, options.json)?;
        }
        Command::ConfigList => {
            let project = project::resolve(options.project.as_deref())?;
            let report = analyzers::configs::analyze(&project)?;
            output::print_configs(&report, options.json)?;
        }
        Command::ConfigSources => {
            let project = project::resolve(options.project.as_deref())?;
            let report = analyzers::configs::analyze(&project)?;
            output::print_config_sources(&report, options.json)?;
        }
        Command::ProviderList => {
            let project = project::resolve(options.project.as_deref())?;
            let report = analyzers::providers::analyze(&project)?;
            output::print_providers(&report, options.json)?;
        }
        Command::ViewList => {
            let project = project::resolve(options.project.as_deref())?;
            let report = analyzers::views::analyze(&project)?;
            output::print_views(&report, options.json)?;
        }
        Command::DebugBrowse => {
            debug::run(options.project.as_deref())?;
        }
        Command::DebugWeb => {
            debug_web::run(options.project.as_deref())?;
        }
    }

    Ok(())
}
