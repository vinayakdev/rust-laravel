use crate::project::LaravelProject;
use crate::types::{ConfigReference, ConfigReport};
use std::fs;
use std::path::{Path, PathBuf};

pub fn analyze(project: &LaravelProject) -> Result<ConfigReport, String> {
    let mut references = Vec::new();

    collect_config_definitions(project, &mut references)?;
    collect_env_files(project, &mut references)?;

    let mut php_files = Vec::new();
    collect_php_files(&project.root, &mut php_files);
    for file in php_files {
        let source = fs::read(&file)
            .map_err(|error| format!("failed to read {}: {error}", file.display()))?;
        references.extend(find_function_references(
            &project.root,
            &file,
            &source,
            "config",
            "usage",
        ));
        references.extend(find_function_references(
            &project.root,
            &file,
            &source,
            "env",
            "env-usage",
        ));
    }

    references.sort_by(|left, right| {
        left.file
            .cmp(&right.file)
            .then(left.line.cmp(&right.line))
            .then(left.kind.cmp(&right.kind))
            .then(left.key.cmp(&right.key))
    });

    Ok(ConfigReport {
        project_name: project.name.clone(),
        project_root: project.root.clone(),
        reference_count: references.len(),
        references,
    })
}

fn collect_config_definitions(
    project: &LaravelProject,
    references: &mut Vec<ConfigReference>,
) -> Result<(), String> {
    let config_dir = project.root.join("config");
    let entries = fs::read_dir(&config_dir)
        .map_err(|error| format!("failed to read {}: {error}", config_dir.display()))?;

    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("php") {
            continue;
        }

        let source = fs::read(&path)
            .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
        let namespace = path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .unwrap_or("config")
            .to_string();

        references.extend(find_config_definitions(
            &project.root,
            &path,
            &source,
            &namespace,
        ));
    }

    Ok(())
}

fn collect_env_files(
    project: &LaravelProject,
    references: &mut Vec<ConfigReference>,
) -> Result<(), String> {
    for name in [".env", ".env.example"] {
        let path = project.root.join(name);
        if !path.is_file() {
            continue;
        }

        let text = fs::read_to_string(&path)
            .map_err(|error| format!("failed to read {}: {error}", path.display()))?;

        for (index, line) in text.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }
            if let Some((key, _)) = trimmed.split_once('=') {
                let column = line.find(key).map_or(1, |offset| offset + 1);
                references.push(ConfigReference {
                    file: strip_root(&project.root, &path),
                    line: index + 1,
                    column,
                    kind: "env-file".to_string(),
                    key: key.trim().to_string(),
                });
            }
        }
    }

    Ok(())
}

fn find_config_definitions(
    root: &Path,
    file: &Path,
    source: &[u8],
    namespace: &str,
) -> Vec<ConfigReference> {
    let text = String::from_utf8_lossy(source);
    let mut stack: Vec<String> = Vec::new();
    let mut references = Vec::new();

    for (index, raw_line) in text.lines().enumerate() {
        let trimmed = raw_line.trim();
        if trimmed.is_empty() || trimmed.starts_with("//") || trimmed.starts_with('#') {
            continue;
        }

        let close_count = trimmed.chars().filter(|ch| *ch == ']').count();
        if let Some((key, opens_array, column)) = parse_config_assignment(raw_line) {
            let full_key = if stack.is_empty() {
                format!("{namespace}.{key}")
            } else {
                format!("{namespace}.{}.{}", stack.join("."), key)
            };

            references.push(ConfigReference {
                file: strip_root(root, file),
                line: index + 1,
                column,
                kind: "definition".to_string(),
                key: full_key,
            });

            if opens_array {
                stack.push(key);
            }
        } else {
            pop_many(&mut stack, close_count);
        }
    }

    references
}

fn parse_config_assignment(line: &str) -> Option<(String, bool, usize)> {
    let trimmed = line.trim_start();
    let leading = line.len() - trimmed.len();
    let mut chars = trimmed.chars();
    let quote = chars.next()?;
    if quote != '\'' && quote != '"' {
        return None;
    }

    let mut key = String::new();
    let mut escaped = false;
    let mut rest_start = None;

    for (index, ch) in trimmed[1..].char_indices() {
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
            rest_start = Some(index + 2);
            break;
        }
        key.push(ch);
    }

    let rest = trimmed.get(rest_start?..)?.trim_start();
    if !rest.starts_with("=>") {
        return None;
    }

    let value = rest[2..].trim_start();
    Some((key, value.starts_with('['), leading + 1))
}

fn find_function_references(
    root: &Path,
    file: &Path,
    source: &[u8],
    function_name: &str,
    kind: &str,
) -> Vec<ConfigReference> {
    let text = String::from_utf8_lossy(source);
    let needle = format!("{function_name}(");
    let mut references = Vec::new();
    let mut search_start = 0;

    while let Some(relative_index) = text[search_start..].find(&needle) {
        let index = search_start + relative_index;
        let line = 1 + text[..index].bytes().filter(|byte| *byte == b'\n').count();
        let line_start = text[..index].rfind('\n').map_or(0, |offset| offset + 1);
        let column = index - line_start + 1;
        let key = parse_first_string_arg(&text[index + needle.len()..])
            .unwrap_or_else(|| "<dynamic>".to_string());
        references.push(ConfigReference {
            file: strip_root(root, file),
            line,
            column,
            kind: kind.to_string(),
            key,
        });
        search_start = index + needle.len();
    }

    references
}

fn parse_first_string_arg(text: &str) -> Option<String> {
    let mut chars = text.chars();
    let quote = chars.next()?;
    if quote != '\'' && quote != '"' {
        return None;
    }

    let mut value = String::new();
    let mut escaped = false;
    for ch in text[1..].chars() {
        if escaped {
            value.push(ch);
            escaped = false;
            continue;
        }
        if ch == '\\' {
            escaped = true;
            continue;
        }
        if ch == quote {
            return Some(value);
        }
        value.push(ch);
    }

    None
}

fn pop_many(stack: &mut Vec<String>, count: usize) {
    for _ in 0..count {
        if stack.pop().is_none() {
            break;
        }
    }
}

fn collect_php_files(dir: &Path, files: &mut Vec<PathBuf>) {
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                let skip = path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| {
                        matches!(name, "vendor" | "node_modules" | "target" | ".git")
                    });
                if !skip {
                    collect_php_files(&path, files);
                }
            } else if path.extension().and_then(|ext| ext.to_str()) == Some("php") {
                files.push(path);
            }
        }
    }
}

fn strip_root(root: &Path, file: &Path) -> PathBuf {
    file.strip_prefix(root).unwrap_or(file).to_path_buf()
}
