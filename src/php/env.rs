use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

// Loads .env or .env.example from the project root. Returns an empty map if
// neither file exists rather than erroring — missing env is normal in CI.
pub fn load_env_map(project_root: &Path) -> Result<BTreeMap<String, String>, String> {
    for name in [".env", ".env.example"] {
        let path = project_root.join(name);
        if path.is_file() {
            let text = fs::read_to_string(&path)
                .map_err(|e| format!("failed to read {}: {e}", path.display()))?;
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

fn resolve_env_value(
    key: &str,
    raw: &BTreeMap<String, String>,
    stack: &mut Vec<String>,
) -> String {
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
        .or_else(|| {
            value
                .strip_prefix('\'')
                .and_then(|v| v.strip_suffix('\''))
        })
        .unwrap_or(value)
}
