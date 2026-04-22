mod analyzers;
mod browser;
mod command;
mod reports;
mod web;

pub use browser::run as run_browse;
pub use rust_php_foundation::project;
pub use web::run as run_web;

mod output {
    pub use rust_php_output::text;
}

mod types {
    pub use rust_php_routes::types::RouteEntry;
}
