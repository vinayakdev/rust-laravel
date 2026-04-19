use std::cmp::min;
use std::io::{self, Write};
use std::path::Path;
use std::time::Duration;

use crossterm::cursor::{Hide, MoveTo, Show};
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use crossterm::execute;
use crossterm::style::{Attribute, Print, SetAttribute};
use crossterm::terminal::{
    self, Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode,
    enable_raw_mode,
};

use crate::analyzers;
use crate::output::text;
use crate::project::{self, LaravelProject};

const COMMANDS: [DebugCommand; 6] = [
    DebugCommand::RouteList,
    DebugCommand::RouteSources,
    DebugCommand::MiddlewareList,
    DebugCommand::ConfigList,
    DebugCommand::ConfigSources,
    DebugCommand::ProviderList,
];

#[derive(Clone, Copy, PartialEq, Eq)]
enum FocusPane {
    Projects,
    Commands,
    Output,
}

#[derive(Clone, Copy)]
enum DebugCommand {
    RouteList,
    RouteSources,
    MiddlewareList,
    ConfigList,
    ConfigSources,
    ProviderList,
}

impl DebugCommand {
    fn label(self) -> &'static str {
        match self {
            DebugCommand::RouteList => "route:list",
            DebugCommand::RouteSources => "route:sources",
            DebugCommand::MiddlewareList => "middleware:list",
            DebugCommand::ConfigList => "config:list",
            DebugCommand::ConfigSources => "config:sources",
            DebugCommand::ProviderList => "provider:list",
        }
    }
}

struct App {
    projects: Vec<LaravelProject>,
    selected_project: usize,
    selected_command: usize,
    focus: FocusPane,
    output_lines: Vec<String>,
    output_scroll: usize,
    status: String,
}

pub fn run(initial_project: Option<&str>) -> Result<(), String> {
    let projects = project::discover_projects()?;
    if projects.is_empty() {
        return Err(
            "no Laravel projects found. put one under ./laravel-example/<project> or run from a Laravel app"
                .to_string(),
        );
    }

    let mut app = App {
        selected_project: resolve_initial_project(initial_project, &projects)?,
        selected_command: 0,
        focus: FocusPane::Projects,
        output_lines: vec![
            "Debug browser".to_string(),
            String::new(),
            "Left pane: choose a project and command.".to_string(),
            "Arrow keys move. Left/Right switches focus.".to_string(),
            "Enter runs the selected analyzer.".to_string(),
            "When Output is focused, Up/Down scroll.".to_string(),
            "Press q or Esc to exit.".to_string(),
        ],
        output_scroll: 0,
        status: "Ready".to_string(),
        projects,
    };

    let mut stdout = io::stdout();
    enable_raw_mode().map_err(|e| e.to_string())?;
    execute!(stdout, EnterAlternateScreen, Hide).map_err(|e| e.to_string())?;

    let run_result = loop_run(&mut stdout, &mut app);

    let cleanup_result = (|| -> Result<(), String> {
        disable_raw_mode().map_err(|e| e.to_string())?;
        execute!(stdout, Show, LeaveAlternateScreen).map_err(|e| e.to_string())?;
        Ok(())
    })();

    run_result.and(cleanup_result)
}

fn loop_run(stdout: &mut io::Stdout, app: &mut App) -> Result<(), String> {
    loop {
        draw(stdout, app)?;

        if !event::poll(Duration::from_millis(250)).map_err(|e| e.to_string())? {
            continue;
        }

        let Event::Key(key) = event::read().map_err(|e| e.to_string())? else {
            continue;
        };
        if key.kind != KeyEventKind::Press {
            continue;
        }

        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => return Ok(()),
            KeyCode::Left => focus_left(app),
            KeyCode::Right | KeyCode::Tab => focus_right(app),
            KeyCode::Up => move_up(app),
            KeyCode::Down => move_down(app),
            KeyCode::PageUp => scroll_output(app, 10, true),
            KeyCode::PageDown => scroll_output(app, 10, false),
            KeyCode::Enter | KeyCode::Char(' ') => run_selected(app),
            _ => {}
        }
    }
}

fn resolve_initial_project(
    initial_project: Option<&str>,
    projects: &[LaravelProject],
) -> Result<usize, String> {
    let Some(value) = initial_project else {
        return Ok(0);
    };

    let resolved = project::resolve(Some(value))?;
    Ok(projects
        .iter()
        .position(|candidate| candidate.root == resolved.root)
        .unwrap_or(0))
}

fn run_selected(app: &mut App) {
    let project = &app.projects[app.selected_project];
    let command = COMMANDS[app.selected_command];
    app.status = format!("Running {} for {}", command.label(), project.name);

    match render_report(project, command) {
        Ok(output) => {
            app.output_lines = output.lines().map(|line| line.to_string()).collect();
            if app.output_lines.is_empty() {
                app.output_lines.push("No output".to_string());
            }
            app.output_scroll = 0;
            app.status = format!("Loaded {} for {}", command.label(), project.name);
        }
        Err(error) => {
            app.output_lines = error.lines().map(|line| line.to_string()).collect();
            app.output_scroll = 0;
            app.status = "Analyzer failed".to_string();
        }
    }
}

fn render_report(project: &LaravelProject, command: DebugCommand) -> Result<String, String> {
    match command {
        DebugCommand::RouteList | DebugCommand::RouteSources => {
            let report = analyzers::routes::analyze(project)?;
            Ok(match command {
                DebugCommand::RouteList => text::routes::render_route_table(&report.routes),
                DebugCommand::RouteSources => {
                    text::routes::render_route_source_table(&report.routes)
                }
                _ => unreachable!(),
            })
        }
        DebugCommand::MiddlewareList => {
            let report = analyzers::middleware::analyze(project)?;
            Ok(text::middleware::render_middleware_tables(&report))
        }
        DebugCommand::ConfigList | DebugCommand::ConfigSources => {
            let report = analyzers::configs::analyze(project)?;
            Ok(match command {
                DebugCommand::ConfigList => text::configs::render_config_table(&report),
                DebugCommand::ConfigSources => {
                    text::configs::render_config_source_table(&report)
                }
                _ => unreachable!(),
            })
        }
        DebugCommand::ProviderList => {
            let report = analyzers::providers::analyze(project)?;
            Ok(text::providers::render_provider_table(&report))
        }
    }
}

fn focus_left(app: &mut App) {
    app.focus = match app.focus {
        FocusPane::Projects => FocusPane::Projects,
        FocusPane::Commands => FocusPane::Projects,
        FocusPane::Output => FocusPane::Commands,
    };
}

fn focus_right(app: &mut App) {
    app.focus = match app.focus {
        FocusPane::Projects => FocusPane::Commands,
        FocusPane::Commands => FocusPane::Output,
        FocusPane::Output => FocusPane::Output,
    };
}

fn move_up(app: &mut App) {
    match app.focus {
        FocusPane::Projects => {
            if app.selected_project > 0 {
                app.selected_project -= 1;
            }
        }
        FocusPane::Commands => {
            if app.selected_command > 0 {
                app.selected_command -= 1;
            }
        }
        FocusPane::Output => scroll_output(app, 1, true),
    }
}

fn move_down(app: &mut App) {
    match app.focus {
        FocusPane::Projects => {
            if app.selected_project + 1 < app.projects.len() {
                app.selected_project += 1;
            }
        }
        FocusPane::Commands => {
            if app.selected_command + 1 < COMMANDS.len() {
                app.selected_command += 1;
            }
        }
        FocusPane::Output => scroll_output(app, 1, false),
    }
}

fn scroll_output(app: &mut App, amount: usize, up: bool) {
    if up {
        app.output_scroll = app.output_scroll.saturating_sub(amount);
    } else {
        app.output_scroll = app.output_scroll.saturating_add(amount);
    }
}

fn draw(stdout: &mut io::Stdout, app: &App) -> Result<(), String> {
    let (width, height) = terminal::size().map_err(|e| e.to_string())?;
    let nav_width = min(40, width.saturating_div(3).max(28));
    let output_width = width.saturating_sub(nav_width + 1);
    let content_height = height.saturating_sub(2);
    let project_height = content_height.saturating_div(2).max(6);
    let command_height = content_height.saturating_sub(project_height);

    execute!(stdout, MoveTo(0, 0), Clear(ClearType::All)).map_err(|e| e.to_string())?;

    draw_box(stdout, 0, 0, nav_width, project_height, " Projects ", app.focus == FocusPane::Projects)?;
    draw_box(
        stdout,
        0,
        project_height,
        nav_width,
        command_height,
        " Commands ",
        app.focus == FocusPane::Commands,
    )?;
    draw_box(
        stdout,
        nav_width,
        0,
        output_width,
        content_height,
        " Output ",
        app.focus == FocusPane::Output,
    )?;
    draw_status(stdout, 0, height.saturating_sub(1), width, &app.status)?;

    draw_projects(stdout, app, nav_width, project_height)?;
    draw_commands(stdout, app, nav_width, project_height, command_height)?;
    draw_output(stdout, app, nav_width, output_width, content_height)?;

    stdout.flush().map_err(|e| e.to_string())
}

fn draw_projects(
    stdout: &mut io::Stdout,
    app: &App,
    width: u16,
    height: u16,
) -> Result<(), String> {
    let visible_rows = height.saturating_sub(2) as usize;
    let start = list_window(app.selected_project, app.projects.len(), visible_rows);
    for (row, project) in app.projects.iter().enumerate().skip(start).take(visible_rows) {
        let y = 1 + (row - start) as u16;
        let selected = row == app.selected_project;
        let text = format!("{} [{}]", project.name, display_path(&project.root));
        draw_line(stdout, 1, y, width.saturating_sub(2), &text, selected)?;
    }
    Ok(())
}

fn draw_commands(
    stdout: &mut io::Stdout,
    app: &App,
    width: u16,
    top: u16,
    height: u16,
) -> Result<(), String> {
    let visible_rows = height.saturating_sub(2) as usize;
    let start = list_window(app.selected_command, COMMANDS.len(), visible_rows);
    for row in start..min(start + visible_rows, COMMANDS.len()) {
        let y = top + 1 + (row - start) as u16;
        let selected = row == app.selected_command;
        draw_line(
            stdout,
            1,
            y,
            width.saturating_sub(2),
            COMMANDS[row].label(),
            selected,
        )?;
    }
    Ok(())
}

fn draw_output(
    stdout: &mut io::Stdout,
    app: &App,
    left: u16,
    width: u16,
    height: u16,
) -> Result<(), String> {
    let usable_width = width.saturating_sub(2) as usize;
    let mut wrapped = Vec::new();
    for line in &app.output_lines {
        wrap_line(line, usable_width.max(1), &mut wrapped);
    }

    let visible_rows = height.saturating_sub(2) as usize;
    let max_scroll = wrapped.len().saturating_sub(visible_rows);
    let scroll = min(app.output_scroll, max_scroll);

    for (row, line) in wrapped.iter().skip(scroll).take(visible_rows).enumerate() {
        draw_line(
            stdout,
            left + 1,
            1 + row as u16,
            width.saturating_sub(2),
            line,
            false,
        )?;
    }
    Ok(())
}

fn draw_status(
    stdout: &mut io::Stdout,
    x: u16,
    y: u16,
    width: u16,
    status: &str,
) -> Result<(), String> {
    let text = truncate(status, width as usize);
    execute!(
        stdout,
        MoveTo(x, y),
        SetAttribute(Attribute::Reverse),
        Print(format!("{text:<width$}", width = width as usize)),
        SetAttribute(Attribute::Reset)
    )
    .map_err(|e| e.to_string())
}

fn draw_box(
    stdout: &mut io::Stdout,
    x: u16,
    y: u16,
    width: u16,
    height: u16,
    title: &str,
    focused: bool,
) -> Result<(), String> {
    if width < 2 || height < 2 {
        return Ok(());
    }

    let horizontal = "─".repeat(width.saturating_sub(2) as usize);
    execute!(
        stdout,
        MoveTo(x, y),
        Print("┌"),
        Print(&horizontal),
        Print("┐")
    )
    .map_err(|e| e.to_string())?;

    for row in 1..height.saturating_sub(1) {
        execute!(
            stdout,
            MoveTo(x, y + row),
            Print("│"),
            MoveTo(x + width.saturating_sub(1), y + row),
            Print("│")
        )
        .map_err(|e| e.to_string())?;
    }

    execute!(
        stdout,
        MoveTo(x, y + height.saturating_sub(1)),
        Print("└"),
        Print(&horizontal),
        Print("┘")
    )
    .map_err(|e| e.to_string())?;

    let title_text = if focused {
        format!(">{title}<")
    } else {
        title.to_string()
    };
    execute!(stdout, MoveTo(x + 2, y), Print(truncate(&title_text, width as usize - 4)))
        .map_err(|e| e.to_string())
}

fn draw_line(
    stdout: &mut io::Stdout,
    x: u16,
    y: u16,
    width: u16,
    text: &str,
    selected: bool,
) -> Result<(), String> {
    let clipped = truncate(text, width as usize);
    let padded = format!("{clipped:<width$}", width = width as usize);
    if selected {
        execute!(
            stdout,
            MoveTo(x, y),
            SetAttribute(Attribute::Reverse),
            Print(padded),
            SetAttribute(Attribute::Reset)
        )
        .map_err(|e| e.to_string())
    } else {
        execute!(stdout, MoveTo(x, y), Print(padded)).map_err(|e| e.to_string())
    }
}

fn list_window(selected: usize, len: usize, visible: usize) -> usize {
    if len <= visible || visible == 0 {
        return 0;
    }
    let half = visible / 2;
    let start = selected.saturating_sub(half);
    min(start, len.saturating_sub(visible))
}

fn wrap_line(line: &str, width: usize, out: &mut Vec<String>) {
    if line.is_empty() {
        out.push(String::new());
        return;
    }

    let mut current = String::new();
    let mut count = 0usize;
    for ch in line.chars() {
        current.push(ch);
        count += 1;
        if count >= width {
            out.push(current);
            current = String::new();
            count = 0;
        }
    }

    if !current.is_empty() {
        out.push(current);
    }
}

fn truncate(text: &str, width: usize) -> String {
    let count = text.chars().count();
    if count <= width {
        return text.to_string();
    }
    if width <= 1 {
        return "…".to_string();
    }
    let mut output = String::new();
    for ch in text.chars().take(width.saturating_sub(1)) {
        output.push(ch);
    }
    output.push('…');
    output
}

fn display_path(path: &Path) -> String {
    if let Some(name) = path.file_name().and_then(|part| part.to_str()) {
        name.to_string()
    } else {
        path.to_string_lossy().into_owned()
    }
}
