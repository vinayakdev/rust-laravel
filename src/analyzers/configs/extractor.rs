use std::collections::BTreeMap;
use std::path::Path;

use crate::php::ast::strip_root;
use crate::types::{ConfigItem, ConfigSource};

pub(crate) fn find_config_items(
    root: &Path,
    file: &Path,
    source: &str,
    namespace: &str,
    env: &BTreeMap<String, String>,
    item_source: &ConfigSource,
) -> Vec<ConfigItem> {
    let mut stack: Vec<String> = Vec::new();
    let mut items = Vec::new();

    for (index, raw_line) in source.lines().enumerate() {
        let trimmed = raw_line.trim();
        if trimmed.is_empty() || trimmed.starts_with("//") || trimmed.starts_with('#') {
            continue;
        }

        let close_count = trimmed.chars().filter(|&c| c == ']').count();
        if let Some(parsed) = parse_config_assignment(raw_line) {
            let full_key = if stack.is_empty() {
                format!("{namespace}.{}", parsed.key)
            } else {
                format!("{namespace}.{}.{}", stack.join("."), parsed.key)
            };

            let env_value = parsed.env_key.as_ref().and_then(|k| env.get(k).cloned());

            items.push(ConfigItem {
                file: strip_root(root, file),
                line: index + 1,
                column: parsed.column,
                key: full_key,
                env_key: parsed.env_key.clone(),
                default_value: parsed.default_value.clone(),
                env_value,
                source: item_source.clone(),
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
        .map(|(k, d)| (Some(k), d))
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
    let default_value = args.get(1).and_then(|p| parse_literal_value(p.trim()));
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
    if let Some(s) = parse_quoted_string(value) {
        return Some(s);
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

fn pop_many(stack: &mut Vec<String>, count: usize) {
    for _ in 0..count {
        if stack.pop().is_none() {
            break;
        }
    }
}
