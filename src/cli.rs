use crate::export::{ExportOptions, PickerMode};
use crate::types::OutputMode;

#[derive(Clone, Copy)]
pub enum Command {
    RouteList,
    RouteSources,
    MiddlewareList,
    ConfigList,
    ConfigSources,
    ControllerList,
    ProviderList,
    ViewList,
    LivewireList,
    ModelList,
    MigrationList,
    PublicList,
    Lsp,
    ExportLsp,
    BenchmarkIndex,
    DebugBrowse,
    DebugWeb,
}

pub struct CliOptions {
    pub command: Command,
    pub json: OutputMode,
    pub project: Option<String>,
    pub export: ExportOptions,
}

pub fn parse<I>(args: I) -> Result<CliOptions, String>
where
    I: IntoIterator<Item = String>,
{
    let mut command = Command::RouteList;
    let mut json = OutputMode::Text;
    let mut project = None;
    let mut export = ExportOptions {
        output_dir: "test/output".into(),
        picker: PickerMode::Arrows,
        projects: Vec::new(),
        jobs: None,
    };
    let mut iter = args.into_iter();

    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "route:list" => command = Command::RouteList,
            "route:sources" => command = Command::RouteSources,
            "middleware:list" => command = Command::MiddlewareList,
            "config:list" => command = Command::ConfigList,
            "config:sources" => command = Command::ConfigSources,
            "controller:list" => command = Command::ControllerList,
            "provider:list" => command = Command::ProviderList,
            "view:list" => command = Command::ViewList,
            "livewire:list" => command = Command::LivewireList,
            "model:list" => command = Command::ModelList,
            "migration:list" => command = Command::MigrationList,
            "public:list" => command = Command::PublicList,
            "lsp" => command = Command::Lsp,
            "export:lsp" => command = Command::ExportLsp,
            "benchmark:index" => command = Command::BenchmarkIndex,
            "debug:browse" => command = Command::DebugBrowse,
            "debug:web" => command = Command::DebugWeb,
            "--json" => json = OutputMode::Json,
            "--project" => {
                let value = iter
                    .next()
                    .ok_or_else(|| "--project expects a value".to_string())?;
                project = Some(value);
            }
            "--projects" => {
                let value = iter
                    .next()
                    .ok_or_else(|| "--projects expects a comma-separated value".to_string())?;
                export.projects = value
                    .split(',')
                    .map(str::trim)
                    .filter(|item| !item.is_empty())
                    .map(ToOwned::to_owned)
                    .collect();
            }
            "--out" => {
                let value = iter
                    .next()
                    .ok_or_else(|| "--out expects a directory path".to_string())?;
                export.output_dir = value.into();
            }
            "--picker" => {
                let value = iter
                    .next()
                    .ok_or_else(|| "--picker expects a value".to_string())?;
                export.picker = PickerMode::parse(&value)?;
            }
            "--jobs" => {
                let value = iter
                    .next()
                    .ok_or_else(|| "--jobs expects a positive integer".to_string())?;
                let jobs = value
                    .parse::<usize>()
                    .map_err(|_| "--jobs expects a positive integer".to_string())?;
                if jobs == 0 {
                    return Err("--jobs expects a positive integer".to_string());
                }
                export.jobs = Some(jobs);
            }
            "--help" | "-h" | "help" => return Err(help_text()),
            other => return Err(format!("unknown argument: {other}\n\n{}", help_text())),
        }
    }

    Ok(CliOptions {
        command,
        json,
        project,
        export,
    })
}

fn help_text() -> String {
    [
        "Usage:",
        "  rust-php route:list [--project <path-or-name>] [--json]",
        "  rust-php route:sources [--project <path-or-name>] [--json]",
        "  rust-php middleware:list [--project <path-or-name>] [--json]",
        "  rust-php config:list [--project <path-or-name>] [--json]",
        "  rust-php config:sources [--project <path-or-name>] [--json]",
        "  rust-php controller:list [--project <path-or-name>] [--json]",
        "  rust-php provider:list [--project <path-or-name>] [--json]",
        "  rust-php view:list [--project <path-or-name>] [--json]",
        "  rust-php livewire:list [--project <path-or-name>] [--json]",
        "  rust-php model:list [--project <path-or-name>] [--json]",
        "  rust-php migration:list [--project <path-or-name>] [--json]",
        "  rust-php public:list [--project <path-or-name>] [--json]",
        "  rust-php lsp",
        "  rust-php export:lsp [--projects <name,...>] [--picker <arrows|fuzzy>] [--out <dir>] [--jobs <n>]",
        "  rust-php benchmark:index [--project <path-or-name>] [--json]",
        "  rust-php debug:browse [--project <path-or-name>]",
        "  rust-php debug:web [--project <path-or-name>]",
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
        "  cargo run -- middleware:list --project sandbox-app",
        "  cargo run -- config:sources --project sandbox-app",
        "  cargo run -- controller:list --project sandbox-app",
        "  cargo run -- provider:list --project sandbox-app",
        "  cargo run -- view:list --project starter-demo",
        "  cargo run -- livewire:list --project starter-demo",
        "  cargo run -- model:list --project sandbox-app",
        "  cargo run -- migration:list --project sandbox-app",
        "  cargo run -- public:list --project sandbox-app",
        "  cargo run -- lsp",
        "  cargo run -- export:lsp --picker arrows --out test/output",
        "  cargo run -- export:lsp --projects sandbox-app,demo-app --jobs 4",
        "  cargo run -- benchmark:index --project sandbox-app",
        "  cargo run -- debug:browse",
        "  cargo run -- debug:web",
        "  cargo run -- route:list --project laravel-example/demo-app",
        "  cargo run -- route:list --project demo-app --json",
        "  ./target/release/rust-php config:list --project demo-app",
    ]
    .join("\n")
}
