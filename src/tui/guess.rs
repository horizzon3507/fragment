use std::time::{Duration, Instant};

use anyhow::Result;
use ratatui::Frame;
use ratatui::crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Gauge, Paragraph};

use crate::game::{GuessOutcome, LEVELS, Round};
use crate::player::Player;
use crate::tui::Tui;

/// Header strip above the play area (round n/N · score · streak).
#[derive(Debug, Clone, Copy)]
pub struct Header {
    pub round: usize,
    pub total: usize,
    pub score: u32,
    pub streak: u32,
}

/// How an interactive round ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoundEnd {
    /// Correct guess.
    Solved,
    /// Esc give-up.
    GaveUp,
    /// Final level consumed without a correct guess.
    Exhausted,
    /// Ctrl+C — abort the whole session.
    Quit,
}

/// Event-loop tick: caps the poll timeout so the timer bar stays smooth.
const TICK: Duration = Duration::from_millis(50);

/// Per-frame view state, kept separate from `Round` so `draw` is a pure
/// function of what the screen should show right now.
struct View<'a> {
    header: &'a Header,
    input: &'a str,
    level_start: Instant,
    status: Option<&'a str>,
    reveal: Option<Reveal>,
}

/// The frozen data behind the reveal screen.
#[derive(Debug, Clone, Copy)]
struct Reveal {
    end: RoundEnd,
    elapsed: Duration,
}

/// Run one interactive round: play the snippet, handle guessing,
/// escalation and the reveal screen. Returns when the round is over.
pub fn play_round(
    term: &mut Tui,
    round: &mut Round,
    player: &Player,
    header: &Header,
) -> Result<RoundEnd> {
    let mut input = String::new();
    let mut status = None;
    let mut reveal = None;
    let mut level_start = start_level(round, player, &mut status);

    loop {
        let level_len = Duration::from_secs(round.level_secs());
        let view = View {
            header,
            input: &input,
            level_start,
            status: status.as_deref(),
            reveal,
        };
        term.draw(|frame| draw(frame, round, &view))?;

        // The level timer ran out: same as pressing Tab.
        if reveal.is_none() && level_start.elapsed() >= level_len {
            if advance(round, player, &mut status, &mut level_start) {
                reveal = Some(Reveal {
                    end: RoundEnd::Exhausted,
                    elapsed: round.elapsed(),
                });
            }
            continue;
        }

        let timeout = if reveal.is_some() {
            TICK
        } else {
            level_len
                .saturating_sub(level_start.elapsed())
                .min(TICK)
        };
        if !event::poll(timeout)? {
            continue;
        }
        let key = match event::read()? {
            Event::Key(key) => key,
            // Pasted text counts as typed input, minus control chars.
            Event::Paste(s) => {
                input.extend(s.chars().filter(|c| !c.is_control()));
                continue;
            }
            _ => continue,
        };
        if key.kind != KeyEventKind::Press {
            continue;
        }

        // Ctrl+C quits from anywhere, including the reveal screen.
        if matches!(key.code, KeyCode::Char('c' | 'C'))
            && key.modifiers.contains(KeyModifiers::CONTROL)
        {
            let _ = player.stop();
            return Ok(RoundEnd::Quit);
        }

        // Any other press dismisses the reveal screen.
        if let Some(reveal) = reveal {
            let _ = player.stop();
            return Ok(reveal.end);
        }

        match (key.code, key.modifiers) {
            (KeyCode::Esc, _) => {
                reveal = Some(Reveal {
                    end: RoundEnd::GaveUp,
                    elapsed: round.elapsed(),
                });
            }
            // A blank Enter clears the prompt without spending the level.
            (KeyCode::Enter, _) if input.trim().is_empty() => input.clear(),
            (KeyCode::Enter, _) => match round.submit(&input) {
                GuessOutcome::Correct => {
                    reveal = Some(Reveal {
                        end: RoundEnd::Solved,
                        elapsed: round.elapsed(),
                    });
                }
                GuessOutcome::Wrong => {
                    input.clear();
                    if advance(round, player, &mut status, &mut level_start) {
                        reveal = Some(Reveal {
                            end: RoundEnd::Exhausted,
                            elapsed: round.elapsed(),
                        });
                    }
                }
            },
            (KeyCode::Tab, _) | (KeyCode::Char('+'), _)
                if advance(round, player, &mut status, &mut level_start) =>
            {
                reveal = Some(Reveal {
                    end: RoundEnd::Exhausted,
                    elapsed: round.elapsed(),
                });
            }
            // advance() already ran in the guard above; swallow the key.
            (KeyCode::Tab, _) | (KeyCode::Char('+'), _) => {}
            (KeyCode::Backspace, _) => {
                input.pop();
            }
            (KeyCode::Char(c), m) if !m.contains(KeyModifiers::CONTROL) => input.push(c),
            _ => {}
        }
    }
}

/// Play the snippet for the round's current level and restart the level
/// timer. Playback errors land on the status line and the round keeps
/// running.
fn start_level(round: &Round, player: &Player, status: &mut Option<String>) -> Instant {
    let result = player.play_snippet(
        round.track().path.as_path(),
        round.offset(),
        Duration::from_secs(round.level_secs()),
    );
    *status = result.err().map(|e| format!("playback: {e}"));
    Instant::now()
}

/// Consume the current level and start the next one. Returns true when the
/// final level was consumed and the round is exhausted.
fn advance(
    round: &mut Round,
    player: &Player,
    status: &mut Option<String>,
    level_start: &mut Instant,
) -> bool {
    round.escalate();
    if round.is_exhausted() {
        return true;
    }
    *level_start = start_level(round, player, status);
    false
}

fn draw(frame: &mut Frame, round: &Round, view: &View) {
    let [header_area, body, misses_area, status_area, input_area] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(5),
        Constraint::Length(LEVELS.len() as u16),
        Constraint::Length(1),
        Constraint::Length(1),
    ])
    .areas(frame.area());

    let header = Line::from(vec![
        Span::styled(
            format!("round {}/{}", view.header.round, view.header.total),
            Style::default().add_modifier(Modifier::BOLD),
        ),
        Span::raw(format!(
            "  ·  score {}  ·  streak {}",
            view.header.score, view.header.streak
        )),
    ]);
    frame.render_widget(
        Paragraph::new(header).alignment(Alignment::Center),
        header_area,
    );

    match view.reveal {
        Some(reveal) => draw_reveal(frame, round, body, reveal),
        None => draw_play(frame, round, body, view.level_start),
    }

    let misses: Vec<Line> = round
        .misses()
        .iter()
        .map(|m| {
            Line::from(vec![
                Span::styled("✗ ", Style::default().fg(Color::Red)),
                Span::raw(m.clone()),
            ])
        })
        .collect();
    frame.render_widget(Paragraph::new(misses), misses_area);

    let status = match view.status {
        Some(msg) => Line::from(Span::styled(
            format!(" {msg}"),
            Style::default().fg(Color::Red),
        )),
        None => Line::from(Span::styled(
            " enter: guess · tab/+: more time · esc: give up · ctrl+c: quit",
            Style::default().add_modifier(Modifier::DIM),
        )),
    };
    frame.render_widget(Paragraph::new(status), status_area);

    let mut input_spans = vec![
        Span::styled("> ", Style::default().add_modifier(Modifier::BOLD)),
        Span::raw(view.input.to_string()),
    ];
    if view.reveal.is_none() {
        input_spans.push(Span::styled("█", Style::default().add_modifier(Modifier::DIM)));
    }
    frame.render_widget(Paragraph::new(Line::from(input_spans)), input_area);
}

/// The escalation ladder and the current level's countdown bar.
fn draw_play(frame: &mut Frame, round: &Round, area: Rect, level_start: Instant) {
    let [_, ladder_area, _, gauge_area, _, _] = Layout::vertical([
        Constraint::Fill(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Fill(1),
    ])
    .areas(area);
    let [_, gauge_area, _] = Layout::horizontal([
        Constraint::Fill(1),
        Constraint::Percentage(60),
        Constraint::Fill(1),
    ])
    .areas(gauge_area);

    let mut spans = Vec::with_capacity(LEVELS.len() * 2);
    for (i, secs) in LEVELS.iter().enumerate() {
        if i > 0 {
            spans.push(Span::styled(
                " ▶ ",
                Style::default().add_modifier(Modifier::DIM),
            ));
        }
        let style = if i == round.level_idx() {
            Style::default().add_modifier(Modifier::BOLD | Modifier::REVERSED)
        } else if i < round.level_idx() {
            Style::default().add_modifier(Modifier::DIM)
        } else {
            Style::default()
        };
        spans.push(Span::styled(format!("{secs}s"), style));
    }
    frame.render_widget(
        Paragraph::new(Line::from(spans)).alignment(Alignment::Center),
        ladder_area,
    );

    let level_len = Duration::from_secs(round.level_secs());
    let remaining = level_len.saturating_sub(level_start.elapsed());
    let ratio = (remaining.as_secs_f64() / level_len.as_secs_f64()).clamp(0.0, 1.0);
    let gauge = Gauge::default()
        .use_unicode(true)
        .gauge_style(Style::default().fg(Color::Cyan))
        .ratio(ratio)
        .label(format!("{:.1}s", remaining.as_secs_f64()));
    frame.render_widget(gauge, gauge_area);
}

/// The answer, shown once the round is over until a key is pressed.
fn draw_reveal(frame: &mut Frame, round: &Round, area: Rect, reveal: Reveal) {
    let track = round.track();
    let mut lines = vec![
        Line::from(Span::styled(
            track.title.clone(),
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Line::default(),
    ];
    if let Some(artist) = &track.artist {
        lines.push(Line::from(Span::styled(
            artist.clone(),
            Style::default().add_modifier(Modifier::DIM),
        )));
    }
    lines.push(Line::from(format!(
        "level {}/{} · {}s snippet · {:.1}s elapsed",
        round.level_idx() + 1,
        LEVELS.len(),
        round.level_secs(),
        reveal.elapsed.as_secs_f64(),
    )));
    lines.push(Line::default());
    lines.push(Line::from(Span::styled(
        "press any key",
        Style::default().add_modifier(Modifier::DIM),
    )));

    let height = lines.len() as u16;
    let [_, reveal_area, _] = Layout::vertical([
        Constraint::Fill(1),
        Constraint::Length(height),
        Constraint::Fill(1),
    ])
    .areas(area);
    frame.render_widget(
        Paragraph::new(lines).alignment(Alignment::Center),
        reveal_area,
    );
}
