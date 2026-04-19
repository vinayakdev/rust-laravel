use crate::project::LaravelProject;
use crate::types::{ProviderEntry, ProviderReport};
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};

pub fn analyze(project: &LaravelProject) -> Result<ProviderReport, String> {
    let mappings = collect_psr4_mappings(project)?;
    let mut providers = Vec::new();

    providers.extend(read_bootstrap_providers(project, &mappings)?);
    providers.extend(read_root_composer_providers(project, &mappings)?);
    providers.extend(read_local_package_providers(project, &mappings)?);

    providers.sort_by(|left, right| {
        left.declared_in
            .cmp(&right.declared_in)
            .then(left.line.cmp(&right.line))
            .then(left.provider_class.cmp(&right.provider_class))
    });

    Ok(ProviderReport {
        project_name: project.name.clone(),
        project_root: project.root.clone(),
        provider_count: providers.len(),
        providers,
    })
}

#[derive(Clone)]
struct Psr4Mapping {
    prefix: String,
    base_dir: PathBuf,
    package_name: Option<String>,
}

fn collect_psr4_mappings(project: &LaravelProject) -> Result<Vec<Psr4Mapping>, String> {
    let mut mappings = Vec::new();
    let root_composer = read_json(&project.root.join("composer.json"))?;
    mappings.extend(psr4_from_composer(&project.root, &root_composer, None));

    let packages_root = project.root.join("packages");
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

fn read_bootstrap_providers(
    project: &LaravelProject,
    mappings: &[Psr4Mapping],
) -> Result<Vec<ProviderEntry>, String> {
    let path = project.root.join("bootstrap/providers.php");
    if !path.is_file() {
        return Ok(Vec::new());
    }

    let source = fs::read_to_string(&path)
        .map_err(|error| format!("failed to read {}: {error}", path.display()))?;

    Ok(extract_class_references(&source)
        .into_iter()
        .map(|reference| {
            build_provider_entry(project, &path, "bootstrap", None, reference, mappings)
        })
        .collect())
}

fn read_root_composer_providers(
    project: &LaravelProject,
    mappings: &[Psr4Mapping],
) -> Result<Vec<ProviderEntry>, String> {
    let path = project.root.join("composer.json");
    let composer = read_json(&path)?;
    let mut providers = Vec::new();

    if let Some(classes) = laravel_providers(&composer) {
        for class in classes {
            let (line, column) = find_json_string_position(&path, &class)?;
            providers.push(build_provider_entry(
                project,
                &path,
                "composer-discovered",
                None,
                ClassReference {
                    class,
                    line,
                    column,
                },
                mappings,
            ));
        }
    }

    Ok(providers)
}

fn read_local_package_providers(
    project: &LaravelProject,
    mappings: &[Psr4Mapping],
) -> Result<Vec<ProviderEntry>, String> {
    let mut providers = Vec::new();
    let packages_root = project.root.join("packages");

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
                    if !composer_path.is_file() {
                        continue;
                    }

                    let composer = read_json(&composer_path)?;
                    let package_name = composer
                        .get("name")
                        .and_then(Value::as_str)
                        .map(ToString::to_string);

                    if let Some(classes) = laravel_providers(&composer) {
                        for class in classes {
                            let (line, column) = find_json_string_position(&composer_path, &class)?;
                            providers.push(build_provider_entry(
                                project,
                                &composer_path,
                                "local-package-composer",
                                package_name.clone(),
                                ClassReference {
                                    class,
                                    line,
                                    column,
                                },
                                mappings,
                            ));
                        }
                    }
                }
            }
        }
    }

    Ok(providers)
}

#[derive(Clone)]
struct ClassReference {
    class: String,
    line: usize,
    column: usize,
}

fn extract_class_references(source: &str) -> Vec<ClassReference> {
    let mut refs = Vec::new();

    for (index, line) in source.lines().enumerate() {
        let mut search = 0;
        while let Some(found) = line[search..].find("::class") {
            let end = search + found;
            let start = line[..end]
                .rfind(|c: char| [' ', '\t', '[', ',', '('].contains(&c))
                .map_or(0, |value| value + 1);
            let class = line[start..end].trim().trim_start_matches('\\').to_string();
            if !class.is_empty() {
                refs.push(ClassReference {
                    class,
                    line: index + 1,
                    column: start + 1,
                });
            }
            search = end + "::class".len();
        }
    }

    refs
}

fn build_provider_entry(
    project: &LaravelProject,
    declared_in: &Path,
    registration_kind: &str,
    package_name: Option<String>,
    reference: ClassReference,
    mappings: &[Psr4Mapping],
) -> ProviderEntry {
    let source_file = resolve_class_file(&reference.class, mappings);
    let package_name = package_name.or_else(|| package_name_for_source(&source_file, mappings));
    let source_available = source_file.is_some();
    let status = if source_available {
        "static_exact"
    } else {
        "source_missing"
    }
    .to_string();

    ProviderEntry {
        provider_class: reference.class,
        line: reference.line,
        column: reference.column,
        registration_kind: registration_kind.to_string(),
        declared_in: strip_root(&project.root, declared_in),
        package_name,
        source_file: source_file
            .as_ref()
            .map(|path| strip_root(&project.root, path)),
        source_available,
        status,
    }
}

fn resolve_class_file(class: &str, mappings: &[Psr4Mapping]) -> Option<PathBuf> {
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

fn package_name_for_source(path: &Option<PathBuf>, mappings: &[Psr4Mapping]) -> Option<String> {
    let path = path.as_ref()?;
    for mapping in mappings {
        if path.starts_with(&mapping.base_dir) && mapping.package_name.is_some() {
            return mapping.package_name.clone();
        }
    }
    None
}

fn psr4_from_composer(
    root: &Path,
    composer: &Value,
    package_name: Option<String>,
) -> Vec<Psr4Mapping> {
    let mut mappings = Vec::new();
    let Some(psr4) = composer
        .get("autoload")
        .and_then(|autoload| autoload.get("psr-4"))
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

fn laravel_providers(composer: &Value) -> Option<Vec<String>> {
    composer
        .get("extra")
        .and_then(|extra| extra.get("laravel"))
        .and_then(|laravel| laravel.get("providers"))
        .and_then(Value::as_array)
        .map(|providers| {
            providers
                .iter()
                .filter_map(Value::as_str)
                .map(ToString::to_string)
                .collect()
        })
}

fn read_json(path: &Path) -> Result<Value, String> {
    let text = fs::read_to_string(path)
        .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    serde_json::from_str(&text)
        .map_err(|error| format!("failed to parse {}: {error}", path.display()))
}

fn find_json_string_position(path: &Path, needle: &str) -> Result<(usize, usize), String> {
    let text = fs::read_to_string(path)
        .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    let quoted = serde_json::to_string(needle)
        .map_err(|error| format!("failed to encode JSON string: {error}"))?;
    let index = text
        .find(&quoted)
        .ok_or_else(|| format!("failed to locate {needle} in {}", path.display()))?;
    let line = 1 + text[..index].bytes().filter(|byte| *byte == b'\n').count();
    let line_start = text[..index].rfind('\n').map_or(0, |offset| offset + 1);
    let column = index - line_start + 1;
    Ok((line, column))
}

fn strip_root(root: &Path, path: &Path) -> PathBuf {
    path.strip_prefix(root).unwrap_or(path).to_path_buf()
}
