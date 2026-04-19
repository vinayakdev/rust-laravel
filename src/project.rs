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

    let root = match project_arg {
        Some(value) => {
            let direct = PathBuf::from(value);
            if looks_like_laravel_project(&direct) {
                direct
            } else {
                let nested = workspace_root.join(value);
                if looks_like_laravel_project(&nested) {
                    nested
                } else {
                    return Err(format!(
                        "could not resolve Laravel project: {value}\nlooked at: {}\nand: {}",
                        direct.display(),
                        nested.display()
                    ));
                }
            }
        }
        None => {
            if looks_like_laravel_project(&cwd) {
                cwd
            } else if looks_like_laravel_project(&workspace_root) {
                workspace_root
            } else {
                auto_pick_project(&workspace_root)?
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

fn auto_pick_project(workspace_root: &Path) -> Result<PathBuf, String> {
    let mut matches = Vec::new();

    if let Ok(entries) = fs::read_dir(workspace_root) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() && looks_like_laravel_project(&path) {
                matches.push(path);
            }
        }
    }

    matches.sort();

    match matches.len() {
        0 => Err(format!(
            "no Laravel project found.\nput one under {}/<project> or pass --project <path>",
            workspace_root.display()
        )),
        1 => Ok(matches.remove(0)),
        _ => Err(format!(
            "multiple Laravel projects found under {}.\npass --project <name>.",
            workspace_root.display()
        )),
    }
}

fn looks_like_laravel_project(path: &Path) -> bool {
    path.is_dir() && path.join("routes").is_dir()
}
