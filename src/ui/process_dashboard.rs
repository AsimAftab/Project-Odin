use anyhow::Result;
use crossterm::{
    event::{self, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Table, Row},
    Terminal,
};
use std::io::Stdout;
use std::time::Duration;

use crate::models::process::ProcessInfo;
use crate::services::process_service::ProcessService;

pub async fn run() -> Result<()> {
    // Setup terminal
    enable_raw_mode()?;
    let stdout = std::io::stdout();
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    execute!(terminal.backend_mut(), EnterAlternateScreen)?;

    // Create app state
    let mut app = App::new();

    // Run the app
    let res = app_loop(&mut terminal, &mut app).await;

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen
    )?;
    terminal.show_cursor()?;

    res
}

struct App {
    processes: Vec<ProcessInfo>,
    selected_index: usize,
    sort_column: SortColumn,
    descending: bool,
    filter: String,
    total_processes: u32,
}

#[derive(Clone, Copy, PartialEq)]
enum SortColumn {
    PID,
    Name,
    Memory,
    CPU,
}

impl App {
    fn new() -> Self {
        Self {
            processes: Vec::new(),
            selected_index: 0,
            sort_column: SortColumn::Memory,
            descending: true,
            filter: String::new(),
            total_processes: 0,
        }
    }

    async fn update(&mut self) -> Result<()> {
        let processes = ProcessService::get_all_processes().await?;
        let stats = ProcessService::get_process_stats().await?;

        // Filter processes
        let filtered: Vec<ProcessInfo> = processes
            .into_iter()
            .filter(|p| {
                p.name.to_lowercase().contains(self.filter.to_lowercase().as_str())
            })
            .collect();

        // Sort processes
        let mut sorted = filtered;
        match self.sort_column {
            SortColumn::PID => sorted.sort_by_key(|p| p.pid),
            SortColumn::Name => sorted.sort_by(|a, b| a.name.cmp(&b.name)),
            SortColumn::Memory => sorted.sort_by(|a, b| {
                b.memory_mb.partial_cmp(&a.memory_mb).unwrap_or(std::cmp::Ordering::Equal)
            }),
            SortColumn::CPU => sorted.sort_by(|a, b| {
                b.cpu_percent.partial_cmp(&a.cpu_percent).unwrap_or(std::cmp::Ordering::Equal)
            }),
        }

        if !self.descending {
            sorted.reverse();
        }

        self.processes = sorted;
        self.total_processes = stats.total_processes;

        if self.selected_index >= self.processes.len() && !self.processes.is_empty() {
            self.selected_index = self.processes.len() - 1;
        }

        Ok(())
    }
}

async fn app_loop(
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    app: &mut App,
) -> Result<()> {
    let tick_rate = Duration::from_millis(500);
    let mut last_tick = std::time::Instant::now();

    loop {
        app.update().await?;
        terminal.draw(|f| ui(f, app))?;

        let timeout = tick_rate.saturating_sub(last_tick.elapsed());
        if crossterm::event::poll(timeout)? {
            if let Event::Key(key) = event::read()? {
                match key.code {
                    KeyCode::Char('q') | KeyCode::Esc => return Ok(()),
                    KeyCode::Up => {
                        if app.selected_index > 0 {
                            app.selected_index -= 1;
                        }
                    }
                    KeyCode::Down => {
                        if app.selected_index < app.processes.len().saturating_sub(1) {
                            app.selected_index += 1;
                        }
                    }
                    KeyCode::Char('k') | KeyCode::Char('K') => {
                        if !app.processes.is_empty() {
                            let proc = &app.processes[app.selected_index];
                            let _ = ProcessService::kill_process(proc.pid).await;
                        }
                    }
                    KeyCode::Char('1') => app.sort_column = SortColumn::PID,
                    KeyCode::Char('2') => app.sort_column = SortColumn::Name,
                    KeyCode::Char('3') => app.sort_column = SortColumn::Memory,
                    KeyCode::Char('4') => app.sort_column = SortColumn::CPU,
                    KeyCode::Char('r') => app.descending = !app.descending,
                    _ => {}
                }
            }
        }

        if last_tick.elapsed() >= tick_rate {
            last_tick = std::time::Instant::now();
        }
    }
}

fn ui(f: &mut ratatui::Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints(
            [
                Constraint::Length(3),
                Constraint::Min(10),
                Constraint::Length(2),
            ]
            .as_ref(),
        )
        .split(f.area());

    // Header
    let header_text = vec![
        Line::from(vec![
            Span::styled("ODIN Process Dashboard", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        ]),
        Line::from(vec![
            Span::raw(format!("Total Processes: {}", app.total_processes)),
        ]),
    ];
    let header = Paragraph::new(header_text)
        .block(Block::default().borders(Borders::BOTTOM));
    f.render_widget(header, chunks[0]);

    // Process table
    let rows: Vec<Row> = app
        .processes
        .iter()
        .enumerate()
        .map(|(idx, proc)| {
            let style = if idx == app.selected_index {
                Style::default().bg(Color::DarkGray).fg(Color::White)
            } else {
                Style::default()
            };

            Row::new(vec![
                format!("{}", proc.pid),
                proc.name.clone(),
                format!("{:.1} MB", proc.memory_mb),
                format!("{:.1}%", proc.cpu_percent),
            ])
            .style(style)
        })
        .collect();

    let table = Table::new(
        rows,
        [
            Constraint::Length(10),
            Constraint::Min(20),
            Constraint::Length(12),
            Constraint::Length(8),
        ],
    )
    .header(
        Row::new(vec!["PID", "NAME", "MEMORY", "CPU"])
            .style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))
    )
    .block(Block::default().borders(Borders::ALL).title("Processes"));

    f.render_widget(table, chunks[1]);

    // Footer
    let help_text = "↑↓: Navigate | K: Kill | 1-4: Sort | R: Reverse | Q: Quit";
    let footer = Paragraph::new(help_text)
        .style(Style::default().fg(Color::Gray))
        .alignment(Alignment::Center);
    f.render_widget(footer, chunks[2]);
}
