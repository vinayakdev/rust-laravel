use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

pub fn load_classmap(project_root: &Path) -> HashMap<String, PathBuf> {
    let vendor_dir = project_root.join("vendor");
    let base_dir = project_root;
    let classmap_path = vendor_dir.join("composer/autoload_classmap.php");

    let Ok(content) = fs::read_to_string(&classmap_path) else {
        return HashMap::new();
    };

    let vendor_str = vendor_dir.display().to_string();
    let base_str = base_dir.display().to_string();
    let mut map = HashMap::new();

    for line in content.lines() {
        let line = line.trim();
        let Some(fqn) = extract_quoted(line) else {
            continue;
        };
        let full_path = if line.contains("$vendorDir") {
            extract_path(line, "$vendorDir . '").map(|p| format!("{vendor_str}{p}"))
        } else if line.contains("$baseDir") {
            extract_path(line, "$baseDir . '").map(|p| format!("{base_str}{p}"))
        } else {
            None
        };
        if let Some(path) = full_path {
            map.insert(fqn, PathBuf::from(path));
        }
    }

    map
}

pub fn collect_chainable_methods(fqn: &str, classmap: &HashMap<String, PathBuf>) -> Vec<String> {
    let mut visited = HashSet::new();
    let mut methods = Vec::new();
    collect_methods_recursive(fqn, classmap, &mut visited, &mut methods);
    methods.dedup();
    methods
}

pub fn collect_chainable_methods_with_source(
    fqn: &str,
    classmap: &HashMap<String, PathBuf>,
) -> Vec<(String, String)> {
    let mut visited = HashSet::new();
    let mut methods = Vec::new();
    collect_methods_with_source_recursive(fqn, classmap, &mut visited, &mut methods);
    methods
}

fn collect_methods_with_source_recursive(
    class_fqn: &str,
    classmap: &HashMap<String, PathBuf>,
    visited: &mut HashSet<String>,
    methods: &mut Vec<(String, String)>,
) {
    if !visited.insert(class_fqn.to_string()) {
        return;
    }
    let Some(path) = classmap.get(class_fqn) else {
        return;
    };
    let Ok(source) = fs::read_to_string(path) else {
        return;
    };

    let namespace = parse_namespace(&source);
    let use_map = parse_use_statements(&source);
    let short = class_fqn.split('\\').last().unwrap_or(class_fqn).to_string();

    for name in parse_chainable_methods(&source) {
        methods.push((name, short.clone()));
    }

    for trait_short in parse_used_traits(&source) {
        let trait_fqn = resolve_fqn(&trait_short, &namespace, &use_map);
        collect_methods_with_source_recursive(&trait_fqn, classmap, visited, methods);
    }

    if let Some(parent_short) = parse_extends(&source) {
        let parent_fqn = resolve_fqn(&parent_short, &namespace, &use_map);
        collect_methods_with_source_recursive(&parent_fqn, classmap, visited, methods);
    }
}

fn collect_methods_recursive(
    class_fqn: &str,
    classmap: &HashMap<String, PathBuf>,
    visited: &mut HashSet<String>,
    methods: &mut Vec<String>,
) {
    if !visited.insert(class_fqn.to_string()) {
        return;
    }
    let Some(path) = classmap.get(class_fqn) else {
        return;
    };
    let Ok(source) = fs::read_to_string(path) else {
        return;
    };

    let namespace = parse_namespace(&source);
    let use_map = parse_use_statements(&source);

    for name in parse_chainable_methods(&source) {
        methods.push(name);
    }

    for trait_short in parse_used_traits(&source) {
        let fqn = resolve_fqn(&trait_short, &namespace, &use_map);
        collect_methods_recursive(&fqn, classmap, visited, methods);
    }

    if let Some(parent_short) = parse_extends(&source) {
        let fqn = resolve_fqn(&parent_short, &namespace, &use_map);
        collect_methods_recursive(&fqn, classmap, visited, methods);
    }
}

fn parse_namespace(source: &str) -> String {
    for line in source.lines() {
        let t = line.trim();
        if t.starts_with("namespace ") && t.ends_with(';') {
            return t[10..t.len() - 1].to_string();
        }
    }
    String::new()
}

fn parse_use_statements(source: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for line in source.lines() {
        let t = line.trim();
        if t.starts_with("class ")
            || t.starts_with("abstract class ")
            || t.starts_with("trait ")
            || t.starts_with("interface ")
        {
            break;
        }
        if t.starts_with("use ") && t.ends_with(';') && !t.contains('{') {
            let inner = &t[4..t.len() - 1];
            if let Some((fqn, alias)) = inner.split_once(" as ") {
                map.insert(alias.trim().to_string(), fqn.trim().to_string());
            } else {
                let fqn = inner.trim().to_string();
                let short = fqn.split('\\').last().unwrap_or(&fqn).to_string();
                map.insert(short, fqn);
            }
        }
    }
    map
}

fn parse_used_traits(source: &str) -> Vec<String> {
    let mut traits = Vec::new();
    let mut in_body = false;
    let mut depth = 0i32;

    for line in source.lines() {
        let t = line.trim();
        if !in_body {
            if t.contains('{')
                && (t.starts_with("class ")
                    || t.starts_with("abstract class ")
                    || t.starts_with("trait ")
                    || t.starts_with("interface "))
            {
                in_body = true;
                depth = 1;
                continue;
            }
            if t == "{" {
                in_body = true;
                depth = 1;
                continue;
            }
            continue;
        }
        for ch in t.chars() {
            match ch {
                '{' => depth += 1,
                '}' => depth -= 1,
                _ => {}
            }
        }
        if depth == 1 && t.starts_with("use ") && t.ends_with(';') && !t.contains('(') {
            let inner = &t[4..t.len() - 1];
            for part in inner.split(',') {
                let name = part.trim().split('{').next().unwrap_or(part).trim();
                if !name.is_empty() {
                    traits.push(name.to_string());
                }
            }
        }
    }
    traits
}

fn parse_extends(source: &str) -> Option<String> {
    for line in source.lines() {
        let t = line.trim();
        if (t.starts_with("class ") || t.starts_with("abstract class ")) && t.contains("extends") {
            let after = t.split("extends").nth(1)?;
            let parent = after
                .split_whitespace()
                .next()?
                .trim_matches(|c| c == '{' || c == ',');
            return Some(parent.to_string());
        }
    }
    None
}

fn parse_chainable_methods(source: &str) -> Vec<String> {
    let mut methods = Vec::new();
    for line in source.lines() {
        let t = line.trim();
        if t.starts_with("public function ")
            && (t.contains("): static") || t.contains("): self"))
            && !t.contains("abstract ")
        {
            let after = &t["public function ".len()..];
            if let Some(name) = after.split('(').next() {
                let name = name.trim();
                if !name.is_empty() {
                    methods.push(name.to_string());
                }
            }
        }
    }
    methods
}

pub fn collect_class_properties(fqn: &str, classmap: &HashMap<String, PathBuf>) -> Vec<String> {
    let mut visited = HashSet::new();
    let mut properties = Vec::new();
    collect_properties_recursive(fqn, classmap, &mut visited, &mut properties);
    properties.dedup();
    properties
}

pub fn collect_class_properties_with_source(
    fqn: &str,
    classmap: &HashMap<String, PathBuf>,
) -> Vec<(String, String)> {
    let mut visited = HashSet::new();
    let mut properties = Vec::new();
    collect_properties_with_source_recursive(fqn, classmap, &mut visited, &mut properties);
    properties
}

fn collect_properties_with_source_recursive(
    class_fqn: &str,
    classmap: &HashMap<String, PathBuf>,
    visited: &mut HashSet<String>,
    properties: &mut Vec<(String, String)>,
) {
    if !visited.insert(class_fqn.to_string()) {
        return;
    }
    let Some(path) = classmap.get(class_fqn) else { return; };
    let Ok(source) = fs::read_to_string(path) else { return; };

    let namespace = parse_namespace(&source);
    let use_map = parse_use_statements(&source);
    let short = class_fqn.split('\\').last().unwrap_or(class_fqn).to_string();

    for name in parse_class_properties(&source) {
        properties.push((name, short.clone()));
    }
    for trait_short in parse_used_traits(&source) {
        let trait_fqn = resolve_fqn(&trait_short, &namespace, &use_map);
        collect_properties_with_source_recursive(&trait_fqn, classmap, visited, properties);
    }
    if let Some(parent_short) = parse_extends(&source) {
        let parent_fqn = resolve_fqn(&parent_short, &namespace, &use_map);
        collect_properties_with_source_recursive(&parent_fqn, classmap, visited, properties);
    }
}

fn collect_properties_recursive(
    class_fqn: &str,
    classmap: &HashMap<String, PathBuf>,
    visited: &mut HashSet<String>,
    properties: &mut Vec<String>,
) {
    if !visited.insert(class_fqn.to_string()) {
        return;
    }
    let Some(path) = classmap.get(class_fqn) else { return; };
    let Ok(source) = fs::read_to_string(path) else { return; };

    let namespace = parse_namespace(&source);
    let use_map = parse_use_statements(&source);

    for name in parse_class_properties(&source) {
        properties.push(name);
    }
    for trait_short in parse_used_traits(&source) {
        let fqn = resolve_fqn(&trait_short, &namespace, &use_map);
        collect_properties_recursive(&fqn, classmap, visited, properties);
    }
    if let Some(parent_short) = parse_extends(&source) {
        let fqn = resolve_fqn(&parent_short, &namespace, &use_map);
        collect_properties_recursive(&fqn, classmap, visited, properties);
    }
}

fn parse_class_properties(source: &str) -> Vec<String> {
    let mut properties = Vec::new();
    let mut in_body = false;
    let mut depth = 0i32;

    for line in source.lines() {
        let t = line.trim();
        if !in_body {
            if t.contains('{')
                && (t.starts_with("class ")
                    || t.starts_with("abstract class ")
                    || t.starts_with("trait "))
            {
                in_body = true;
                depth = 1;
                continue;
            }
            if t == "{" {
                in_body = true;
                depth = 1;
                continue;
            }
            continue;
        }
        for ch in t.chars() {
            match ch {
                '{' => depth += 1,
                '}' => depth -= 1,
                _ => {}
            }
        }
        if depth == 1
            && t.contains('$')
            && (t.starts_with("public ")
                || t.starts_with("protected ")
                || t.starts_with("private "))
        {
            if let Some(dollar_pos) = t.find('$') {
                let after_dollar = &t[dollar_pos + 1..];
                let name: String = after_dollar
                    .chars()
                    .take_while(|c| c.is_alphanumeric() || *c == '_')
                    .collect();
                if !name.is_empty() && !name.starts_with('_') {
                    properties.push(name);
                }
            }
        }
    }
    properties
}

/// Returns `(name, stub, source_class)` tuples walking the full hierarchy.
/// `stub` is the declaration up to (but not including) `=`, e.g.
/// `"protected static ?string $model"`.
pub fn collect_class_property_stubs_with_source(
    fqn: &str,
    classmap: &HashMap<String, PathBuf>,
) -> Vec<(String, String, String)> {
    let mut visited = HashSet::new();
    let mut out = Vec::new();
    collect_property_stubs_recursive(fqn, classmap, &mut visited, &mut out);
    out
}

fn collect_property_stubs_recursive(
    class_fqn: &str,
    classmap: &HashMap<String, PathBuf>,
    visited: &mut HashSet<String>,
    out: &mut Vec<(String, String, String)>,
) {
    if !visited.insert(class_fqn.to_string()) {
        return;
    }
    let Some(path) = classmap.get(class_fqn) else { return; };
    let Ok(source) = fs::read_to_string(path) else { return; };

    let namespace = parse_namespace(&source);
    let use_map = parse_use_statements(&source);
    let short = class_fqn.split('\\').last().unwrap_or(class_fqn).to_string();

    for (name, stub) in parse_class_property_stubs(&source) {
        out.push((name, stub, short.clone()));
    }
    for trait_short in parse_used_traits(&source) {
        let fqn = resolve_fqn(&trait_short, &namespace, &use_map);
        collect_property_stubs_recursive(&fqn, classmap, visited, out);
    }
    if let Some(parent_short) = parse_extends(&source) {
        let fqn = resolve_fqn(&parent_short, &namespace, &use_map);
        collect_property_stubs_recursive(&fqn, classmap, visited, out);
    }
}

/// Parse property declarations, returning `(name, stub)` where `stub` is the
/// full declaration up to (but not including) `=`, trimmed.
/// E.g. `protected $fillable = [];` → `("fillable", "protected $fillable")`.
fn parse_class_property_stubs(source: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut in_body = false;
    let mut depth = 0i32;

    for line in source.lines() {
        let t = line.trim();
        if !in_body {
            if t.contains('{')
                && (t.starts_with("class ")
                    || t.starts_with("abstract class ")
                    || t.starts_with("trait "))
            {
                in_body = true;
                depth = 1;
                continue;
            }
            if t == "{" {
                in_body = true;
                depth = 1;
                continue;
            }
            continue;
        }
        for ch in t.chars() {
            match ch {
                '{' => depth += 1,
                '}' => depth -= 1,
                _ => {}
            }
        }
        if depth == 1
            && t.contains('$')
            && (t.starts_with("public ")
                || t.starts_with("protected ")
                || t.starts_with("private "))
        {
            if let Some(pair) = extract_property_stub(t) {
                if !pair.0.starts_with('_') {
                    out.push(pair);
                }
            }
        }
    }
    out
}

fn extract_property_stub(t: &str) -> Option<(String, String)> {
    let dollar_pos = t.find('$')?;
    let after_dollar = &t[dollar_pos + 1..];
    let name: String = after_dollar
        .chars()
        .take_while(|c| c.is_alphanumeric() || *c == '_')
        .collect();
    if name.is_empty() {
        return None;
    }
    let stub = if let Some(eq_pos) = t.find('=') {
        t[..eq_pos].trim_end().to_string()
    } else if let Some(semi_pos) = t.find(';') {
        t[..semi_pos].trim_end().to_string()
    } else {
        t[..dollar_pos + 1 + name.len()].to_string()
    };
    Some((name, stub))
}

pub fn parse_namespace_pub(source: &str) -> String {
    parse_namespace(source)
}

pub fn parse_extends_pub(source: &str) -> Option<String> {
    parse_extends(source)
}

pub fn parse_file_use_statements(source: &str) -> HashMap<String, String> {
    parse_use_statements(source)
}

pub fn resolve_fqn(short: &str, namespace: &str, use_map: &HashMap<String, String>) -> String {
    if short.starts_with('\\') {
        return short[1..].to_string();
    }
    let root = short.split('\\').next().unwrap_or(short);
    if let Some(fqn) = use_map.get(root) {
        let rest: &str = short.splitn(2, '\\').nth(1).unwrap_or("");
        if rest.is_empty() {
            return fqn.clone();
        }
        let base_ns = fqn.rsplitn(2, '\\').last().unwrap_or(fqn.as_str());
        return format!("{base_ns}\\{rest}");
    }
    if namespace.is_empty() {
        short.to_string()
    } else {
        format!("{namespace}\\{short}")
    }
}

fn extract_quoted(s: &str) -> Option<String> {
    let start = s.find('\'')? + 1;
    let end = s[start..].find('\'')? + start;
    Some(s[start..end].replace("\\\\", "\\"))
}

fn extract_path<'a>(line: &'a str, marker: &str) -> Option<&'a str> {
    let start = line.find(marker)? + marker.len();
    let end = line[start..].find('\'')?;
    Some(&line[start..start + end])
}
