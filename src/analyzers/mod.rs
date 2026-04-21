pub mod configs;
pub mod controllers;
pub mod middleware;
pub mod migrations;
pub mod models;
pub mod providers;
pub mod routes;
pub mod views;

use crate::project::LaravelProject;

#[allow(dead_code)]
/// Every analyzer must implement this trait.
/// `Report` is the typed output produced by analyzing a Laravel project.
pub trait Analyzer {
    type Report;
    fn analyze(project: &LaravelProject) -> Result<Self::Report, String>;
}
