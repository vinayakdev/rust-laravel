use crate::project::LaravelProject;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DebugCommand {
    Dashboard,
    RouteList,
    RouteCompare,
    RouteSources,
    MiddlewareList,
    ConfigList,
    ConfigSources,
    ControllerList,
    ProviderList,
    ViewList,
    ModelList,
    MigrationList,
    PublicList,
    VendorList,
}

pub(crate) const BROWSER_COMMANDS: [DebugCommand; 12] = [
    DebugCommand::RouteList,
    DebugCommand::RouteSources,
    DebugCommand::MiddlewareList,
    DebugCommand::ConfigList,
    DebugCommand::ConfigSources,
    DebugCommand::ControllerList,
    DebugCommand::ProviderList,
    DebugCommand::ViewList,
    DebugCommand::ModelList,
    DebugCommand::MigrationList,
    DebugCommand::PublicList,
    DebugCommand::VendorList,
];

impl DebugCommand {
    pub(crate) fn label(self) -> &'static str {
        match self {
            DebugCommand::Dashboard => "dashboard",
            DebugCommand::RouteList => "route:list",
            DebugCommand::RouteCompare => "route:compare",
            DebugCommand::RouteSources => "route:sources",
            DebugCommand::MiddlewareList => "middleware:list",
            DebugCommand::ConfigList => "config:list",
            DebugCommand::ConfigSources => "config:sources",
            DebugCommand::ControllerList => "controller:list",
            DebugCommand::ProviderList => "provider:list",
            DebugCommand::ViewList => "view:list",
            DebugCommand::ModelList => "model:list",
            DebugCommand::MigrationList => "migration:list",
            DebugCommand::PublicList => "public:list",
            DebugCommand::VendorList => "vendor:list",
        }
    }

    pub(crate) fn title(self) -> &'static str {
        match self {
            DebugCommand::Dashboard => "Dashboard",
            DebugCommand::RouteList => "Routes",
            DebugCommand::RouteCompare => "Route Compare",
            DebugCommand::RouteSources => "Route Sources",
            DebugCommand::MiddlewareList => "Middleware",
            DebugCommand::ConfigList => "Config",
            DebugCommand::ConfigSources => "Config Sources",
            DebugCommand::ControllerList => "Controllers",
            DebugCommand::ProviderList => "Providers",
            DebugCommand::ViewList => "Views",
            DebugCommand::ModelList => "Models",
            DebugCommand::MigrationList => "Migrations",
            DebugCommand::PublicList => "Public Files",
            DebugCommand::VendorList => "Vendor Classes",
        }
    }

    pub(crate) fn parse(value: &str) -> Option<Self> {
        match value {
            "dashboard" => Some(Self::Dashboard),
            "route:list" => Some(Self::RouteList),
            "route:compare" => Some(Self::RouteCompare),
            "route:sources" => Some(Self::RouteSources),
            "middleware:list" => Some(Self::MiddlewareList),
            "config:list" => Some(Self::ConfigList),
            "config:sources" => Some(Self::ConfigSources),
            "controller:list" => Some(Self::ControllerList),
            "provider:list" => Some(Self::ProviderList),
            "view:list" => Some(Self::ViewList),
            "model:list" => Some(Self::ModelList),
            "migration:list" => Some(Self::MigrationList),
            "public:list" => Some(Self::PublicList),
            "vendor:list" => Some(Self::VendorList),
            _ => None,
        }
    }
}

pub(crate) fn resolve_initial_project(
    initial_project: Option<&str>,
    projects: &[LaravelProject],
) -> Result<usize, String> {
    let Some(value) = initial_project else {
        return Ok(0);
    };

    let resolved = crate::project::resolve(Some(value))?;
    Ok(projects
        .iter()
        .position(|candidate| candidate.root == resolved.root)
        .unwrap_or(0))
}
