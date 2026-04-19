mod analyzers;
mod cli;
mod output;
mod project;
mod types;

use cli::Command;

pub fn run() -> Result<(), String> {
    let options = cli::parse(std::env::args().skip(1))?;
    let project = project::resolve(options.project.as_deref())?;

    match options.command {
        Command::RouteList => {
            let report = analyzers::routes::analyze(&project)?;
            output::print_routes(&report, options.json)?;
        }
        Command::ConfigList => {
            let report = analyzers::configs::analyze(&project)?;
            output::print_configs(&report, options.json)?;
        }
    }

    Ok(())
}
