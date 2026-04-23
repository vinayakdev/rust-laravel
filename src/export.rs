use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;

use dialoguer::{FuzzySelect, MultiSelect, theme::ColorfulTheme};
use rayon::ThreadPoolBuilder;
use rayon::prelude::*;
use serde::Serialize;

use crate::analyzers;
use crate::project::{self, LaravelProject};

#[derive(Clone)]
pub struct ExportOptions {
    pub output_dir: PathBuf,
    pub picker: PickerMode,
    pub projects: Vec<String>,
    pub jobs: Option<usize>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum PickerMode {
    Arrows,
    Fuzzy,
}

impl PickerMode {
    pub fn parse(value: &str) -> Result<Self, String> {
        match value {
            "arrows" => Ok(Self::Arrows),
            "fuzzy" => Ok(Self::Fuzzy),
            _ => Err(format!(
                "unknown picker: {value}. expected one of: arrows, fuzzy"
            )),
        }
    }
}

impl fmt::Display for PickerMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Arrows => f.write_str("arrows"),
            Self::Fuzzy => f.write_str("fuzzy"),
        }
    }
}

pub fn run(options: &ExportOptions) -> Result<(), String> {
    let selected_projects = if options.projects.is_empty() {
        select_projects(options.picker)?
    } else {
        resolve_projects(&options.projects)?
    };

    if selected_projects.is_empty() {
        return Err("no projects selected".to_string());
    }

    fs::create_dir_all(&options.output_dir).map_err(|error| {
        format!(
            "failed to create output dir {}: {error}",
            options.output_dir.display()
        )
    })?;

    let jobs = options.jobs.unwrap_or_else(default_parallelism);
    let pool = ThreadPoolBuilder::new()
        .num_threads(jobs.max(1))
        .build()
        .map_err(|error| error.to_string())?;

    let started_at = Instant::now();
    let output_dir = options.output_dir.clone();
    let summaries = pool.install(|| {
        selected_projects
            .par_iter()
            .map(|project| export_project(project, &output_dir))
            .collect::<Vec<_>>()
    });

    let mut completed = Vec::new();
    for summary in summaries {
        completed.push(summary?);
    }

    completed.sort_by(|left, right| left.project_name.cmp(&right.project_name));

    println!(
        "Exported {} project(s) to {} in {} ms using {} worker(s).",
        completed.len(),
        options.output_dir.display(),
        started_at.elapsed().as_millis(),
        jobs.max(1)
    );

    for summary in completed {
        println!(
            "\n{} -> {} ({} ms, {})",
            summary.project_name,
            summary.output_dir.display(),
            summary.duration_ms,
            human_size(summary.total_size_bytes)
        );
        for file in summary.files {
            println!(
                "  {}: {} ({} ms)",
                file.path.display(),
                human_size(file.size_bytes),
                file.duration_ms
            );
        }
    }

    Ok(())
}

fn export_project(project: &LaravelProject, output_dir: &Path) -> Result<ProjectSummary, String> {
    let project_started = Instant::now();
    let project_dir = output_dir.join(sanitize_name(&project.name));
    fs::create_dir_all(&project_dir).map_err(|error| {
        format!(
            "failed to create project output dir {}: {error}",
            project_dir.display()
        )
    })?;

    let mut files = Vec::new();

    files.push(write_yaml_report(
        &project_dir.join("routes.yaml"),
        &analyzers::routes::analyze(project)?,
    )?);
    files.push(write_yaml_report(
        &project_dir.join("middleware.yaml"),
        &analyzers::middleware::analyze(project)?,
    )?);
    files.push(write_yaml_report(
        &project_dir.join("configs.yaml"),
        &analyzers::configs::analyze(project)?,
    )?);
    files.push(write_yaml_report(
        &project_dir.join("controllers.yaml"),
        &analyzers::controllers::analyze(project)?,
    )?);
    files.push(write_yaml_report(
        &project_dir.join("providers.yaml"),
        &analyzers::providers::analyze(project)?,
    )?);
    files.push(write_yaml_report(
        &project_dir.join("views.yaml"),
        &analyzers::views::analyze(project)?,
    )?);
    files.push(write_yaml_report(
        &project_dir.join("models.yaml"),
        &analyzers::models::analyze(project)?,
    )?);
    files.push(write_yaml_report(
        &project_dir.join("migrations.yaml"),
        &analyzers::migrations::analyze(project)?,
    )?);

    let total_size_bytes = files.iter().map(|file| file.size_bytes).sum();

    Ok(ProjectSummary {
        project_name: project.name.clone(),
        output_dir: project_dir,
        duration_ms: project_started.elapsed().as_millis(),
        total_size_bytes,
        files,
    })
}

fn write_yaml_report<T>(path: &Path, report: &T) -> Result<FileSummary, String>
where
    T: Serialize,
{
    let started_at = Instant::now();
    let yaml = serde_yaml::to_string(report).map_err(|error| error.to_string())?;
    fs::write(path, yaml.as_bytes())
        .map_err(|error| format!("failed to write {}: {error}", path.display()))?;
    let size_bytes = fs::metadata(path)
        .map_err(|error| format!("failed to read metadata for {}: {error}", path.display()))?
        .len();

    Ok(FileSummary {
        path: path.to_path_buf(),
        size_bytes,
        duration_ms: started_at.elapsed().as_millis(),
    })
}

fn resolve_projects(values: &[String]) -> Result<Vec<LaravelProject>, String> {
    values
        .iter()
        .map(|value| project::resolve(Some(value)))
        .collect()
}

fn select_projects(picker: PickerMode) -> Result<Vec<LaravelProject>, String> {
    let projects = project::discover_projects()?;
    if projects.is_empty() {
        return Err(
            "no Laravel projects found. put one under ./laravel-example/<project>, ./test/<project>, or run from a Laravel app"
                .to_string(),
        );
    }

    match picker {
        PickerMode::Arrows => select_projects_arrows(&projects),
        PickerMode::Fuzzy => select_projects_fuzzy(&projects),
    }
}

fn select_projects_arrows(projects: &[LaravelProject]) -> Result<Vec<LaravelProject>, String> {
    let theme = ColorfulTheme::default();
    let labels = projects
        .iter()
        .map(|project| format!("{}  {}", project.name, project.root.display()))
        .collect::<Vec<_>>();

    let selected = MultiSelect::with_theme(&theme)
        .with_prompt("Select Laravel projects to export")
        .items(&labels)
        .interact()
        .map_err(|error| error.to_string())?;

    Ok(selected
        .into_iter()
        .map(|index| projects[index].clone())
        .collect())
}

fn select_projects_fuzzy(projects: &[LaravelProject]) -> Result<Vec<LaravelProject>, String> {
    let theme = ColorfulTheme::default();
    let mut remaining = projects.to_vec();
    let mut selected = Vec::new();

    loop {
        let mut labels = remaining
            .iter()
            .map(|project| format!("{}  {}", project.name, project.root.display()))
            .collect::<Vec<_>>();
        labels.push("[done]".to_string());

        let choice = FuzzySelect::with_theme(&theme)
            .with_prompt("Fuzzy-pick a project, or choose [done]")
            .items(&labels)
            .default(0)
            .interact()
            .map_err(|error| error.to_string())?;

        if choice == remaining.len() {
            break;
        }

        selected.push(remaining.remove(choice));
    }

    Ok(selected)
}

fn sanitize_name(value: &str) -> String {
    let sanitized = value
        .chars()
        .map(|ch| match ch {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '-' | '_' => ch,
            _ => '-',
        })
        .collect::<String>();

    if sanitized.is_empty() {
        "project".to_string()
    } else {
        sanitized
    }
}

fn default_parallelism() -> usize {
    std::thread::available_parallelism()
        .map(|value| value.get())
        .unwrap_or(1)
}

fn human_size(size_bytes: u64) -> String {
    const UNITS: [&str; 4] = ["B", "KB", "MB", "GB"];
    let mut size = size_bytes as f64;
    let mut unit = 0usize;

    while size >= 1024.0 && unit + 1 < UNITS.len() {
        size /= 1024.0;
        unit += 1;
    }

    if unit == 0 {
        format!("{size_bytes} {}", UNITS[unit])
    } else {
        format!("{size:.1} {}", UNITS[unit])
    }
}

struct ProjectSummary {
    project_name: String,
    output_dir: PathBuf,
    duration_ms: u128,
    total_size_bytes: u64,
    files: Vec<FileSummary>,
}

struct FileSummary {
    path: PathBuf,
    size_bytes: u64,
    duration_ms: u128,
}
