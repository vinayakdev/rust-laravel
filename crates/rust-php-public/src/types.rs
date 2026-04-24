use serde::Serialize;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize)]
pub struct PublicAssetUsage {
    pub helper: String,
    pub source_kind: String,
    pub file: PathBuf,
    pub line: usize,
    pub column: usize,
    pub raw_reference: String,
    pub asset_path: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct PublicAssetEntry {
    pub file: PathBuf,
    pub asset_path: String,
    pub extension: Option<String>,
    pub size_bytes: u64,
    pub usages: Vec<PublicAssetUsage>,
}

#[derive(Debug, Serialize)]
pub struct PublicAssetReport {
    pub project_name: String,
    pub project_root: PathBuf,
    pub file_count: usize,
    pub usage_count: usize,
    pub assets: Vec<PublicAssetEntry>,
}
