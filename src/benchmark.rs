use crate::analyzers;
use crate::project::LaravelProject;
use crate::types::{
    ConfigReport, ControllerReport, MiddlewareReport, MigrationReport, ModelReport, OutputMode,
    ProviderReport, RouteReport, ViewReport,
};
use serde::Serialize;
use std::time::Instant;

pub fn run(project: &LaravelProject, mode: OutputMode) -> Result<(), String> {
    let benchmark = benchmark_project(project)?;

    match mode {
        OutputMode::Json => {
            println!(
                "{}",
                serde_json::to_string_pretty(&benchmark).map_err(|error| error.to_string())?
            );
        }
        OutputMode::Text => print_text(&benchmark),
    }

    Ok(())
}

fn benchmark_project(project: &LaravelProject) -> Result<BenchmarkReport, String> {
    let suite_start = ResourceSnapshot::capture()?;
    let wall_start = Instant::now();

    let mut artifacts = Vec::new();
    let mut steps = Vec::new();

    run_step("routes", &mut artifacts, &mut steps, || {
        analyzers::routes::analyze(project).map(Artifact::Routes)
    })?;
    run_step("middleware", &mut artifacts, &mut steps, || {
        analyzers::middleware::analyze(project).map(Artifact::Middleware)
    })?;
    run_step("config", &mut artifacts, &mut steps, || {
        analyzers::configs::analyze(project).map(Artifact::Config)
    })?;
    run_step("providers", &mut artifacts, &mut steps, || {
        analyzers::providers::analyze(project).map(Artifact::Providers)
    })?;
    run_step("views", &mut artifacts, &mut steps, || {
        analyzers::views::analyze(project).map(Artifact::Views)
    })?;
    run_step("models", &mut artifacts, &mut steps, || {
        analyzers::models::analyze(project).map(Artifact::Models)
    })?;
    run_step("migrations", &mut artifacts, &mut steps, || {
        analyzers::migrations::analyze(project).map(Artifact::Migrations)
    })?;
    run_step("controllers", &mut artifacts, &mut steps, || {
        analyzers::controllers::analyze(project).map(Artifact::Controllers)
    })?;

    let suite_end = ResourceSnapshot::capture()?;
    let retained_report_bytes = artifacts.iter().map(Artifact::json_size_bytes).sum();

    Ok(BenchmarkReport {
        project_name: project.name.clone(),
        project_root: project.root.clone(),
        analyzer_count: steps.len(),
        total_wall_ms: wall_start.elapsed().as_secs_f64() * 1000.0,
        total_cpu_user_ms: diff_ms(suite_end.user_cpu_sec, suite_start.user_cpu_sec),
        total_cpu_system_ms: diff_ms(suite_end.system_cpu_sec, suite_start.system_cpu_sec),
        start_peak_rss_bytes: suite_start.peak_rss_bytes,
        end_peak_rss_bytes: suite_end.peak_rss_bytes,
        peak_rss_delta_bytes: suite_end
            .peak_rss_bytes
            .saturating_sub(suite_start.peak_rss_bytes),
        retained_report_bytes,
        steps,
    })
}

fn run_step<F>(
    name: &'static str,
    artifacts: &mut Vec<Artifact>,
    steps: &mut Vec<BenchmarkStep>,
    analyze: F,
) -> Result<(), String>
where
    F: FnOnce() -> Result<Artifact, String>,
{
    let start = ResourceSnapshot::capture()?;
    let wall = Instant::now();
    let artifact = analyze()?;
    let end = ResourceSnapshot::capture()?;

    steps.push(BenchmarkStep {
        analyzer: name,
        item_count: artifact.item_count(),
        wall_ms: wall.elapsed().as_secs_f64() * 1000.0,
        cpu_user_ms: diff_ms(end.user_cpu_sec, start.user_cpu_sec),
        cpu_system_ms: diff_ms(end.system_cpu_sec, start.system_cpu_sec),
        peak_rss_delta_bytes: end.peak_rss_bytes.saturating_sub(start.peak_rss_bytes),
        peak_rss_bytes: end.peak_rss_bytes,
        retained_report_bytes: artifact.json_size_bytes(),
    });

    artifacts.push(artifact);
    Ok(())
}

fn print_text(report: &BenchmarkReport) {
    println!("Project: {}", report.project_name);
    println!("Root: {}", report.project_root.display());
    println!("Analyzers: {}", report.analyzer_count);
    println!(
        "Total: wall {:.2} ms | cpu user {:.2} ms | cpu system {:.2} ms",
        report.total_wall_ms, report.total_cpu_user_ms, report.total_cpu_system_ms
    );
    println!(
        "Memory: peak start {} | peak end {} | peak delta {} | retained reports {}",
        format_bytes(report.start_peak_rss_bytes),
        format_bytes(report.end_peak_rss_bytes),
        format_bytes(report.peak_rss_delta_bytes),
        format_bytes(report.retained_report_bytes),
    );
    println!();
    println!(
        "{:<12} {:>8} {:>12} {:>12} {:>12} {:>12} {:>12} {:>12}",
        "analyzer", "items", "wall ms", "user ms", "sys ms", "peak delta", "peak rss", "report"
    );
    for step in &report.steps {
        println!(
            "{:<12} {:>8} {:>12.2} {:>12.2} {:>12.2} {:>12} {:>12} {:>12}",
            step.analyzer,
            step.item_count,
            step.wall_ms,
            step.cpu_user_ms,
            step.cpu_system_ms,
            format_bytes(step.peak_rss_delta_bytes),
            format_bytes(step.peak_rss_bytes),
            format_bytes(step.retained_report_bytes),
        );
    }
}

fn format_bytes(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];

    let mut value = bytes as f64;
    let mut unit = 0usize;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }

    if unit == 0 {
        format!("{bytes}{}", UNITS[unit])
    } else {
        format!("{value:.2}{}", UNITS[unit])
    }
}

fn diff_ms(end: f64, start: f64) -> f64 {
    (end - start).max(0.0) * 1000.0
}

#[derive(Serialize)]
struct BenchmarkReport {
    project_name: String,
    project_root: std::path::PathBuf,
    analyzer_count: usize,
    total_wall_ms: f64,
    total_cpu_user_ms: f64,
    total_cpu_system_ms: f64,
    start_peak_rss_bytes: u64,
    end_peak_rss_bytes: u64,
    peak_rss_delta_bytes: u64,
    retained_report_bytes: u64,
    steps: Vec<BenchmarkStep>,
}

#[derive(Serialize)]
struct BenchmarkStep {
    analyzer: &'static str,
    item_count: usize,
    wall_ms: f64,
    cpu_user_ms: f64,
    cpu_system_ms: f64,
    peak_rss_delta_bytes: u64,
    peak_rss_bytes: u64,
    retained_report_bytes: u64,
}

struct ResourceSnapshot {
    user_cpu_sec: f64,
    system_cpu_sec: f64,
    peak_rss_bytes: u64,
}

impl ResourceSnapshot {
    fn capture() -> Result<Self, String> {
        let usage = current_usage()?;
        Ok(Self {
            user_cpu_sec: timeval_to_sec(usage.ru_utime),
            system_cpu_sec: timeval_to_sec(usage.ru_stime),
            peak_rss_bytes: peak_rss_bytes(&usage),
        })
    }
}

#[cfg(unix)]
fn current_usage() -> Result<libc::rusage, String> {
    let mut usage = std::mem::MaybeUninit::<libc::rusage>::zeroed();
    let code = unsafe { libc::getrusage(libc::RUSAGE_SELF, usage.as_mut_ptr()) };
    if code != 0 {
        return Err(std::io::Error::last_os_error().to_string());
    }
    Ok(unsafe { usage.assume_init() })
}

#[cfg(unix)]
fn timeval_to_sec(value: libc::timeval) -> f64 {
    value.tv_sec as f64 + (value.tv_usec as f64 / 1_000_000.0)
}

#[cfg(target_os = "macos")]
fn peak_rss_bytes(usage: &libc::rusage) -> u64 {
    usage.ru_maxrss as u64
}

#[cfg(all(unix, not(target_os = "macos")))]
fn peak_rss_bytes(usage: &libc::rusage) -> u64 {
    (usage.ru_maxrss as u64) * 1024
}

enum Artifact {
    Routes(RouteReport),
    Middleware(MiddlewareReport),
    Config(ConfigReport),
    Providers(ProviderReport),
    Views(ViewReport),
    Models(ModelReport),
    Migrations(MigrationReport),
    Controllers(ControllerReport),
}

impl Artifact {
    fn item_count(&self) -> usize {
        match self {
            Self::Routes(report) => report.route_count,
            Self::Middleware(report) => {
                report.alias_count + report.group_count + report.pattern_count
            }
            Self::Config(report) => report.item_count,
            Self::Providers(report) => report.provider_count,
            Self::Views(report) => {
                report.view_count + report.blade_component_count + report.livewire_component_count
            }
            Self::Models(report) => report.model_count,
            Self::Migrations(report) => report.migration_count,
            Self::Controllers(report) => report.controller_count,
        }
    }

    fn json_size_bytes(&self) -> u64 {
        match self {
            Self::Routes(report) => json_size(report),
            Self::Middleware(report) => json_size(report),
            Self::Config(report) => json_size(report),
            Self::Providers(report) => json_size(report),
            Self::Views(report) => json_size(report),
            Self::Models(report) => json_size(report),
            Self::Migrations(report) => json_size(report),
            Self::Controllers(report) => json_size(report),
        }
    }
}

fn json_size<T: Serialize>(value: &T) -> u64 {
    serde_json::to_vec(value)
        .map(|bytes| bytes.len() as u64)
        .unwrap_or(0)
}
