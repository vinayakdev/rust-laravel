use crate::project::LaravelProject;
use crate::types::{ConfigReference, ConfigReport};
use std::fs;
use std::path::{Path, PathBuf};

pub fn analyze(project: &LaravelProject) -> Result<ConfigReport, String> {
    let mut files = Vec::new();
    collect_php_files(&project.root, &mut files);

    let mut references = Vec::new();
    for file in files {
        let source = fs::read(&file)
            .map_err(|error| format!("failed to read {}: {error}", file.display()))?;
        references.extend(find_config_references(&project.root, &file, &source));
    }

    references.sort_by(|left, right| {
        left.file
            .cmp(&right.file)
            .then(left.line.cmp(&right.line))
            .then(left.key.cmp(&right.key))
    });

    Ok(ConfigReport {
        project_name: project.name.clone(),
        project_root: project.root.clone(),
        reference_count: references.len(),
        references,
    })
}

fn find_config_references(root: &Path, file: &Path, source: &[u8]) -> Vec<ConfigReference> {
    let text = String::from_utf8_lossy(source);
    let mut references = Vec::new();
    let mut search_start = 0;

    while let Some(relative_index) = text[search_start..].find("config(") {
        let index = search_start + relative_index;
        let line = 1 + text[..index].bytes().filter(|byte| *byte == b'\n').count();
        let key = parse_config_key(&text[index..]).unwrap_or_else(|| "<dynamic>".to_string());
        references.push(ConfigReference {
            file: strip_root(root, file),
            line,
            key,
        });
        search_start = index + "config(".len();
    }

    references
}

fn parse_config_key(text: &str) -> Option<String> {
    let quote = text.chars().nth("config(".len())?;
    if quote != '\'' && quote != '"' {
        return None;
    }

    let mut key = String::new();
    let mut escaped = false;
    for ch in text["config(".len() + 1..].chars() {
        if escaped {
            key.push(ch);
            escaped = false;
            continue;
        }
        if ch == '\\' {
            escaped = true;
            continue;
        }
        if ch == quote {
            return Some(key);
        }
        key.push(ch);
    }

    None
}

fn collect_php_files(dir: &Path, files: &mut Vec<PathBuf>) {
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                if path.file_name().and_then(|name| name.to_str()) == Some("vendor") {
                    continue;
                }
                collect_php_files(&path, files);
            } else if path.extension().and_then(|ext| ext.to_str()) == Some("php") {
                files.push(path);
            }
        }
    }
}

fn strip_root(root: &Path, file: &Path) -> PathBuf {
    file.strip_prefix(root).unwrap_or(file).to_path_buf()
}
