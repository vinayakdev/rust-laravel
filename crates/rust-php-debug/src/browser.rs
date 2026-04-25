use std::cmp::min;
use std::io::{self, Write};
use std::time::Duration;

use crossterm::cursor::{Hide, MoveTo, Show};
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use crossterm::execute;
use crossterm::style::{Attribute, Print, SetAttribute};
use crossterm::terminal::{
    self, Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode,
    enable_raw_mode,
};

use crate::project::{self, LaravelProject};

use super::command::{BROWSER_COMMANDS, resolve_initial_project};
use super::reports::render_text_report;

#[derive(Clone, Copy, PartialEq, Eq)]
enum FocusPane {
    Projects,
    Commands,
    Output,
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
    enable_raw_mode().map_err(|error| error.to_string())?;
    execute!(stdout, EnterAlternateScreen, Hide).map_err(|error| error.to_string())?;

    let run_result = loop_run(&mut stdout, &mut app);

    let cleanup_result = (|| -> Result<(), String> {
        disable_raw_mode().map_err(|error| error.to_string())?;
        execute!(stdout, Show, LeaveAlternateScreen).map_err(|error| error.to_string())?;
        Ok(())
    })();

    run_result.and(cleanup_result)
}

fn loop_run(stdout: &mut io::Stdout, app: &mut App) -> Result<(), String> {
    loop {
        draw(stdout, app)?;

        if !event::poll(Duration::from_millis(250)).map_err(|error| error.to_string())? {
            continue;
        }

        let Event::Key(key) = event::read().map_err(|error| error.to_string())? else {
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

fn run_selected(app: &mut App) {
    let project = &app.projects[app.selected_project];
    let command = BROWSER_COMMANDS[app.selected_command];
    app.status = format!("Running {} for {}", command.label(), project.name);

    match render_text_report(project, command) {
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
            if app.selected_command + 1 < BROWSER_COMMANDS.len() {
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
    let (width, height) = terminal::size().map_err(|error| error.to_string())?;
    let width = width as usize;
    let height = height as usize;
    let left_width = min(32usize, width.saturating_sub(24));
    let projects_height = min(app.projects.len() + 3, height.saturating_sub(10));
    let commands_height = min(
        BROWSER_COMMANDS.len() + 3,
        height.saturating_sub(projects_height + 6),
    );

    execute!(stdout, MoveTo(0, 0), Clear(ClearType::All)).map_err(|error| error.to_string())?;

    draw_box(
        stdout,
        0,
        0,
        left_width as u16,
        projects_height as u16,
        "Projects",
    )?;
    for (index, project) in app.projects.iter().enumerate() {
        let y = 1 + index as u16;
        draw_list_item(
            stdout,
            1,
            y,
            (left_width - 2) as u16,
            &project.name,
            app.focus == FocusPane::Projects && app.selected_project == index,
        )?;
    }

    let commands_y = projects_height as u16 + 1;
    draw_box(
        stdout,
        0,
        commands_y,
        left_width as u16,
        commands_height as u16,
        "Commands",
    )?;
    for (index, command) in BROWSER_COMMANDS.iter().enumerate() {
        let y = commands_y + 1 + index as u16;
        draw_list_item(
            stdout,
            1,
            y,
            (left_width - 2) as u16,
            command.label(),
            app.focus == FocusPane::Commands && app.selected_command == index,
        )?;
    }

    let output_x = left_width as u16 + 1;
    let output_width = width.saturating_sub(left_width + 1);
    let output_height = height.saturating_sub(3);
    draw_box(
        stdout,
        output_x,
        0,
        output_width as u16,
        output_height as u16,
        "Output",
    )?;

    let visible_height = output_height.saturating_sub(2);
    let max_scroll = app.output_lines.len().saturating_sub(visible_height);
    let scroll = app.output_scroll.min(max_scroll);
    for (row, line) in app
        .output_lines
        .iter()
        .skip(scroll)
        .take(visible_height)
        .enumerate()
    {
        let clipped = clip(line, output_width.saturating_sub(2));
        execute!(
            stdout,
            MoveTo(output_x + 1, row as u16 + 1),
            Print(format!(
                "{clipped:width$}",
                width = output_width.saturating_sub(2)
            ))
        )
        .map_err(|error| error.to_string())?;
    }

    let footer = format!(
        "{} | {} | {}",
        app.projects[app.selected_project].name,
        BROWSER_COMMANDS[app.selected_command].title(),
        app.status
    );
    execute!(
        stdout,
        MoveTo(0, height.saturating_sub(1) as u16),
        Print(format!("{:width$}", clip(&footer, width), width = width))
    )
    .map_err(|error| error.to_string())?;

    stdout.flush().map_err(|error| error.to_string())
}

fn draw_box(
    stdout: &mut io::Stdout,
    x: u16,
    y: u16,
    width: u16,
    height: u16,
    title: &str,
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
    .map_err(|error| error.to_string())?;

    for row in 1..height.saturating_sub(1) {
        execute!(
            stdout,
            MoveTo(x, y + row),
            Print("│"),
            MoveTo(x + width - 1, y + row),
            Print("│")
        )
        .map_err(|error| error.to_string())?;
    }

    execute!(
        stdout,
        MoveTo(x, y + height - 1),
        Print("└"),
        Print(&horizontal),
        Print("┘"),
        MoveTo(x + 2, y),
        Print(clip(title, width.saturating_sub(4) as usize))
    )
    .map_err(|error| error.to_string())
}

fn draw_list_item(
    stdout: &mut io::Stdout,
    x: u16,
    y: u16,
    width: u16,
    label: &str,
    selected: bool,
) -> Result<(), String> {
    let text = format!(
        "{:width$}",
        clip(label, width as usize),
        width = width as usize
    );
    if selected {
        execute!(
            stdout,
            MoveTo(x, y),
            SetAttribute(Attribute::Reverse),
            Print(text),
            SetAttribute(Attribute::NoReverse)
        )
        .map_err(|error| error.to_string())
    } else {
        execute!(stdout, MoveTo(x, y), Print(text)).map_err(|error| error.to_string())
    }
}

fn clip(text: &str, max_width: usize) -> String {
    let char_count = text.chars().count();
    if char_count <= max_width {
        return text.to_string();
    }
    if max_width <= 1 {
        return "…".to_string();
    }

    text.chars().take(max_width - 1).collect::<String>() + "…"
}
