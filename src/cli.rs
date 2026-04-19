use crate::types::OutputMode;

#[derive(Clone, Copy)]
pub enum Command {
    RouteList,
    RouteSources,
    ConfigList,
    ConfigSources,
    ProviderList,
}

pub struct CliOptions {
    pub command: Command,
    pub json: OutputMode,
    pub project: Option<String>,
}

pub fn parse<I>(args: I) -> Result<CliOptions, String>
where
    I: IntoIterator<Item = String>,
{
    let mut command = Command::RouteList;
    let mut json = OutputMode::Text;
    let mut project = None;
    let mut iter = args.into_iter();

    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "route:list" => command = Command::RouteList,
            "route:sources" => command = Command::RouteSources,
            "config:list" => command = Command::ConfigList,
            "config:sources" => command = Command::ConfigSources,
            "provider:list" => command = Command::ProviderList,
            "--json" => json = OutputMode::Json,
            "--project" => {
                let value = iter
                    .next()
                    .ok_or_else(|| "--project expects a value".to_string())?;
                project = Some(value);
            }
            "--help" | "-h" | "help" => return Err(help_text()),
            other => return Err(format!("unknown argument: {other}\n\n{}", help_text())),
        }
    }

    Ok(CliOptions {
        command,
        json,
        project,
    })
}

fn help_text() -> String {
    [
        "Usage:",
        "  rust-php route:list [--project <path-or-name>] [--json]",
        "  rust-php route:sources [--project <path-or-name>] [--json]",
        "  rust-php config:list [--project <path-or-name>] [--json]",
        "  rust-php config:sources [--project <path-or-name>] [--json]",
        "  rust-php provider:list [--project <path-or-name>] [--json]",
        "",
        "Project resolution:",
        "  1. If --project is a real path, use it.",
        "  2. Otherwise resolve it under ./laravel-example/<name>.",
        "  3. With no --project, use the current directory if it looks like Laravel.",
        "  4. Otherwise auto-pick a single Laravel project under ./laravel-example.",
        "",
        "Examples:",
        "  cargo run -- route:list",
        "  cargo run -- route:sources --project sandbox-app",
        "  cargo run -- config:sources --project sandbox-app",
        "  cargo run -- provider:list --project sandbox-app",
        "  cargo run -- route:list --project laravel-example/demo-app",
        "  cargo run -- route:list --project demo-app --json",
        "  ./target/release/rust-php config:list --project demo-app",
    ]
    .join("\n")
}
