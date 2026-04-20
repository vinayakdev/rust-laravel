use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Clone)]
pub struct Psr4Mapping {
    pub prefix: String,
    pub base_dir: PathBuf,
    pub package_name: Option<String>,
}

/// Collects all PSR-4 autoload mappings from the project root composer.json
/// and any packages under packages/<vendor>/<name>/composer.json.
pub fn collect_psr4_mappings(project_root: &Path) -> Result<Vec<Psr4Mapping>, String> {
    let mut mappings = Vec::new();
    let root_composer = read_json(&project_root.join("composer.json"))?;
    mappings.extend(psr4_from_composer(project_root, &root_composer, None));

    let packages_root = project_root.join("packages");
    if let Ok(vendors) = fs::read_dir(&packages_root) {
        for vendor in vendors.flatten() {
            let vendor_path = vendor.path();
            if !vendor_path.is_dir() {
                continue;
            }
            if let Ok(packages) = fs::read_dir(&vendor_path) {
                for package in packages.flatten() {
                    let package_path = package.path();
                    let composer_path = package_path.join("composer.json");
                    if composer_path.is_file() {
                        let composer = read_json(&composer_path)?;
                        let package_name = composer
                            .get("name")
                            .and_then(Value::as_str)
                            .map(ToString::to_string);
                        mappings.extend(psr4_from_composer(&package_path, &composer, package_name));
                    }
                }
            }
        }
    }
    Ok(mappings)
}

pub fn psr4_from_composer(
    root: &Path,
    composer: &Value,
    package_name: Option<String>,
) -> Vec<Psr4Mapping> {
    let mut mappings = Vec::new();
    let Some(psr4) = composer
        .get("autoload")
        .and_then(|a| a.get("psr-4"))
        .and_then(Value::as_object)
    else {
        return mappings;
    };

    for (prefix, value) in psr4 {
        match value {
            Value::String(path) => mappings.push(Psr4Mapping {
                prefix: prefix.clone(),
                base_dir: root.join(path),
                package_name: package_name.clone(),
            }),
            Value::Array(paths) => {
                for path in paths.iter().filter_map(Value::as_str) {
                    mappings.push(Psr4Mapping {
                        prefix: prefix.clone(),
                        base_dir: root.join(path),
                        package_name: package_name.clone(),
                    });
                }
            }
            _ => {}
        }
    }
    mappings
}

pub fn resolve_class_file(class: &str, mappings: &[Psr4Mapping]) -> Option<PathBuf> {
    let normalized = class.trim_start_matches('\\');
    for mapping in mappings {
        if let Some(rest) = normalized.strip_prefix(&mapping.prefix) {
            let relative = rest.replace('\\', "/");
            let path = mapping.base_dir.join(format!("{relative}.php"));
            if path.is_file() {
                return Some(path);
            }
        }
    }
    None
}

pub fn resolve_namespace_dir(namespace: &str, mappings: &[Psr4Mapping]) -> Option<PathBuf> {
    let normalized = namespace.trim_start_matches('\\').trim_end_matches('\\');
    for mapping in mappings {
        if let Some(rest) = normalized.strip_prefix(mapping.prefix.trim_end_matches('\\')) {
            let rest = rest.trim_start_matches('\\');
            let path = if rest.is_empty() {
                mapping.base_dir.clone()
            } else {
                mapping.base_dir.join(rest.replace('\\', "/"))
            };
            if path.is_dir() {
                return Some(path);
            }
        }
    }
    None
}

pub fn package_name_for_source(path: &Option<PathBuf>, mappings: &[Psr4Mapping]) -> Option<String> {
    let path = path.as_ref()?;
    for mapping in mappings {
        if path.starts_with(&mapping.base_dir) && mapping.package_name.is_some() {
            return mapping.package_name.clone();
        }
    }
    None
}

pub fn read_json(path: &Path) -> Result<Value, String> {
    let text =
        fs::read_to_string(path).map_err(|e| format!("failed to read {}: {e}", path.display()))?;
    serde_json::from_str(&text).map_err(|e| format!("failed to parse {}: {e}", path.display()))
}

pub fn laravel_providers(composer: &Value) -> Option<Vec<String>> {
    composer
        .get("extra")
        .and_then(|e| e.get("laravel"))
        .and_then(|l| l.get("providers"))
        .and_then(Value::as_array)
        .map(|providers| {
            providers
                .iter()
                .filter_map(Value::as_str)
                .map(ToString::to_string)
                .collect()
        })
}
