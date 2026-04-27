use rust_php_migrations::types::ColumnEntry;
use serde::Serialize;
use std::collections::BTreeMap;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize)]
pub struct RelationEntry {
    pub method: String,
    pub relation_type: String,
    pub related_model: String,
    pub related_model_file: Option<PathBuf>,
    pub foreign_key: Option<String>,
    pub local_key: Option<String>,
    pub pivot_table: Option<String>,
    pub line: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct ModelEntry {
    pub file: PathBuf,
    pub line: usize,
    pub class_name: String,
    pub namespace: String,
    pub table: String,
    pub table_inferred: bool,
    pub primary_key: String,
    pub key_type: String,
    pub incrementing: bool,
    pub timestamps: bool,
    pub soft_deletes: bool,
    pub connection: Option<String>,
    pub fillable: Vec<String>,
    pub guarded: Vec<String>,
    pub hidden: Vec<String>,
    pub casts: BTreeMap<String, String>,
    pub appends: Vec<String>,
    pub with: Vec<String>,
    pub traits: Vec<String>,
    pub relations: Vec<RelationEntry>,
    pub scopes: Vec<String>,
    pub accessors: Vec<String>,
    pub mutators: Vec<String>,
    pub methods: Vec<String>,
    pub columns: Vec<ColumnEntry>,
}

#[derive(Debug, Serialize)]
pub struct ModelReport {
    pub project_name: String,
    pub project_root: PathBuf,
    pub model_count: usize,
    pub models: Vec<ModelEntry>,
}
