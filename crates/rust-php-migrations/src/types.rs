use serde::Serialize;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize)]
pub struct ColumnEntry {
    pub name: String,
    pub column_type: String,
    pub nullable: bool,
    pub default: Option<String>,
    pub unique: bool,
    pub unsigned: bool,
    pub primary: bool,
    pub enum_values: Vec<String>,
    pub comment: Option<String>,
    pub references: Option<String>,
    pub on_table: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct IndexEntry {
    pub columns: Vec<String>,
    pub index_type: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct MigrationEntry {
    pub file: PathBuf,
    pub timestamp: String,
    pub class_name: String,
    pub table: String,
    pub operation: String,
    pub columns: Vec<ColumnEntry>,
    pub indexes: Vec<IndexEntry>,
    pub dropped_columns: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct MigrationReport {
    pub project_name: String,
    pub project_root: PathBuf,
    pub migration_count: usize,
    pub migrations: Vec<MigrationEntry>,
}
