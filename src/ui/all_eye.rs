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
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{
        Block, BorderType, Borders, Cell, Gauge, List, ListItem, Paragraph, Row, Table, Wrap,
    },
    Frame, Terminal,
};

use crate::models::{
    git::GitConfigSnapshot, machine::MachineSnapshot, package::PackageSnapshot,
    vscode::VsCodeExtensionsSnapshot,
};
use crate::ui::theme::{BIFROST, HEIM_GOLD, RUNE_BLUE, SHADOW};

#[derive(Debug, Clone)]
pub enum HealthStatus {
    Ok(String),
    Warn(String),
    Bad(String),
}

#[derive(Debug, Clone)]
pub struct HealthCheck {
    pub label: String,
    pub status: HealthStatus,
}

pub struct AllEyeData {
    pub snapshot_dir: String,
    pub github_repo: Option<String>,
    pub sync_branch: String,
    pub machine: Option<MachineSnapshot>,
    pub packages: Option<PackageSnapshot>,
    pub vscode: Option<VsCodeExtensionsSnapshot>,
    pub git: Option<GitConfigSnapshot>,
    pub health: Vec<HealthCheck>,
}

pub fn run(data: AllEyeData) -> Result<()> {
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
    data: &AllEyeData,
) -> Result<()> {
    let mut tick: u64 = 0;
    let mut show_help = false;
    loop {
        terminal.draw(|frame| draw(frame, data, tick, show_help))?;

        if event::poll(Duration::from_millis(250))? {
            if let Event::Key(key) = event::read()? {
                match key.code {
                    KeyCode::Char('q') | KeyCode::Esc => break,
                    KeyCode::Char('?') | KeyCode::F(1) => show_help = !show_help,
                    _ if show_help => show_help = false,
                    _ => {}
                }
            }
        }
        tick = tick.wrapping_add(1);
    }
    Ok(())
}

fn draw(frame: &mut Frame, data: &AllEyeData, tick: u64, show_help: bool) {
    let area = frame.area();
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(7),
            Constraint::Length(8),
            Constraint::Min(12),
            Constraint::Length(3),
        ])
        .split(area);

    draw_banner(frame, vertical[0], tick);
    draw_identity(frame, vertical[1], data);
    draw_body(frame, vertical[2], data);
    draw_footer(frame, vertical[3]);

    if show_help {
        draw_help_overlay(frame, area);
    }
}

fn draw_help_overlay(frame: &mut Frame, area: Rect) {
    let popup = centered_rect(56, 50, area);
    frame.render_widget(ratatui::widgets::Clear, popup);
    let lines = vec![
        Line::from(vec![Span::styled(
            "ᚢ  All-Eye — keybindings",
            Style::default().fg(HEIM_GOLD).add_modifier(Modifier::BOLD),
        )]),
        Line::from(""),
        help_row("?  /  F1", "toggle this scroll"),
        help_row("Q  /  Esc", "leave the high seat"),
        Line::from(""),
        Line::from(vec![Span::styled(
            "  the All-Eye is a read-only view; the realm itself is",
            Style::default().fg(SHADOW),
        )]),
        Line::from(vec![Span::styled(
            "  shaped by `odin snapshot`, `odin restore`, and `odin sync`.",
            Style::default().fg(SHADOW),
        )]),
        Line::from(""),
        Line::from(vec![Span::styled(
            "press any key to close",
            Style::default().fg(SHADOW).add_modifier(Modifier::ITALIC),
        )]),
    ];
    frame.render_widget(
        Paragraph::new(lines).alignment(Alignment::Left).block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Double)
                .border_style(Style::default().fg(HEIM_GOLD))
                .title(Span::styled(
                    " ◈ scroll of bindings ",
                    Style::default()
                        .fg(Color::Black)
                        .bg(HEIM_GOLD)
                        .add_modifier(Modifier::BOLD),
                )),
        ),
        popup,
    );
}

fn help_row(key: &'static str, label: &'static str) -> Line<'static> {
    Line::from(vec![
        Span::raw("  "),
        Span::styled(
            format!("{:>14}", key),
            Style::default().fg(RUNE_BLUE).add_modifier(Modifier::BOLD),
        ),
        Span::raw("   "),
        Span::styled(label, Style::default().fg(Color::White)),
    ])
}

fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(area);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

fn draw_banner(frame: &mut Frame, area: Rect, tick: u64) {
    // Animated eye: blink slowly, with rare wink.
    let eye_phase = (tick / 4) % 16;
    let eye = match eye_phase {
        14 => "◔",
        15 => "◐",
        _ => "◉",
    };

    // Ravens — Hugin (thought) and Munin (memory) — toggle wing position.
    let raven_left = match (tick / 6) % 2 {
        0 => "ᕈ",
        _ => "ᔓ",
    };
    let raven_right = match (tick / 6) % 2 {
        0 => "ᕇ",
        _ => "ᔕ",
    };

    let lines = vec![
        Line::from(vec![
            Span::styled(format!("    {raven_left}  "), Style::default().fg(SHADOW)),
            Span::styled(
                "ᚨ  ᛚ  ᛚ  —  ᛖ  ᛃ  ᛖ",
                Style::default().fg(HEIM_GOLD).add_modifier(Modifier::BOLD),
            ),
            Span::styled(format!("  {raven_right}"), Style::default().fg(SHADOW)),
        ]),
        Line::from(vec![
            Span::styled("       ╭───╮  ", Style::default().fg(HEIM_GOLD)),
            Span::styled(
                format!(" {eye} "),
                Style::default().fg(HEIM_GOLD).add_modifier(Modifier::BOLD),
            ),
            Span::styled("  ╭───╮", Style::default().fg(HEIM_GOLD)),
        ]),
        Line::from(vec![
            Span::styled("       ╰───╯  ", Style::default().fg(HEIM_GOLD)),
            Span::styled(
                "the gaze of Odin",
                Style::default()
                    .fg(RUNE_BLUE)
                    .add_modifier(Modifier::ITALIC),
            ),
            Span::styled("  ╰───╯", Style::default().fg(HEIM_GOLD)),
        ]),
        Line::from(""),
        Line::from(vec![Span::styled(
            "from Hliðskjálf, all nine realms are seen.",
            Style::default().fg(SHADOW).add_modifier(Modifier::ITALIC),
        )]),
    ];

    let banner = Paragraph::new(lines).alignment(Alignment::Center).block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Double)
            .border_style(Style::default().fg(HEIM_GOLD))
            .title(Span::styled(
                " ALL-EYE ",
                Style::default()
                    .fg(Color::Black)
                    .bg(HEIM_GOLD)
                    .add_modifier(Modifier::BOLD),
            )),
    );
    frame.render_widget(banner, area);
}

fn draw_identity(frame: &mut Frame, area: Rect, data: &AllEyeData) {
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(55), Constraint::Percentage(45)])
        .split(area);

    let mut realm_lines: Vec<Line> = Vec::new();
    realm_lines.push(Line::from(vec![
        Span::styled("vault   ", Style::default().fg(SHADOW)),
        Span::styled(data.snapshot_dir.clone(), Style::default().fg(RUNE_BLUE)),
    ]));
    if let Some(machine) = &data.machine {
        realm_lines.push(Line::from(vec![
            Span::styled("realm   ", Style::default().fg(SHADOW)),
            Span::styled(
                format!("{} ({})", machine.hostname, machine.username),
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
        ]));
        realm_lines.push(Line::from(vec![
            Span::styled("seen    ", Style::default().fg(SHADOW)),
            Span::styled(
                machine.captured_at.to_rfc3339(),
                Style::default().fg(Color::White),
            ),
        ]));
        realm_lines.push(Line::from(vec![
            Span::styled("rune    ", Style::default().fg(SHADOW)),
            Span::styled(
                machine.snapshot_id.to_string(),
                Style::default().fg(BIFROST),
            ),
        ]));
        realm_lines.push(Line::from(vec![
            Span::styled("forge   ", Style::default().fg(SHADOW)),
            Span::styled(
                format!(
                    "{} cores · {:.1} GB · {}",
                    machine.cpu_count,
                    machine.total_memory_bytes as f64 / 1_073_741_824.0,
                    machine.os_name
                ),
                Style::default().fg(Color::White),
            ),
        ]));
    } else {
        realm_lines.push(Line::from(vec![Span::styled(
            "no snapshot — run `odin snapshot` to capture this realm",
            Style::default().fg(Color::Yellow),
        )]));
    }

    frame.render_widget(
        Paragraph::new(realm_lines).wrap(Wrap { trim: true }).block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(RUNE_BLUE))
                .title(Span::styled(
                    " ◉ Hliðskjálf · the high seat ",
                    Style::default().fg(HEIM_GOLD).add_modifier(Modifier::BOLD),
                )),
        ),
        cols[0],
    );

    let bifrost_lines = vec![
        Line::from(vec![
            Span::styled("repo    ", Style::default().fg(SHADOW)),
            match &data.github_repo {
                Some(r) => Span::styled(r.clone(), Style::default().fg(BIFROST)),
                None => Span::styled("not configured", Style::default().fg(Color::Yellow)),
            },
        ]),
        Line::from(vec![
            Span::styled("branch  ", Style::default().fg(SHADOW)),
            Span::styled(data.sync_branch.clone(), Style::default().fg(Color::White)),
        ]),
        Line::from(vec![
            Span::styled("hoard   ", Style::default().fg(SHADOW)),
            Span::styled(
                format!(
                    "{} pkg · {} ext · {} git",
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
                        .unwrap_or_default(),
                ),
                Style::default().fg(Color::White),
            ),
        ]),
    ];

    frame.render_widget(
        Paragraph::new(bifrost_lines)
            .wrap(Wrap { trim: true })
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .border_style(Style::default().fg(BIFROST))
                    .title(Span::styled(
                        " ⌒ Bifrost · the rainbow bridge ",
                        Style::default().fg(HEIM_GOLD).add_modifier(Modifier::BOLD),
                    )),
            ),
        cols[1],
    );
}

fn draw_body(frame: &mut Frame, area: Rect, data: &AllEyeData) {
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(35),
            Constraint::Percentage(35),
            Constraint::Percentage(30),
        ])
        .split(area);

    let manager_rows = data
        .machine
        .as_ref()
        .map(|machine| {
            machine
                .package_managers
                .iter()
                .map(|m| {
                    let (status, color) = if m.installed {
                        ("✓ ready", Color::Green)
                    } else {
                        ("· dormant", Color::DarkGray)
                    };
                    Row::new(vec![
                        Cell::from(Span::styled(
                            m.name.clone(),
                            Style::default()
                                .fg(Color::White)
                                .add_modifier(Modifier::BOLD),
                        )),
                        Cell::from(Span::styled(status, Style::default().fg(color))),
                        Cell::from(Span::styled(
                            m.version.clone().unwrap_or_else(|| "—".to_string()),
                            Style::default().fg(SHADOW),
                        )),
                    ])
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    frame.render_widget(
        Table::new(
            manager_rows,
            [
                Constraint::Length(12),
                Constraint::Length(12),
                Constraint::Min(8),
            ],
        )
        .header(
            Row::new(vec!["forge", "state", "rune"])
                .style(Style::default().fg(HEIM_GOLD).add_modifier(Modifier::BOLD)),
        )
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(SHADOW))
                .title(Span::styled(
                    " ⚒ forges · package managers ",
                    Style::default().fg(HEIM_GOLD),
                )),
        ),
        cols[0],
    );

    let tool_rows = data
        .machine
        .as_ref()
        .map(|machine| {
            machine
                .developer_tools
                .iter()
                .take(20)
                .map(|tool| {
                    Row::new(vec![
                        Cell::from(Span::styled(
                            tool.name.clone(),
                            Style::default().fg(Color::White),
                        )),
                        Cell::from(Span::styled(
                            tool.version.clone().unwrap_or_else(|| "—".to_string()),
                            Style::default().fg(SHADOW),
                        )),
                    ])
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    frame.render_widget(
        Table::new(tool_rows, [Constraint::Length(18), Constraint::Min(10)])
            .header(
                Row::new(vec!["tool", "rune"])
                    .style(Style::default().fg(HEIM_GOLD).add_modifier(Modifier::BOLD)),
            )
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .border_style(Style::default().fg(SHADOW))
                    .title(Span::styled(
                        " ◈ ravens · developer tools ",
                        Style::default().fg(HEIM_GOLD),
                    )),
            ),
        cols[1],
    );

    let inner = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(4), Constraint::Length(3)])
        .split(cols[2]);

    let health_items: Vec<ListItem> = data
        .health
        .iter()
        .map(|hc| {
            let (icon, fg) = match &hc.status {
                HealthStatus::Ok(_) => ("✓ ", Color::Green),
                HealthStatus::Warn(_) => ("! ", Color::Yellow),
                HealthStatus::Bad(_) => ("✗ ", Color::Red),
            };
            let detail = match &hc.status {
                HealthStatus::Ok(s) | HealthStatus::Warn(s) | HealthStatus::Bad(s) => s.clone(),
            };
            ListItem::new(Line::from(vec![
                Span::styled(icon, Style::default().fg(fg).add_modifier(Modifier::BOLD)),
                Span::styled(
                    format!("{:18}", hc.label),
                    Style::default().fg(Color::White),
                ),
                Span::styled(detail, Style::default().fg(fg)),
            ]))
        })
        .collect();
    frame.render_widget(
        List::new(health_items).block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(SHADOW))
                .title(Span::styled(
                    " ◐ Hugin & Munin · observers ",
                    Style::default().fg(HEIM_GOLD),
                )),
        ),
        inner[0],
    );

    let (ready, total) = data
        .machine
        .as_ref()
        .map(|m| {
            let total = m.package_managers.len();
            let ready = m.package_managers.iter().filter(|p| p.installed).count();
            (ready, total)
        })
        .unwrap_or((0, 0));
    let pct = if total > 0 {
        ((ready as f64 / total as f64) * 100.0) as u16
    } else {
        0
    };
    let label = if total > 0 {
        format!("{ready}/{total} forges lit")
    } else {
        "no realm in vault".to_string()
    };
    frame.render_widget(
        Gauge::default()
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .border_style(Style::default().fg(SHADOW))
                    .title(Span::styled(
                        " ◯ mead-hall · forges ready ",
                        Style::default().fg(HEIM_GOLD),
                    )),
            )
            .gauge_style(Style::default().fg(BIFROST).bg(Color::Black))
            .percent(pct)
            .label(label),
        inner[1],
    );
}

fn draw_footer(frame: &mut Frame, area: Rect) {
    let line = Line::from(vec![
        Span::styled(
            "q / Esc",
            Style::default().fg(HEIM_GOLD).add_modifier(Modifier::BOLD),
        ),
        Span::styled("  leave high seat   ·   ", Style::default().fg(SHADOW)),
        Span::styled(
            "?",
            Style::default().fg(HEIM_GOLD).add_modifier(Modifier::BOLD),
        ),
        Span::styled("  scroll   ·   ", Style::default().fg(SHADOW)),
        Span::styled("odin snapshot", Style::default().fg(RUNE_BLUE)),
        Span::styled(" capture   ·   ", Style::default().fg(SHADOW)),
        Span::styled("odin sync", Style::default().fg(BIFROST)),
        Span::styled(" cross Bifrost", Style::default().fg(SHADOW)),
    ]);
    frame.render_widget(
        Paragraph::new(line).alignment(Alignment::Center).block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(SHADOW)),
        ),
        area,
    );
}
