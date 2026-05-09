use std::io;
use std::time::Duration;

use anyhow::Result;
use crossterm::{
    event::{self, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph, Row, Table},
    Terminal,
};

use crate::models::{
    git::GitConfigSnapshot, machine::MachineSnapshot, package::PackageSnapshot,
    vscode::VsCodeExtensionsSnapshot,
};

pub struct DashboardData {
    pub snapshot_dir: String,
    pub github_repo: Option<String>,
    pub sync_branch: String,
    pub machine: Option<MachineSnapshot>,
    pub packages: Option<PackageSnapshot>,
    pub vscode: Option<VsCodeExtensionsSnapshot>,
    pub git: Option<GitConfigSnapshot>,
    pub health: Vec<String>,
}

pub fn run(data: DashboardData) -> Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    terminal.clear()?;

    let result = run_loop(&mut terminal, &data);

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    result
}

fn run_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    data: &DashboardData,
) -> Result<()> {
    loop {
        terminal.draw(|frame| {
            let area = frame.area();
            let vertical = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(3),
                    Constraint::Length(8),
                    Constraint::Min(10),
                    Constraint::Length(3),
                ])
                .split(area);

            let title = Paragraph::new(Line::from(vec![
                Span::styled(" Odin ", Style::default().fg(Color::Black).bg(Color::Cyan).add_modifier(Modifier::BOLD)),
                Span::raw(" developer environment manager"),
            ]))
            .block(Block::default().borders(Borders::ALL));
            frame.render_widget(title, vertical[0]);

            let summary_chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
                .split(vertical[1]);

            let summary = summary_lines(data);
            frame.render_widget(
                Paragraph::new(summary).block(Block::default().title("Snapshot").borders(Borders::ALL)),
                summary_chunks[0],
            );

            let health_items = data
                .health
                .iter()
                .map(|item| ListItem::new(item.as_str()))
                .collect::<Vec<_>>();
            frame.render_widget(
                List::new(health_items).block(Block::default().title("Health").borders(Borders::ALL)),
                summary_chunks[1],
            );

            let body_chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(45), Constraint::Percentage(55)])
                .split(vertical[2]);

            let manager_rows = data
                .machine
                .as_ref()
                .map(|machine| {
                    machine
                        .package_managers
                        .iter()
                        .map(|manager| {
                            Row::new(vec![
                                manager.name.clone(),
                                if manager.installed { "installed".to_string() } else { "missing".to_string() },
                                manager.version.clone().unwrap_or_else(|| "-".to_string()),
                            ])
                        })
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            let manager_table = Table::new(
                manager_rows,
                [
                    Constraint::Length(12),
                    Constraint::Length(12),
                    Constraint::Min(10),
                ],
            )
            .header(Row::new(vec!["manager", "status", "version"]).style(Style::default().fg(Color::Yellow)))
            .block(Block::default().title("Package Managers").borders(Borders::ALL));
            frame.render_widget(manager_table, body_chunks[0]);

            let tool_rows = data
                .machine
                .as_ref()
                .map(|machine| {
                    machine
                        .developer_tools
                        .iter()
                        .take(12)
                        .map(|tool| {
                            Row::new(vec![
                                tool.name.clone(),
                                tool.version.clone().unwrap_or_else(|| "-".to_string()),
                            ])
                        })
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            let tool_table = Table::new(
                tool_rows,
                [Constraint::Length(18), Constraint::Min(20)],
            )
            .header(Row::new(vec!["tool", "version"]).style(Style::default().fg(Color::Yellow)))
            .block(Block::default().title("Developer Tools").borders(Borders::ALL));
            frame.render_widget(tool_table, body_chunks[1]);

            frame.render_widget(
                Paragraph::new("q quit  |  snapshot: odin snapshot  |  restore: odin restore  |  sync: odin sync")
                    .block(Block::default().borders(Borders::ALL)),
                vertical[3],
            );
        })?;

        if event::poll(Duration::from_millis(250))? {
            if let Event::Key(key) = event::read()? {
                if matches!(key.code, KeyCode::Char('q') | KeyCode::Esc) {
                    break;
                }
            }
        }
    }
    Ok(())
}

fn summary_lines(data: &DashboardData) -> Vec<Line<'static>> {
    let mut lines = vec![Line::from(format!("dir: {}", data.snapshot_dir))];
    if let Some(machine) = &data.machine {
        lines.push(Line::from(format!(
            "host: {} ({})",
            machine.hostname, machine.username
        )));
        lines.push(Line::from(format!("captured: {}", machine.captured_at)));
        lines.push(Line::from(format!("snapshot: {}", machine.snapshot_id)));
    } else {
        lines.push(Line::from("snapshot: missing"));
    }
    lines.push(Line::from(format!(
        "github: {}",
        data.github_repo
            .clone()
            .unwrap_or_else(|| "not configured".to_string())
    )));
    lines.push(Line::from(format!("branch: {}", data.sync_branch)));
    lines.push(Line::from(format!(
        "packages: {}  vscode: {}  git config: {}",
        data.packages
            .as_ref()
            .map(|p| p.packages.len())
            .unwrap_or_default(),
        data.vscode
            .as_ref()
            .map(|v| v.extensions.len())
            .unwrap_or_default(),
        data.git
            .as_ref()
            .map(|g| g.entries.len())
            .unwrap_or_default()
    )));
    lines
}
