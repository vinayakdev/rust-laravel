use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use crate::php::ast::strip_root;
use crate::types::EnvItem;

pub fn load_env_map_with<F>(
    project_root: &Path,
    mut override_loader: F,
) -> Result<BTreeMap<String, String>, String>
where
    F: FnMut(&Path) -> Option<String>,
{
    for name in [".env", ".env.example"] {
        let path = project_root.join(name);
        if let Some(text) = load_env_text(&path, &mut override_loader)? {
            let raw = parse_env_pairs(&text);
            return Ok(resolve_env_pairs(&raw));
        }
    }

    Ok(BTreeMap::new())
}

pub fn load_env_entries_with<F>(
    project_root: &Path,
    mut override_loader: F,
) -> Result<Vec<EnvItem>, String>
where
    F: FnMut(&Path) -> Option<String>,
{
    let mut entries = Vec::new();

    for name in [".env", ".env.example"] {
        let path = project_root.join(name);
        let Some(text) = load_env_text(&path, &mut override_loader)? else {
            continue;
        };

        let raw = parse_env_pairs(&text);
        let resolved = resolve_env_pairs(&raw);

        for (index, raw_line) in text.lines().enumerate() {
            let line = raw_line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            let Some((key, _)) = line.split_once('=') else {
                continue;
            };
            let key = key.trim();
            if key.is_empty() {
                continue;
            }

            entries.push(EnvItem {
                file: strip_root(project_root, &path),
                line: index + 1,
                column: raw_line.find(key).map(|value| value + 1).unwrap_or(1),
                key: key.to_string(),
                value: resolved.get(key).cloned().unwrap_or_default(),
            });
        }
    }

    entries.sort_by(|l, r| {
        l.key
            .cmp(&r.key)
            .then(l.file.cmp(&r.file))
            .then(l.line.cmp(&r.line))
            .then(l.column.cmp(&r.column))
    });

    Ok(entries)
}

fn load_env_text<F>(path: &Path, override_loader: &mut F) -> Result<Option<String>, String>
where
    F: FnMut(&Path) -> Option<String>,
{
    if let Some(text) = override_loader(path) {
        return Ok(Some(text));
    }

    if path.is_file() {
        return fs::read_to_string(path)
            .map(Some)
            .map_err(|e| format!("failed to read {}: {e}", path.display()));
    }

    Ok(None)
}

fn parse_env_pairs(text: &str) -> BTreeMap<String, String> {
    let mut env = BTreeMap::new();
    for raw_line in text.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some((key, value)) = line.split_once('=') {
            env.insert(
                key.trim().to_string(),
                strip_quotes(value.trim()).to_string(),
            );
        }
    }
    env
}

fn resolve_env_pairs(raw: &BTreeMap<String, String>) -> BTreeMap<String, String> {
    let mut resolved = BTreeMap::new();
    for key in raw.keys() {
        let value = resolve_env_value(key, raw, &mut Vec::new());
        resolved.insert(key.clone(), value);
    }
    resolved
}

fn resolve_env_value(key: &str, raw: &BTreeMap<String, String>, stack: &mut Vec<String>) -> String {
    if stack.iter().any(|item| item == key) {
        return raw.get(key).cloned().unwrap_or_default();
    }
    let Some(value) = raw.get(key) else {
        return String::new();
    };
    stack.push(key.to_string());
    let expanded = expand_env_placeholders(value, raw, stack);
    stack.pop();
    expanded
}

fn expand_env_placeholders(
    value: &str,
    raw: &BTreeMap<String, String>,
    stack: &mut Vec<String>,
) -> String {
    let mut output = String::new();
    let chars: Vec<char> = value.chars().collect();
    let mut index = 0;

    while index < chars.len() {
        if chars[index] == '$' && chars.get(index + 1) == Some(&'{') {
            let mut end = index + 2;
            while end < chars.len() && chars[end] != '}' {
                end += 1;
            }
            if end < chars.len() {
                let key: String = chars[index + 2..end].iter().collect();
                output.push_str(&resolve_env_value(&key, raw, stack));
                index = end + 1;
                continue;
            }
        }
        output.push(chars[index]);
        index += 1;
    }
    output
}

fn strip_quotes(value: &str) -> &str {
    value
        .strip_prefix('"')
        .and_then(|v| v.strip_suffix('"'))
        .or_else(|| value.strip_prefix('\'').and_then(|v| v.strip_suffix('\'')))
        .unwrap_or(value)
}
