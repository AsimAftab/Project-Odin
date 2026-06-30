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
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, List, ListItem, Paragraph, Wrap},
    Frame, Terminal,
};

use crate::asgard::profile::{LayoutPreset, Profile, WindowLayout, WindowState};
use crate::asgard::state::RecentEntry;
use crate::ui::theme::{ACTIVE, BIFROST, HEIM_GOLD, RUNE_BLUE, SELECTION_BG, SHADOW};
use crate::utils::time;

/// What the user asked for when leaving the TUI. Caller dispatches.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AsgardAction {
    Activate(String),
    Deactivate,
    Create,
    Edit(String),
    Delete(String),
    Quit,
}

pub fn run(
    profiles: Vec<Profile>,
    active: Option<String>,
    recent: Vec<RecentEntry>,
) -> Result<AsgardAction> {
    enable_raw_mode()?;
    let stdout = std::io::stdout();
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    execute!(terminal.backend_mut(), EnterAlternateScreen)?;

    while crossterm::event::poll(Duration::from_millis(0))? {
        let _ = event::read()?;
    }

    let mut app = App::new(profiles, active, recent);
    let res = app_loop(&mut terminal, &mut app);

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    res.map(|()| app.action)
}

struct App {
    profiles: Vec<Profile>,
    active: Option<String>,
    recent: Vec<RecentEntry>,
    selected: usize,
    mode: Mode,
    action: AsgardAction,
    status: Option<String>,
    tick: u64,
}

enum Mode {
    List,
    ConfirmDelete,
    Help,
}

impl App {
    fn new(profiles: Vec<Profile>, active: Option<String>, recent: Vec<RecentEntry>) -> Self {
        Self {
            profiles,
            active,
            recent,
            selected: 0,
            mode: Mode::List,
            action: AsgardAction::Quit,
            status: None,
            tick: 0,
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
                if key.kind != KeyEventKind::Press {
                    continue;
                }
                match (&app.mode, key.code) {
                    (Mode::List, KeyCode::Char('q') | KeyCode::Char('Q') | KeyCode::Esc) => {
                        app.action = AsgardAction::Quit;
                        return Ok(());
                    }
                    (Mode::List, KeyCode::Up | KeyCode::Char('k')) if app.selected > 0 => {
                        app.selected -= 1;
                    }
                    (Mode::List, KeyCode::Down | KeyCode::Char('j'))
                        if app.selected + 1 < app.profiles.len() =>
                    {
                        app.selected += 1;
                    }
                    (Mode::List, KeyCode::Home | KeyCode::Char('g')) => app.selected = 0,
                    (Mode::List, KeyCode::End | KeyCode::Char('G')) if !app.profiles.is_empty() => {
                        app.selected = app.profiles.len() - 1;
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
                    (Mode::List, KeyCode::Char('x') | KeyCode::Char('X'))
                        if app.active.is_some() =>
                    {
                        app.action = AsgardAction::Deactivate;
                        return Ok(());
                    }
                    (Mode::List, KeyCode::Char('?') | KeyCode::F(1)) => {
                        app.mode = Mode::Help;
                    }
                    (Mode::Help, _) => {
                        app.mode = Mode::List;
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
                        app.status = Some("realm spared".into());
                    }
                    _ => {}
                }
            }
        }

        if last_tick.elapsed() >= tick_rate {
            last_tick = std::time::Instant::now();
            app.tick = app.tick.wrapping_add(1);
        }
    }
}

fn ui(f: &mut Frame, app: &App) {
    let area = f.area();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(9), // banner
            Constraint::Min(10),   // body
            Constraint::Length(3), // footer
        ])
        .split(area);

    draw_banner(f, chunks[0], app);
    draw_body(f, chunks[1], app);
    draw_footer(f, chunks[2], app);

    if matches!(app.mode, Mode::Help) {
        draw_help_overlay(f, area);
    }
}

fn draw_help_overlay(f: &mut Frame, area: Rect) {
    let popup = centered_rect(58, 60, area);
    f.render_widget(ratatui::widgets::Clear, popup);
    let lines = vec![
        Line::from(vec![Span::styled(
            "ᚨ  Asgard — keybindings",
            Style::default().fg(HEIM_GOLD).add_modifier(Modifier::BOLD),
        )]),
        Line::from(""),
        help_row("↑↓  /  j k", "navigate realms"),
        help_row("g  /  G", "first realm / last realm"),
        help_row("Enter", "bind the highlighted realm"),
        help_row("N", "forge a new realm (wizard)"),
        help_row("E", "edit the highlighted realm"),
        help_row("D", "dissolve the highlighted realm"),
        help_row("X", "unbind the active realm"),
        help_row("?  /  F1", "toggle this scroll"),
        help_row("Q  /  Esc", "leave Asgard"),
        Line::from(""),
        Line::from(vec![Span::styled(
            "press any key to close",
            Style::default().fg(SHADOW).add_modifier(Modifier::ITALIC),
        )]),
    ];
    f.render_widget(
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

fn draw_banner(f: &mut Frame, area: Rect, app: &App) {
    // Yggdrasil — the world tree. Glyph in the crown breathes a little.
    let bloom = match (app.tick / 3) % 4 {
        0 => '◇',
        1 => '◈',
        2 => '◇',
        _ => '·',
    };
    let crown = format!("       {bloom}    ╲│╱    {bloom}    ╲│╱    {bloom}");
    let active_line = match &app.active {
        Some(n) => Line::from(vec![
            Span::styled("       ● bound realm  ", Style::default().fg(SHADOW)),
            Span::styled(
                n.clone(),
                Style::default().fg(ACTIVE).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("    realms in Asgard: {}", app.profiles.len()),
                Style::default().fg(SHADOW).add_modifier(Modifier::ITALIC),
            ),
        ]),
        None => Line::from(vec![
            Span::styled("       ○ no realm bound", Style::default().fg(SHADOW)),
            Span::styled(
                format!("    realms in Asgard: {}", app.profiles.len()),
                Style::default().fg(SHADOW).add_modifier(Modifier::ITALIC),
            ),
        ]),
    };
    let lines = vec![
        Line::from(vec![Span::styled(
            crown,
            Style::default().fg(HEIM_GOLD).add_modifier(Modifier::BOLD),
        )]),
        Line::from(vec![Span::styled(
            "        ╲│╱    │    ╲│╱",
            Style::default().fg(HEIM_GOLD),
        )]),
        Line::from(vec![
            Span::styled(
                "         │     │     │     ",
                Style::default().fg(HEIM_GOLD),
            ),
            Span::styled(
                "ASGARD",
                Style::default().fg(HEIM_GOLD).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                "  —  developer profile realm",
                Style::default()
                    .fg(RUNE_BLUE)
                    .add_modifier(Modifier::ITALIC),
            ),
        ]),
        Line::from(vec![Span::styled(
            "       ═╧═════╧═════╧═       ",
            Style::default().fg(BIFROST),
        )]),
        Line::from(vec![Span::styled(
            "          ╲   │   ╱           Yggdrasil — the world tree",
            Style::default().fg(SHADOW).add_modifier(Modifier::ITALIC),
        )]),
        active_line,
    ];

    f.render_widget(
        Paragraph::new(lines).alignment(Alignment::Left).block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Double)
                .border_style(Style::default().fg(HEIM_GOLD))
                .title(Span::styled(
                    " ᚨ ASGARD ",
                    Style::default()
                        .fg(Color::Black)
                        .bg(HEIM_GOLD)
                        .add_modifier(Modifier::BOLD),
                )),
        ),
        area,
    );
}

fn draw_body(f: &mut Frame, area: Rect, app: &App) {
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
        .split(area);

    let left = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(6), Constraint::Length(7)])
        .split(cols[0]);

    draw_list(f, left[0], app);
    draw_recent(f, left[1], app);
    draw_detail(f, cols[1], app);
}

fn draw_recent(f: &mut Frame, area: Rect, app: &App) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(SHADOW))
        .title(Span::styled(
            " ⌘ recent bindings ",
            Style::default().fg(HEIM_GOLD).add_modifier(Modifier::BOLD),
        ));
    if app.recent.is_empty() {
        f.render_widget(
            Paragraph::new(Line::from(Span::styled(
                "no realm has been bound yet",
                Style::default().fg(SHADOW).add_modifier(Modifier::ITALIC),
            )))
            .alignment(Alignment::Center)
            .block(block),
            area,
        );
        return;
    }

    let items: Vec<ListItem> = app
        .recent
        .iter()
        .take(5)
        .map(|e| {
            let is_current = app.active.as_deref() == Some(e.name.as_str());
            ListItem::new(Line::from(vec![
                Span::styled(
                    if is_current { "● " } else { "· " },
                    Style::default().fg(if is_current { ACTIVE } else { SHADOW }),
                ),
                Span::styled(
                    e.name.clone(),
                    Style::default()
                        .fg(if is_current { ACTIVE } else { RUNE_BLUE })
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!("  {}", time::humanize_since(e.activated_at)),
                    Style::default().fg(SHADOW),
                ),
            ]))
        })
        .collect();

    f.render_widget(List::new(items).block(block), area);
}

fn draw_list(f: &mut Frame, area: Rect, app: &App) {
    let items: Vec<ListItem> = if app.profiles.is_empty() {
        vec![
            ListItem::new(Line::from(Span::styled(
                "no realms forged yet",
                Style::default().fg(Color::Yellow),
            ))),
            ListItem::new(""),
            ListItem::new(Line::from(Span::styled(
                "press [N] to forge your first one",
                Style::default().fg(SHADOW).add_modifier(Modifier::ITALIC),
            ))),
        ]
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
                        .bg(SELECTION_BG)
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                };
                let extras = format!(
                    "  apps:{} · urls:{}{}",
                    p.startup_apps.len(),
                    p.browser_urls.len(),
                    if p.vscode_workspace.is_some() {
                        " · vscode"
                    } else {
                        ""
                    }
                );
                let mut spans = vec![
                    Span::styled(
                        marker,
                        Style::default().fg(if is_active { ACTIVE } else { SHADOW }),
                    ),
                    Span::styled(
                        p.name.clone(),
                        Style::default()
                            .fg(if is_active { ACTIVE } else { RUNE_BLUE })
                            .add_modifier(Modifier::BOLD),
                    ),
                ];
                if !p.description.is_empty() {
                    spans.push(Span::styled("  — ", Style::default().fg(SHADOW)));
                    spans.push(Span::styled(
                        p.description.clone(),
                        Style::default().fg(Color::White),
                    ));
                }
                spans.push(Span::styled(extras, Style::default().fg(SHADOW)));
                ListItem::new(Line::from(spans)).style(style)
            })
            .collect()
    };

    f.render_widget(
        List::new(items).block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(RUNE_BLUE))
                .title(Span::styled(
                    " ᛞ realms ",
                    Style::default().fg(HEIM_GOLD).add_modifier(Modifier::BOLD),
                )),
        ),
        area,
    );
}

fn draw_detail(f: &mut Frame, area: Rect, app: &App) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(BIFROST))
        .title(Span::styled(
            " ◈ wisdom scroll ",
            Style::default().fg(HEIM_GOLD).add_modifier(Modifier::BOLD),
        ));

    let Some(profile) = app.profiles.get(app.selected) else {
        f.render_widget(
            Paragraph::new(Line::from(Span::styled(
                "the scroll lies blank — forge a realm to fill it",
                Style::default().fg(SHADOW).add_modifier(Modifier::ITALIC),
            )))
            .alignment(Alignment::Center)
            .block(block),
            area,
        );
        return;
    };

    let mut lines: Vec<Line> = Vec::new();
    let is_active = app.active.as_deref() == Some(profile.name.as_str());

    lines.push(Line::from(vec![
        Span::styled(
            if is_active { "● " } else { "○ " },
            Style::default().fg(if is_active { ACTIVE } else { SHADOW }),
        ),
        Span::styled(
            profile.name.clone(),
            Style::default().fg(HEIM_GOLD).add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            if is_active { "  (bound)" } else { "" },
            Style::default().fg(ACTIVE),
        ),
    ]));
    if !profile.description.is_empty() {
        lines.push(Line::from(vec![Span::styled(
            profile.description.clone(),
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::ITALIC),
        )]));
    }
    lines.push(Line::from(""));

    // env vars section
    section_header(&mut lines, "ᚱ", "runes (env vars)", profile.env.len());
    if profile.env.is_empty() {
        lines.push(empty_line());
    } else {
        for (k, v) in &profile.env {
            lines.push(Line::from(vec![
                Span::styled("  ", Style::default()),
                Span::styled(
                    k.clone(),
                    Style::default().fg(RUNE_BLUE).add_modifier(Modifier::BOLD),
                ),
                Span::styled(" = ", Style::default().fg(SHADOW)),
                Span::styled(truncate(v, 38), Style::default().fg(Color::White)),
            ]));
        }
    }
    lines.push(Line::from(""));

    // startup apps
    section_header(
        &mut lines,
        "⚒",
        "warriors (startup apps)",
        profile.startup_apps.len(),
    );
    if profile.startup_apps.is_empty() {
        lines.push(empty_line());
    } else {
        for (i, a) in profile.startup_apps.iter().enumerate() {
            let mark = match a.window {
                WindowState::Normal => "",
                WindowState::Minimized => "  [min]",
                WindowState::Maximized => "  [max]",
            };
            lines.push(Line::from(vec![
                Span::styled(format!(" {:>2}. ", i + 1), Style::default().fg(SHADOW)),
                Span::styled(
                    a.name.clone(),
                    Style::default().fg(RUNE_BLUE).add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!(
                        "  {}{}",
                        truncate(&a.command, 30),
                        if !a.args.is_empty() {
                            format!(" {}", a.args.join(" "))
                        } else {
                            String::new()
                        }
                    ),
                    Style::default().fg(Color::White),
                ),
                Span::styled(mark, Style::default().fg(SHADOW)),
            ]));
            // Show layout configuration (snap position + monitor) if defined
            if let Some(layout) = &a.layout {
                lines.push(Line::from(vec![
                    Span::styled("       ", Style::default()),
                    Span::styled("⊞ ", Style::default().fg(BIFROST)),
                    Span::styled(
                        format_layout(layout),
                        Style::default().fg(BIFROST).add_modifier(Modifier::ITALIC),
                    ),
                ]));
            }
        }
    }
    lines.push(Line::from(""));

    // browser URLs
    section_header(
        &mut lines,
        "⌒",
        "ravens (browser urls)",
        profile.browser_urls.len(),
    );
    if profile.browser_urls.is_empty() {
        lines.push(empty_line());
    } else {
        for (i, u) in profile.browser_urls.iter().enumerate() {
            lines.push(Line::from(vec![
                Span::styled(format!(" {:>2}. ", i + 1), Style::default().fg(SHADOW)),
                Span::styled(
                    u.name.clone(),
                    Style::default().fg(BIFROST).add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!("  {}", truncate(&u.url, 40)),
                    Style::default().fg(SHADOW),
                ),
            ]));
        }
    }
    lines.push(Line::from(""));

    // vscode workspace
    match &profile.vscode_workspace {
        Some(ws) => {
            section_header(&mut lines, "◇", "VS Code workspace", 1);
            lines.push(Line::from(vec![
                Span::styled("  ", Style::default()),
                Span::styled(truncate(ws, 50), Style::default().fg(Color::White)),
            ]));
        }
        None => {
            section_header(&mut lines, "◇", "VS Code workspace", 0);
            lines.push(empty_line());
        }
    }

    f.render_widget(
        Paragraph::new(lines)
            .wrap(Wrap { trim: false })
            .block(block),
        area,
    );
}

fn section_header(
    lines: &mut Vec<Line<'static>>,
    glyph: &'static str,
    label: &'static str,
    count: usize,
) {
    lines.push(Line::from(vec![
        Span::styled(
            format!(" {} ", glyph),
            Style::default().fg(HEIM_GOLD).add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            label,
            Style::default().fg(HEIM_GOLD).add_modifier(Modifier::BOLD),
        ),
        Span::styled(format!("  ({count})"), Style::default().fg(SHADOW)),
    ]));
}

fn empty_line() -> Line<'static> {
    Line::from(vec![Span::styled(
        "    (none)",
        Style::default().fg(SHADOW).add_modifier(Modifier::ITALIC),
    )])
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let mut out: String = s.chars().take(max.saturating_sub(1)).collect();
        out.push('…');
        out
    }
}

fn format_layout(layout: &WindowLayout) -> String {
    match layout {
        WindowLayout::Preset(preset) => {
            format!("{} · Monitor 1", format_preset(preset))
        }
        WindowLayout::TargetedPreset { preset, monitor } => {
            format!("{} · Monitor {}", format_preset(preset), monitor)
        }
        WindowLayout::Bounds {
            x,
            y,
            width,
            height,
        } => format!("Bounds {},{} {}×{}", x, y, width, height),
    }
}

fn format_preset(preset: &LayoutPreset) -> String {
    match preset {
        LayoutPreset::SnapLeft => "Snap Left".into(),
        LayoutPreset::SnapRight => "Snap Right".into(),
        LayoutPreset::TopHalf => "Top Half".into(),
        LayoutPreset::BottomHalf => "Bottom Half".into(),
        LayoutPreset::Quadrant1 => "Quadrant 1 (Top Right)".into(),
        LayoutPreset::Quadrant2 => "Quadrant 2 (Top Left)".into(),
        LayoutPreset::Quadrant3 => "Quadrant 3 (Bottom Left)".into(),
        LayoutPreset::Quadrant4 => "Quadrant 4 (Bottom Right)".into(),
    }
}

fn draw_footer(f: &mut Frame, area: Rect, app: &App) {
    let chips: Vec<Span> = match app.mode {
        Mode::List => {
            let mut base = vec![
                key_chip("↑↓", "navigate"),
                Span::raw("  "),
                key_chip("Enter", "bind"),
                Span::raw("  "),
                key_chip("N", "new"),
                Span::raw("  "),
                key_chip("E", "edit"),
                Span::raw("  "),
                key_chip("D", "delete"),
            ];
            if app.active.is_some() {
                base.push(Span::raw("  "));
                base.push(key_chip("X", "unbind"));
            }
            base.push(Span::raw("  "));
            base.push(key_chip("?", "help"));
            base.push(Span::raw("  "));
            base.push(key_chip("Q", "leave"));
            match &app.status {
                Some(msg) => {
                    let mut v = vec![
                        Span::styled(
                            format!(" {msg} "),
                            Style::default()
                                .fg(HEIM_GOLD)
                                .add_modifier(Modifier::ITALIC),
                        ),
                        Span::raw("  "),
                    ];
                    v.extend(base);
                    v
                }
                None => base,
            }
        }
        Mode::ConfirmDelete => {
            let name = app
                .profiles
                .get(app.selected)
                .map(|p| p.name.clone())
                .unwrap_or_default();
            vec![
                Span::styled(
                    format!("dissolve realm `{name}`?  "),
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ),
                key_chip("Y", "confirm"),
                Span::raw("  "),
                key_chip("N/Esc", "spare"),
            ]
        }
        Mode::Help => vec![
            Span::styled(
                "scroll of bindings — ",
                Style::default()
                    .fg(HEIM_GOLD)
                    .add_modifier(Modifier::ITALIC),
            ),
            key_chip("any key", "close"),
        ],
    };

    f.render_widget(
        Paragraph::new(Line::from(chips))
            .alignment(Alignment::Center)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .border_style(Style::default().fg(SHADOW)),
            ),
        area,
    );
}

fn key_chip(key: &'static str, label: &'static str) -> Span<'static> {
    Span::styled(
        format!("[{key}] {label}"),
        Style::default().fg(RUNE_BLUE).add_modifier(Modifier::BOLD),
    )
}
