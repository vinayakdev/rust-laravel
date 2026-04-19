use crate::project::LaravelProject;
use crate::types::{ConfigItem, ConfigReport};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

pub fn analyze(project: &LaravelProject) -> Result<ConfigReport, String> {
    let env = load_env_map(project)?;
    let mut items = Vec::new();
    let config_dir = project.root.join("config");
    let entries = fs::read_dir(&config_dir)
        .map_err(|error| format!("failed to read {}: {error}", config_dir.display()))?;

    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("php") {
            continue;
        }

        let source = fs::read_to_string(&path)
            .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
        let namespace = path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .unwrap_or("config")
            .to_string();

        items.extend(find_config_items(
            &project.root,
            &path,
            &source,
            &namespace,
            &env,
        ));
    }

    items.sort_by(|left, right| {
        left.file
            .cmp(&right.file)
            .then(left.line.cmp(&right.line))
            .then(left.column.cmp(&right.column))
            .then(left.key.cmp(&right.key))
    });

    Ok(ConfigReport {
        project_name: project.name.clone(),
        project_root: project.root.clone(),
        item_count: items.len(),
        items,
    })
}

fn load_env_map(project: &LaravelProject) -> Result<BTreeMap<String, String>, String> {
    for name in [".env", ".env.example"] {
        let path = project.root.join(name);
        if path.is_file() {
            let text = fs::read_to_string(&path)
                .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
            let raw = parse_env_pairs(&text);
            return Ok(resolve_env_pairs(&raw));
        }
    }

    Ok(BTreeMap::new())
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

fn find_config_items(
    root: &Path,
    file: &Path,
    source: &str,
    namespace: &str,
    env: &BTreeMap<String, String>,
) -> Vec<ConfigItem> {
    let mut stack: Vec<String> = Vec::new();
    let mut items = Vec::new();

    for (index, raw_line) in source.lines().enumerate() {
        let trimmed = raw_line.trim();
        if trimmed.is_empty() || trimmed.starts_with("//") || trimmed.starts_with('#') {
            continue;
        }

        let close_count = trimmed.chars().filter(|ch| *ch == ']').count();
        if let Some(parsed) = parse_config_assignment(raw_line) {
            let full_key = if stack.is_empty() {
                format!("{namespace}.{}", parsed.key)
            } else {
                format!("{namespace}.{}.{}", stack.join("."), parsed.key)
            };

            let env_value = parsed
                .env_key
                .as_ref()
                .and_then(|env_key| env.get(env_key).cloned());

            items.push(ConfigItem {
                file: strip_root(root, file),
                line: index + 1,
                column: parsed.column,
                key: full_key,
                env_key: parsed.env_key.clone(),
                default_value: parsed.default_value.clone(),
                env_value,
            });

            if parsed.opens_array {
                stack.push(parsed.key);
            }
        } else {
            pop_many(&mut stack, close_count);
        }
    }

    items
}

#[derive(Clone)]
struct ParsedConfigLine {
    key: String,
    column: usize,
    opens_array: bool,
    env_key: Option<String>,
    default_value: Option<String>,
}

fn parse_config_assignment(line: &str) -> Option<ParsedConfigLine> {
    let trimmed = line.trim_start();
    let leading = line.len() - trimmed.len();
    let quote = trimmed.chars().next()?;
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

    let value = rest[2..].trim_start().trim_end_matches(',');
    let opens_array = value.starts_with('[');
    let (env_key, default_value) = parse_env_call(value)
        .map(|(env_key, default_value)| (Some(env_key), default_value))
        .unwrap_or_else(|| (None, parse_literal_value(value)));

    Some(ParsedConfigLine {
        key,
        column: leading + 1,
        opens_array,
        env_key,
        default_value,
    })
}

fn parse_env_call(value: &str) -> Option<(String, Option<String>)> {
    let env_index = value.find("env(")?;
    let args = extract_call_args(&value[env_index + 4..])?;
    let env_key = parse_quoted_string(args.first()?.trim())?;
    let default_value = args
        .get(1)
        .and_then(|part| parse_literal_value(part.trim()));
    Some((env_key, default_value))
}

fn extract_call_args(text: &str) -> Option<Vec<String>> {
    let mut depth = 1usize;
    let mut current = String::new();
    let mut args = Vec::new();
    let mut in_single = false;
    let mut in_double = false;
    let mut escape = false;

    for ch in text.chars() {
        if in_single {
            current.push(ch);
            if escape {
                escape = false;
            } else if ch == '\\' {
                escape = true;
            } else if ch == '\'' {
                in_single = false;
            }
            continue;
        }

        if in_double {
            current.push(ch);
            if escape {
                escape = false;
            } else if ch == '\\' {
                escape = true;
            } else if ch == '"' {
                in_double = false;
            }
            continue;
        }

        match ch {
            '\'' => {
                in_single = true;
                current.push(ch);
            }
            '"' => {
                in_double = true;
                current.push(ch);
            }
            '(' => {
                depth += 1;
                current.push(ch);
            }
            ')' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    args.push(current.trim().to_string());
                    return Some(args);
                }
                current.push(ch);
            }
            ',' if depth == 1 => {
                args.push(current.trim().to_string());
                current.clear();
            }
            _ => current.push(ch),
        }
    }

    None
}

fn parse_literal_value(value: &str) -> Option<String> {
    let value = value.trim();
    if value.is_empty() {
        return None;
    }

    if let Some(string) = parse_quoted_string(value) {
        return Some(string);
    }

    if matches!(value, "true" | "false" | "null") || value.parse::<i64>().is_ok() {
        return Some(value.to_string());
    }

    if value.starts_with('[') {
        return Some("[...]".to_string());
    }

    None
}

fn parse_quoted_string(value: &str) -> Option<String> {
    let mut chars = value.chars();
    let quote = chars.next()?;
    if quote != '\'' && quote != '"' {
        return None;
    }

    let mut output = String::new();
    let mut escaped = false;
    for ch in value[1..].chars() {
        if escaped {
            output.push(ch);
            escaped = false;
            continue;
        }
        if ch == '\\' {
            escaped = true;
            continue;
        }
        if ch == quote {
            return Some(output);
        }
        output.push(ch);
    }

    None
}

fn strip_quotes(value: &str) -> &str {
    if value.len() >= 2 {
        let bytes = value.as_bytes();
        if (bytes[0] == b'\'' && bytes[value.len() - 1] == b'\'')
            || (bytes[0] == b'"' && bytes[value.len() - 1] == b'"')
        {
            return &value[1..value.len() - 1];
        }
    }
    value
}

fn pop_many(stack: &mut Vec<String>, count: usize) {
    for _ in 0..count {
        if stack.pop().is_none() {
            break;
        }
    }
}

fn strip_root(root: &Path, file: &Path) -> PathBuf {
    file.strip_prefix(root).unwrap_or(file).to_path_buf()
}
