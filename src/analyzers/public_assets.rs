use crate::project::LaravelProject;
use crate::types::PublicAssetReport;

pub fn analyze(project: &LaravelProject) -> Result<PublicAssetReport, String> {
    rust_php_public::analyze(project)
}
