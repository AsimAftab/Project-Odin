use std::io::Stdout;
use std::time::Duration;

use anyhow::Result;
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
    Terminal,
};

use crate::asgard::profile::ProfileSummary;

/// What the user asked for when leaving the TUI. Caller dispatches.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AsgardAction {
    Activate(String),
    Create,
    Edit(String),
    Delete(String),
    Quit,
}

pub fn run(profiles: Vec<ProfileSummary>, active: Option<String>) -> Result<AsgardAction> {
    enable_raw_mode()?;
    let stdout = std::io::stdout();
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    execute!(terminal.backend_mut(), EnterAlternateScreen)?;

    // Drain any keystrokes already in the buffer (e.g. the Enter the user
    // pressed to run the command) so we don't react to them on entry.
    while crossterm::event::poll(Duration::from_millis(0))? {
        let _ = event::read()?;
    }

    let mut app = App::new(profiles, active);
    let res = app_loop(&mut terminal, &mut app);

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    res.map(|()| app.action)
}

struct App {
    profiles: Vec<ProfileSummary>,
    active: Option<String>,
    selected: usize,
    mode: Mode,
    action: AsgardAction,
    status: Option<String>,
}

enum Mode {
    List,
    ConfirmDelete,
}

impl App {
    fn new(profiles: Vec<ProfileSummary>, active: Option<String>) -> Self {
        Self {
            profiles,
            active,
            selected: 0,
            mode: Mode::List,
            action: AsgardAction::Quit,
            status: None,
        }
    }
}

fn app_loop(terminal: &mut Terminal<CrosstermBackend<Stdout>>, app: &mut App) -> Result<()> {
    let tick_rate = Duration::from_millis(500);
    let mut last_tick = std::time::Instant::now();

    loop {
        terminal.draw(|f| ui(f, app))?;

        let timeout = tick_rate.saturating_sub(last_tick.elapsed());
        if crossterm::event::poll(timeout)? {
            if let Event::Key(key) = event::read()? {
                // On Windows, crossterm fires both Press and Release; ignore Release.
                if key.kind != KeyEventKind::Press {
                    continue;
                }
                match (&app.mode, key.code) {
                    (Mode::List, KeyCode::Char('q') | KeyCode::Char('Q') | KeyCode::Esc) => {
                        app.action = AsgardAction::Quit;
                        return Ok(());
                    }
                    (Mode::List, KeyCode::Up) if app.selected > 0 => app.selected -= 1,
                    (Mode::List, KeyCode::Down) if app.selected + 1 < app.profiles.len() => {
                        app.selected += 1;
                    }
                    (Mode::List, KeyCode::Enter) if !app.profiles.is_empty() => {
                        app.action =
                            AsgardAction::Activate(app.profiles[app.selected].name.clone());
                        return Ok(());
                    }
                    (Mode::List, KeyCode::Char('n') | KeyCode::Char('N')) => {
                        app.action = AsgardAction::Create;
                        return Ok(());
                    }
                    (Mode::List, KeyCode::Char('e') | KeyCode::Char('E'))
                        if !app.profiles.is_empty() =>
                    {
                        app.action = AsgardAction::Edit(app.profiles[app.selected].name.clone());
                        return Ok(());
                    }
                    (Mode::List, KeyCode::Char('d') | KeyCode::Char('D'))
                        if !app.profiles.is_empty() =>
                    {
                        app.mode = Mode::ConfirmDelete;
                    }
                    (Mode::ConfirmDelete, KeyCode::Char('y') | KeyCode::Char('Y')) => {
                        app.action = AsgardAction::Delete(app.profiles[app.selected].name.clone());
                        return Ok(());
                    }
                    (
                        Mode::ConfirmDelete,
                        KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc,
                    ) => {
                        app.mode = Mode::List;
                        app.status = Some("delete cancelled".into());
                    }
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
                Constraint::Min(6),
                Constraint::Length(2),
            ]
            .as_ref(),
        )
        .split(f.area());

    let header_lines = vec![
        Line::from(vec![Span::styled(
            "ODIN ASGARD",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from(vec![Span::styled(
            "Developer Profile Realm",
            Style::default().fg(Color::DarkGray),
        )]),
        Line::from(match &app.active {
            Some(n) => vec![
                Span::styled("active: ", Style::default().fg(Color::DarkGray)),
                Span::styled(n.clone(), Style::default().fg(Color::Green)),
            ],
            None => vec![Span::styled(
                "active: none",
                Style::default().fg(Color::DarkGray),
            )],
        }),
    ];
    let header = Paragraph::new(header_lines).block(Block::default().borders(Borders::BOTTOM));
    f.render_widget(header, chunks[0]);

    let items: Vec<ListItem> = if app.profiles.is_empty() {
        vec![ListItem::new(Line::from(Span::styled(
            "no profiles yet — press N to create one",
            Style::default().fg(Color::Yellow),
        )))]
    } else {
        app.profiles
            .iter()
            .enumerate()
            .map(|(idx, p)| {
                let selected = idx == app.selected;
                let is_active = app.active.as_deref() == Some(p.name.as_str());
                let marker = if is_active { "● " } else { "  " };
                let style = if selected {
                    Style::default()
                        .bg(Color::DarkGray)
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                };
                let extras = format!(
                    "  apps:{}  urls:{}{}",
                    p.startup_app_count,
                    p.browser_url_count,
                    if p.has_vscode { "  vscode" } else { "" }
                );
                let mut spans = vec![
                    Span::styled(
                        marker,
                        Style::default().fg(if is_active {
                            Color::Green
                        } else {
                            Color::DarkGray
                        }),
                    ),
                    Span::styled(p.name.clone(), Style::default().fg(Color::Cyan)),
                ];
                if !p.description.is_empty() {
                    spans.push(Span::raw(" — "));
                    spans.push(Span::styled(
                        p.description.clone(),
                        Style::default().fg(Color::White),
                    ));
                }
                spans.push(Span::styled(extras, Style::default().fg(Color::DarkGray)));
                ListItem::new(Line::from(spans)).style(style)
            })
            .collect()
    };

    let list = List::new(items).block(Block::default().borders(Borders::ALL).title("Profiles"));
    f.render_widget(list, chunks[1]);

    let footer_text = match app.mode {
        Mode::List => match &app.status {
            Some(msg) => {
                format!("{msg}    Enter: activate · N: new · E: edit · D: delete · Q: quit")
            }
            None => "Enter: activate · N: new · E: edit · D: delete · Q: quit".to_string(),
        },
        Mode::ConfirmDelete => format!(
            "delete `{}`? Y: confirm · N/Esc: cancel",
            app.profiles
                .get(app.selected)
                .map(|p| p.name.clone())
                .unwrap_or_default()
        ),
    };
    let footer = Paragraph::new(footer_text)
        .style(Style::default().fg(match app.mode {
            Mode::List => Color::Gray,
            Mode::ConfirmDelete => Color::Yellow,
        }))
        .alignment(Alignment::Center);
    f.render_widget(footer, chunks[2]);
}
