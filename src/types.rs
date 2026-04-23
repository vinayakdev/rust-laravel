pub use rust_php_configs::types::{ConfigItem, ConfigReport, ConfigSource};
pub use rust_php_controllers::types::{
    ControllerEntry, ControllerMethodEntry, ControllerReport, ControllerVariableEntry,
    RouteControllerTarget,
};
pub use rust_php_foundation::types::{EnvItem, ProviderEntry, ProviderReport};
pub use rust_php_middleware::types::{
    MiddlewareAlias, MiddlewareGroup, MiddlewareReport, MiddlewareSource, RoutePattern,
};
pub use rust_php_migrations::types::{ColumnEntry, IndexEntry, MigrationEntry, MigrationReport};
pub use rust_php_models::types::{ModelEntry, ModelReport, RelationEntry};
pub use rust_php_public::types::{PublicAssetEntry, PublicAssetReport, PublicAssetUsage};
pub use rust_php_routes::types::{RouteEntry, RouteRegistration, RouteReport};
pub use rust_php_views::types::{
    BladeComponentEntry, LivewireComponentEntry, MissingViewEntry, ViewEntry, ViewReport,
    ViewSource, ViewUsage, ViewVariable,
};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum OutputMode {
    Text,
    Json,
}
