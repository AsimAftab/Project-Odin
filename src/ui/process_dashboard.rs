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
    widgets::{Block, BorderType, Borders, Paragraph, Row, Table},
    Terminal,
};
use std::io::Stdout;
use std::time::Duration;

use crate::models::process::ProcessInfo;
use crate::services::process_service::ProcessService;
use crate::ui::theme::{BIFROST, HEIM_GOLD, RUNE_BLUE, SELECTION_BG, SHADOW};

/// Render a small bar like ▆▆▄▁▁▁ proportional to `frac` ∈ [0, 1].
fn bar(frac: f64, width: usize) -> String {
    const BLOCKS: [char; 9] = [' ', '▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];
    let f = frac.clamp(0.0, 1.0);
    let total_steps = (width * 8) as f64;
    let mut filled = (f * total_steps).round() as usize;
    let mut out = String::with_capacity(width);
    for _ in 0..width {
        let take = filled.min(8);
        out.push(BLOCKS[take]);
        filled = filled.saturating_sub(take);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::bar;

    #[test]
    fn bar_zero_is_blank() {
        let b = bar(0.0, 4);
        assert_eq!(b.chars().count(), 4);
        assert!(b.chars().all(|c| c == ' '));
    }

    #[test]
    fn bar_one_is_full() {
        let b = bar(1.0, 4);
        assert_eq!(b.chars().count(), 4);
        assert!(b.chars().all(|c| c == '█'));
    }

    #[test]
    fn bar_clamps_out_of_range() {
        // negative and over-1.0 must not panic, and must produce `width` chars.
        assert_eq!(bar(-0.5, 6).chars().count(), 6);
        assert_eq!(bar(2.0, 6).chars().count(), 6);
    }

    #[test]
    fn bar_half_is_partial() {
        let b = bar(0.5, 4);
        // exactly half -> 16 of 32 steps -> first 2 cells full, next 2 empty
        assert_eq!(b.chars().count(), 4);
    }
}

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
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
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
#[allow(clippy::upper_case_acronyms)]
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
                p.name
                    .to_lowercase()
                    .contains(self.filter.to_lowercase().as_str())
            })
            .collect();

        // Sort processes
        let mut sorted = filtered;
        match self.sort_column {
            SortColumn::PID => sorted.sort_by_key(|p| p.pid),
            SortColumn::Name => sorted.sort_by(|a, b| a.name.cmp(&b.name)),
            SortColumn::Memory => sorted.sort_by(|a, b| {
                b.memory_mb
                    .partial_cmp(&a.memory_mb)
                    .unwrap_or(std::cmp::Ordering::Equal)
            }),
            SortColumn::CPU => sorted.sort_by(|a, b| {
                b.cpu_percent
                    .partial_cmp(&a.cpu_percent)
                    .unwrap_or(std::cmp::Ordering::Equal)
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

async fn app_loop(terminal: &mut Terminal<CrosstermBackend<Stdout>>, app: &mut App) -> Result<()> {
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
                    KeyCode::Up if app.selected_index > 0 => {
                        app.selected_index -= 1;
                    }
                    KeyCode::Down if app.selected_index < app.processes.len().saturating_sub(1) => {
                        app.selected_index += 1;
                    }
                    KeyCode::Char('k') | KeyCode::Char('K') if !app.processes.is_empty() => {
                        let proc = &app.processes[app.selected_index];
                        let _ = ProcessService::kill_process(proc.pid).await;
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
                Constraint::Length(4),
                Constraint::Min(10),
                Constraint::Length(3),
            ]
            .as_ref(),
        )
        .split(f.area());

    let sort_label = match app.sort_column {
        SortColumn::PID => "rune",
        SortColumn::Name => "name",
        SortColumn::Memory => "weight",
        SortColumn::CPU => "fury",
    };
    let arrow = if app.descending { "▼" } else { "▲" };
    let header_text = vec![
        Line::from(vec![
            Span::styled(
                "ᛉ  HOST OF EINHERJAR",
                Style::default().fg(HEIM_GOLD).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                "  ·  warriors of the host",
                Style::default().fg(SHADOW).add_modifier(Modifier::ITALIC),
            ),
        ]),
        Line::from(vec![
            Span::styled("warriors  ", Style::default().fg(SHADOW)),
            Span::styled(
                app.total_processes.to_string(),
                Style::default().fg(RUNE_BLUE).add_modifier(Modifier::BOLD),
            ),
            Span::styled("    sort by  ", Style::default().fg(SHADOW)),
            Span::styled(
                format!("{sort_label} {arrow}"),
                Style::default().fg(BIFROST).add_modifier(Modifier::BOLD),
            ),
            if app.filter.is_empty() {
                Span::raw("")
            } else {
                Span::styled(
                    format!("    filter  {}", app.filter),
                    Style::default().fg(HEIM_GOLD),
                )
            },
        ]),
    ];
    let header = Paragraph::new(header_text).block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Double)
            .border_style(Style::default().fg(HEIM_GOLD)),
    );
    f.render_widget(header, chunks[0]);

    // Pre-compute the heaviest warrior so the memory bar scales sensibly.
    let max_mem = app
        .processes
        .iter()
        .map(|p| p.memory_mb as f64)
        .fold(0.0_f64, f64::max)
        .max(64.0);

    let rows: Vec<Row> = app
        .processes
        .iter()
        .enumerate()
        .map(|(idx, proc)| {
            let mem_color = if proc.memory_mb > 2048.0 {
                Color::Red
            } else if proc.memory_mb > 512.0 {
                Color::Yellow
            } else {
                Color::Green
            };
            let cpu_color = if proc.cpu_percent > 50.0 {
                Color::Red
            } else if proc.cpu_percent > 10.0 {
                Color::Yellow
            } else {
                Color::Green
            };

            let row_style = if idx == app.selected_index {
                Style::default()
                    .bg(SELECTION_BG)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };

            let mem_bar = bar(proc.memory_mb as f64 / max_mem, 6);
            let cpu_bar = bar(proc.cpu_percent as f64 / 100.0, 6);

            Row::new(vec![
                ratatui::widgets::Cell::from(Span::styled(
                    format!("{}", proc.pid),
                    Style::default().fg(SHADOW),
                )),
                ratatui::widgets::Cell::from(Span::styled(
                    proc.name.clone(),
                    Style::default().fg(Color::White),
                )),
                ratatui::widgets::Cell::from(Span::styled(
                    format!("{} {:>7.1} MB", mem_bar, proc.memory_mb),
                    Style::default().fg(mem_color),
                )),
                ratatui::widgets::Cell::from(Span::styled(
                    format!("{} {:>5.1}%", cpu_bar, proc.cpu_percent),
                    Style::default().fg(cpu_color),
                )),
            ])
            .style(row_style)
        })
        .collect();

    let table = Table::new(
        rows,
        [
            Constraint::Length(10),
            Constraint::Min(20),
            Constraint::Length(20),
            Constraint::Length(15),
        ],
    )
    .header(
        Row::new(vec!["rune", "name", "weight", "fury"])
            .style(Style::default().fg(HEIM_GOLD).add_modifier(Modifier::BOLD)),
    )
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(SHADOW))
            .title(Span::styled(
                " ⚔ warriors ",
                Style::default().fg(HEIM_GOLD).add_modifier(Modifier::BOLD),
            )),
    );

    f.render_widget(table, chunks[1]);

    let chips = Line::from(vec![
        Span::styled(
            "[↑↓]",
            Style::default().fg(RUNE_BLUE).add_modifier(Modifier::BOLD),
        ),
        Span::styled(" navigate  ·  ", Style::default().fg(SHADOW)),
        Span::styled(
            "[K]",
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        ),
        Span::styled(" fell  ·  ", Style::default().fg(SHADOW)),
        Span::styled(
            "[1-4]",
            Style::default().fg(BIFROST).add_modifier(Modifier::BOLD),
        ),
        Span::styled(" sort  ·  ", Style::default().fg(SHADOW)),
        Span::styled(
            "[R]",
            Style::default().fg(BIFROST).add_modifier(Modifier::BOLD),
        ),
        Span::styled(" reverse  ·  ", Style::default().fg(SHADOW)),
        Span::styled(
            "[Q]",
            Style::default().fg(HEIM_GOLD).add_modifier(Modifier::BOLD),
        ),
        Span::styled(" leave", Style::default().fg(SHADOW)),
    ]);
    let footer = Paragraph::new(chips).alignment(Alignment::Center).block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(SHADOW)),
    );
    f.render_widget(footer, chunks[2]);
}
