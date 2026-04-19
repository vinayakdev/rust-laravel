use std::fs;
use std::path::{Path, PathBuf};

#[derive(Clone)]
pub struct LaravelProject {
    pub root: PathBuf,
    pub name: String,
}

pub fn resolve(project_arg: Option<&str>) -> Result<LaravelProject, String> {
    let cwd = std::env::current_dir().map_err(|error| error.to_string())?;
    let workspace_root = cwd.join("laravel-example");
    let test_root = cwd.join("test");

    let root = match project_arg {
        Some(value) => {
            let direct = cwd.join(value);
            if looks_like_laravel_project(&direct) {
                direct
            } else {
                let nested = workspace_root.join(value);
                if looks_like_laravel_project(&nested) {
                    nested
                } else {
                    let test_nested = test_root.join(value);
                    if looks_like_laravel_project(&test_nested) {
                        test_nested
                    } else {
                        return Err(format!(
                            "could not resolve Laravel project: {value}\nlooked at: {}\nand: {}\nand: {}",
                            direct.display(),
                            nested.display(),
                            test_nested.display()
                        ));
                    }
                }
            }
        }
        None => {
            if looks_like_laravel_project(&workspace_root) {
                workspace_root
            } else if looks_like_laravel_project(&cwd) {
                cwd
            } else {
                auto_pick_project(&[workspace_root, test_root])?
            }
        }
    };

    let name = root
        .file_name()
        .and_then(|part| part.to_str())
        .unwrap_or("laravel-project")
        .to_string();

    Ok(LaravelProject { root, name })
}

pub fn discover_projects() -> Result<Vec<LaravelProject>, String> {
    let cwd = std::env::current_dir().map_err(|error| error.to_string())?;
    let workspace_root = cwd.join("laravel-example");
    let test_root = cwd.join("test");
    let mut roots = Vec::new();

    if looks_like_laravel_project(&cwd) {
        roots.push(cwd.clone());
    }

    roots.extend(discover_projects_under(&workspace_root));
    roots.extend(discover_projects_under(&test_root));

    roots.sort();
    roots.dedup();

    Ok(roots
        .into_iter()
        .map(|root| LaravelProject {
            name: root
                .file_name()
                .and_then(|part| part.to_str())
                .unwrap_or("laravel-project")
                .to_string(),
            root,
        })
        .collect())
}

fn discover_projects_under(root: &Path) -> Vec<PathBuf> {
    let mut projects = Vec::new();

    if let Ok(entries) = fs::read_dir(root) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() && looks_like_laravel_project(&path) {
                projects.push(path);
            }
        }
    }

    projects
}

fn auto_pick_project(search_roots: &[PathBuf]) -> Result<PathBuf, String> {
    let mut matches = Vec::new();

    for search_root in search_roots {
        if let Ok(entries) = fs::read_dir(search_root) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() && looks_like_laravel_project(&path) {
                    matches.push(path);
                }
            }
        }
    }

    matches.sort();

    match matches.len() {
        0 => Err(format!(
            "no Laravel project found.\nput one under ./laravel-example/<project> or ./test/<project> or pass --project <path>"
        )),
        1 => Ok(matches.remove(0)),
        _ => Err(format!(
            "multiple Laravel projects found under ./laravel-example or ./test.\npass --project <name>."
        )),
    }
}

fn looks_like_laravel_project(path: &Path) -> bool {
    path.is_dir() && path.join("routes").is_dir() && path.join("config").is_dir()
}
