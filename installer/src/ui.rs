//! Rendering. Theme-agnostic: no hard-coded background (the terminal's own),
//! bold/reverse for emphasis, and the conventional green/yellow/red for status
//! so it reads on any color scheme.

use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, List, ListItem, Paragraph, Wrap},
};

use crate::app::{App, SdState, Step};
use crate::config::Field;
use crate::preflight::Status;

pub fn render(frame: &mut Frame, app: &App) {
    let [header, body, footer] = Layout::vertical([
        Constraint::Length(2),
        Constraint::Min(0),
        Constraint::Length(1),
    ])
    .areas(frame.area());

    render_header(frame, header);
    let [steps, main] =
        Layout::horizontal([Constraint::Length(22), Constraint::Min(0)]).areas(body);
    render_steps(frame, steps, app);
    render_main(frame, main, app);
    render_footer(frame, footer, app);
}

fn render_header(frame: &mut Frame, area: Rect) {
    let title = Line::from(vec![
        Span::styled("TYPOENA", Style::new().add_modifier(Modifier::BOLD)),
        Span::styled("  installer", Style::new().fg(Color::DarkGray)),
    ]);
    frame.render_widget(Paragraph::new(title), area);
}

fn render_steps(frame: &mut Frame, area: Rect, app: &App) {
    let items: Vec<ListItem> = Step::ALL
        .iter()
        .enumerate()
        .map(|(i, &s)| {
            let active = s == app.step;
            let marker = if active { "▸ " } else { "  " };
            let style = if active {
                Style::new().add_modifier(Modifier::BOLD | Modifier::REVERSED)
            } else {
                Style::new().fg(Color::DarkGray)
            };
            ListItem::new(Line::styled(
                format!("{marker}{}. {}", i + 1, s.title()),
                style,
            ))
        })
        .collect();
    frame.render_widget(
        List::new(items).block(Block::bordered().title(" steps ")),
        area,
    );
}

fn render_main(frame: &mut Frame, area: Rect, app: &App) {
    let block = Block::bordered().title(format!(" {} ", app.step.title()));
    match app.step {
        Step::Preflight => render_preflight(frame, area, app, block),
        Step::Configure => render_configure(frame, area, app, block),
        Step::SdCard => render_sdcard(frame, area, app, block),
        Step::Done => render_done(frame, area, block),
    }
}

fn render_preflight(frame: &mut Frame, area: Rect, app: &App, block: Block) {
    let mut lines = vec![
        Line::styled(
            "Checking your Mac and the card.",
            Style::new().fg(Color::DarkGray),
        ),
        Line::from(""),
    ];
    for c in &app.preflight.checks {
        let (glyph, color) = status_glyph(c.status);
        lines.push(Line::from(vec![
            Span::styled(
                format!(" {glyph} "),
                Style::new().fg(color).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("{:<16}", c.label),
                Style::new().add_modifier(Modifier::BOLD),
            ),
            Span::styled(c.detail.clone(), Style::new().fg(Color::DarkGray)),
        ]));
    }
    lines.push(Line::from(""));
    lines.push(if app.preflight.ready() {
        Line::styled(
            "Ready. Press Enter to continue.",
            Style::new().fg(Color::Green),
        )
    } else {
        Line::styled(
            "Fix the ✗ items, then press r to re-check.",
            Style::new().fg(Color::Yellow),
        )
    });
    frame.render_widget(paragraph(lines, block), area);
}

fn render_configure(frame: &mut Frame, area: Rect, app: &App, block: Block) {
    let mut lines: Vec<Line> = vec![
        Line::styled(
            "Pre-filled from this Mac where possible. Type to edit · ↑/↓ move · Enter next field.",
            Style::new().fg(Color::DarkGray),
        ),
        Line::from(""),
    ];

    for (i, &f) in Field::ALL.iter().enumerate() {
        let focused = i == app.focus;
        let val = app.config.get(f);
        let empty = val.trim().is_empty();
        let shown: String = if f.secret() && !empty {
            "•".repeat(val.chars().count())
        } else {
            val.to_string()
        };

        let mut spans = vec![
            Span::styled(
                if focused { "▸ " } else { "  " },
                Style::new().add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("{:<22}", f.label()),
                if focused {
                    Style::new().add_modifier(Modifier::BOLD)
                } else {
                    Style::new()
                },
            ),
        ];
        if focused {
            spans.push(Span::raw(shown));
            spans.push(Span::styled(
                " ",
                Style::new().add_modifier(Modifier::REVERSED),
            )); // block caret
        } else if empty {
            let (text, color) = if f.required() {
                ("(required)", Color::Yellow)
            } else {
                ("(optional)", Color::DarkGray)
            };
            spans.push(Span::styled(text, Style::new().fg(color)));
        } else {
            spans.push(Span::raw(shown));
        }
        lines.push(Line::from(spans));
    }

    lines.push(Line::from(""));
    if let Some(msg) = &app.status {
        lines.push(Line::styled(msg.clone(), Style::new().fg(Color::Cyan)));
    } else {
        let missing = app.config.missing_required();
        if missing.is_empty() {
            lines.push(Line::styled(
                "All required fields set — Enter on the last field goes to the SD-card step.",
                Style::new().fg(Color::Green),
            ));
        } else {
            let names: Vec<&str> = missing.iter().map(|f| f.label()).collect();
            lines.push(Line::styled(
                format!("Required still empty: {}", names.join(", ")),
                Style::new().fg(Color::Yellow),
            ));
        }
    }
    if let Some(hint) = field_hint(app.focused_field()) {
        lines.push(Line::styled(hint, Style::new().fg(Color::DarkGray)));
    }

    frame.render_widget(paragraph(lines, block), area);
}

fn render_sdcard(frame: &mut Frame, area: Rect, app: &App, block: Block) {
    let dim = |s: String| Line::styled(s, Style::new().fg(Color::DarkGray));
    let mut lines: Vec<Line> = Vec::new();
    match &app.sd {
        SdState::ConfirmWipe(info) => {
            let vol = app
                .cards
                .get(app.card_sel.min(app.cards.len().saturating_sub(1)))
                .map(|c| c.volume.display().to_string())
                .unwrap_or_default();
            lines.push(Line::styled(
                format!("⚠  {vol} already holds a Typoena card."),
                Style::new().fg(Color::Red).add_modifier(Modifier::BOLD),
            ));
            lines.push(Line::from(""));
            if let Some(o) = &info.origin {
                lines.push(dim(format!("  origin: {o}")));
            }
            if let Some(h) = &info.head {
                lines.push(dim(format!("  HEAD:   {h}")));
            }
            if info.dirty > 0 {
                lines.push(Line::styled(
                    format!(
                        "  {} unpublished edit(s) will be LOST (not yet published from the device).",
                        info.dirty
                    ),
                    Style::new().fg(Color::Red).add_modifier(Modifier::BOLD),
                ));
            }
            lines.push(Line::from(""));
            lines.push(Line::from(format!(
                "This ERASES repo/ (and the dirty journal) and re-clones {} onto the card.",
                app.config.remote_url
            )));
            lines.push(Line::from(""));
            lines.push(Line::styled(
                "Press y to wipe and continue · n to cancel.",
                Style::new().fg(Color::Yellow).add_modifier(Modifier::BOLD),
            ));
        }
        SdState::Idle => {
            if app.cards.is_empty() {
                lines.push(Line::styled(
                    "No removable card detected.",
                    Style::new().fg(Color::Yellow),
                ));
                lines.push(Line::from(""));
                lines.push(dim("Insert a FAT32 SD card, then press r to rescan.".into()));
            } else {
                lines.push(dim("Choose the card to write (↑/↓), then Enter:".into()));
                lines.push(Line::from(""));
                for (i, c) in app.cards.iter().enumerate() {
                    let sel = i == app.card_sel;
                    let marker = if sel { "▸ " } else { "  " };
                    let warn = if c.fat {
                        String::new()
                    } else {
                        "  (not FAT32 — the device may not mount it)".to_string()
                    };
                    let style = if sel {
                        Style::new().add_modifier(Modifier::BOLD | Modifier::REVERSED)
                    } else {
                        Style::new()
                    };
                    lines.push(Line::styled(
                        format!("{marker}{}  [{}]{}", c.name, c.fs, warn),
                        style,
                    ));
                }
                lines.push(Line::from(""));
                if app.config.missing_required().is_empty() {
                    lines.push(Line::styled(
                        "Enter writes the card: clone → seed config → typoena.conf → eject.",
                        Style::new().fg(Color::Green),
                    ));
                } else {
                    lines.push(Line::styled(
                        "Configure the required fields first (↑ to go back a step).",
                        Style::new().fg(Color::Yellow),
                    ));
                }
            }
            if let Some(msg) = &app.status {
                lines.push(Line::from(""));
                lines.push(Line::styled(msg.clone(), Style::new().fg(Color::Cyan)));
            }
        }
        rest => {
            lines.push(match rest {
                SdState::Running => Line::styled(
                    "Provisioning the card…  (Ctrl-C aborts)",
                    Style::new().fg(Color::Yellow),
                ),
                SdState::Failed(e) => {
                    Line::styled(format!("Failed: {e}"), Style::new().fg(Color::Red))
                }
                _ => Line::styled(
                    "Card ready ✓ — remove it and insert into Typoena.",
                    Style::new().fg(Color::Green),
                ),
            });
            lines.push(Line::from(""));
            let start = app.sd_log.len().saturating_sub(12);
            for l in &app.sd_log[start..] {
                lines.push(dim(l.clone()));
            }
        }
    }
    frame.render_widget(paragraph(lines, block), area);
}

fn render_done(frame: &mut Frame, area: Rect, block: Block) {
    let lines = vec![
        Line::styled(
            "All set.",
            Style::new().fg(Color::Green).add_modifier(Modifier::BOLD),
        ),
        Line::from(""),
        Line::from("Remove the card and insert it into your Typoena, then power on."),
        Line::styled(
            "Open lid → write → push. Nothing else runs on it.",
            Style::new().fg(Color::DarkGray),
        ),
    ];
    frame.render_widget(paragraph(lines, block), area);
}

fn field_hint(f: Field) -> Option<&'static str> {
    match f {
        Field::Pat => Some(
            "Fine-grained PAT, contents:write on the notes repo. Never derived; masked. ^U clears.",
        ),
        Field::WifiPass => {
            Some("^K fills this from your Keychain for the current SSID (may prompt macOS).")
        }
        Field::RemoteUrl => {
            Some("HTTPS URL of your notes repo, e.g. https://github.com/you/notes.git")
        }
        _ => None,
    }
}

fn status_glyph(s: Status) -> (&'static str, Color) {
    match s {
        Status::Ok => ("✓", Color::Green),
        Status::Warn => ("!", Color::Yellow),
        Status::Missing => ("✗", Color::Red),
    }
}

fn paragraph<'a>(lines: Vec<Line<'a>>, block: Block<'a>) -> Paragraph<'a> {
    Paragraph::new(Text::from(lines))
        .block(block)
        .wrap(Wrap { trim: false })
}

fn render_footer(frame: &mut Frame, area: Rect, app: &App) {
    let key = |k: &str| {
        Span::styled(
            format!(" {k} "),
            Style::new().add_modifier(Modifier::REVERSED),
        )
    };
    let lbl = |l: &'static str| Span::styled(l, Style::new().fg(Color::DarkGray));
    let sep = || Span::raw("  ");

    let spans = if matches!(app.sd, SdState::ConfirmWipe(_)) && app.step == Step::SdCard {
        vec![
            key("y"),
            lbl(" wipe & continue"),
            sep(),
            key("n"),
            lbl(" cancel"),
            sep(),
            key("^C"),
            lbl(" quit"),
        ]
    } else {
        match app.step {
            Step::Configure => vec![
                key("↑/↓"),
                lbl(" field"),
                sep(),
                key("Enter"),
                lbl(" next"),
                sep(),
                key("^K"),
                lbl(" wifi pw"),
                sep(),
                key("^U"),
                lbl(" clear"),
                sep(),
                key("Esc"),
                lbl(" quit"),
            ],
            Step::SdCard => vec![
                key("↑/↓"),
                lbl(" card"),
                sep(),
                key("r"),
                lbl(" rescan"),
                sep(),
                key("Enter"),
                lbl(" write"),
                sep(),
                key("q"),
                lbl(" quit"),
            ],
            _ => vec![
                key("↑/↓ Tab"),
                lbl(" step"),
                sep(),
                key("Enter"),
                lbl(" next"),
                sep(),
                key("r"),
                lbl(" re-check"),
                sep(),
                key("q"),
                lbl(" quit"),
            ],
        }
    };
    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}
