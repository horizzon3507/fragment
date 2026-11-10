use std::time::{Duration, Instant};

use anyhow::Result;
use ratatui::Frame;
use ratatui::crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Gauge, Padding, Paragraph};

use crate::game::{GuessOutcome, LEVELS, Round};
use crate::player::Player;
use crate::suggest::{Candidate, Candidates};
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
    suggestions: &'a [&'a Candidate],
    sel: Option<usize>,
}

/// The frozen data behind the reveal screen.
#[derive(Debug, Clone, Copy)]
struct Reveal {
    end: RoundEnd,
    elapsed: Duration,
}

/// Run one interactive round: play the snippet, handle guessing,
/// escalation and the reveal screen. `cands` feeds the title autocomplete
/// under the input. Returns how the round ended and the elapsed time
/// frozen when the reveal screen appeared, so dwell on that screen does
/// not leak into scoring.
pub fn play_round(
    term: &mut Tui,
    round: &mut Round,
    player: &Player,
    header: &Header,
    cands: &Candidates,
) -> Result<(RoundEnd, Duration)> {
    let mut input = String::new();
    let mut status = None;
    let mut reveal = None;
    // Highlighted row of the suggestion dropdown, if any.
    let mut sel: Option<usize> = None;
    let mut level_start = start_level(round, player, &mut status);

    loop {
        let level_len = Duration::from_secs(round.level_secs());
        // The dropdown hides on the reveal screen and on an empty input.
        let suggestions: Vec<&Candidate> = if reveal.is_some() {
            Vec::new()
        } else {
            cands.search(&input, 5)
        };
        // A stale highlight cannot outlive the list it points into.
        if sel.is_some_and(|s| s >= suggestions.len()) {
            sel = None;
        }
        let view = View {
            header,
            input: &input,
            level_start,
            status: status.as_deref(),
            reveal,
            suggestions: &suggestions,
            sel,
        };
        term.draw(|frame| draw(frame, round, &view))?;

        // The level timer ran out: same as pressing Tab.
        if reveal.is_none() && level_start.elapsed() >= level_len {
            if advance(round, player, &mut status, &mut level_start) {
                reveal = Some(enter_reveal(round, player, RoundEnd::Exhausted));
            }
            continue;
        }

        let timeout = if reveal.is_some() {
            TICK
        } else {
            level_len.saturating_sub(level_start.elapsed()).min(TICK)
        };
        if !event::poll(timeout)? {
            continue;
        }
        let key = match event::read()? {
            Event::Key(key) => key,
            // On the reveal screen a paste dismisses it like any key;
            // during play, pasted text counts as typed input minus
            // control chars.
            Event::Paste(s) => {
                if let Some(reveal) = reveal {
                    let _ = player.stop();
                    return Ok((reveal.end, reveal.elapsed));
                }
                input.extend(s.chars().filter(|c| !c.is_control()));
                sel = None;
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
            return Ok((RoundEnd::Quit, round.elapsed()));
        }

        // Any other press dismisses the reveal screen.
        if let Some(reveal) = reveal {
            let _ = player.stop();
            return Ok((reveal.end, reveal.elapsed));
        }

        match (key.code, key.modifiers) {
            // Esc clears a dropdown pick first; it gives up otherwise.
            (KeyCode::Esc, _) if sel.is_some() => sel = None,
            (KeyCode::Esc, _) => {
                reveal = Some(enter_reveal(round, player, RoundEnd::GaveUp));
            }
            (KeyCode::Up, _) if !suggestions.is_empty() => {
                sel = Some(sel.map_or(0, |s| s.saturating_sub(1)));
            }
            (KeyCode::Down, _) if !suggestions.is_empty() => {
                sel = Some(sel.map_or(0, |s| (s + 1).min(suggestions.len() - 1)));
            }
            // Right accepts the highlight, or the top row when nothing is
            // picked. Enter accepts only a picked row; otherwise it submits.
            (KeyCode::Right, _) => accept(&suggestions, &mut sel, &mut input),
            (KeyCode::Enter, _) if sel.is_some() => accept(&suggestions, &mut sel, &mut input),
            // A blank Enter clears the prompt without spending the level.
            (KeyCode::Enter, _) if input.trim().is_empty() => input.clear(),
            (KeyCode::Enter, _) => match round.submit(&input) {
                GuessOutcome::Correct => {
                    reveal = Some(enter_reveal(round, player, RoundEnd::Solved));
                }
                GuessOutcome::Wrong => {
                    input.clear();
                    if advance(round, player, &mut status, &mut level_start) {
                        reveal = Some(enter_reveal(round, player, RoundEnd::Exhausted));
                    }
                }
            },
            // Ctrl+R replays the current snippet from the same offset.
            (KeyCode::Char('r' | 'R'), m) if m.contains(KeyModifiers::CONTROL) => {
                level_start = start_level(round, player, &mut status);
            }
            (KeyCode::Tab, _) | (KeyCode::Char('+'), _)
                if advance(round, player, &mut status, &mut level_start) =>
            {
                reveal = Some(enter_reveal(round, player, RoundEnd::Exhausted));
            }
            // advance() already ran in the guard above; swallow the key.
            (KeyCode::Tab, _) | (KeyCode::Char('+'), _) => {}
            (KeyCode::Backspace, _) => {
                input.pop();
                sel = None;
            }
            (KeyCode::Char(c), m) if !m.contains(KeyModifiers::CONTROL) => {
                input.push(c);
                sel = None;
            }
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

/// Enter the reveal screen: stop the snippet and freeze the end state
/// and elapsed time.
fn enter_reveal(round: &Round, player: &Player, end: RoundEnd) -> Reveal {
    let _ = player.stop();
    Reveal {
        end,
        elapsed: round.elapsed(),
    }
}

/// Fill the input from a suggestion row: the highlighted one, or the top
/// row when nothing is highlighted. No-op on an empty list.
fn accept(suggestions: &[&Candidate], sel: &mut Option<usize>, input: &mut String) {
    if let Some(cand) = suggestions.get(sel.take().unwrap_or(0)) {
        *input = cand.fill().to_string();
    }
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
    let [
        header_area,
        body,
        misses_area,
        status_area,
        input_area,
        sugg_area,
    ] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(4),
        Constraint::Max(LEVELS.len() as u16),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Max(5),
    ])
    .areas(frame.area());

    let dim = Style::default().add_modifier(Modifier::DIM);
    let header = view.header;
    let streak_style = if header.streak >= 2 {
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default()
    };
    let header_line = Line::from(vec![
        Span::styled(
            format!("round {}/{}", header.round, header.total),
            Style::default().add_modifier(Modifier::BOLD),
        ),
        Span::styled(" · ", dim),
        Span::styled(
            format!("score {}", header.score),
            Style::default().add_modifier(Modifier::BOLD),
        ),
        Span::styled(" · ", dim),
        Span::styled(format!("streak {}", header.streak), streak_style),
    ]);
    frame.render_widget(
        Paragraph::new(header_line).alignment(Alignment::Center),
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
                Span::styled(
                    "✗ ",
                    Style::default().fg(Color::Red).add_modifier(Modifier::DIM),
                ),
                Span::styled(m.clone(), dim),
            ])
        })
        .collect();
    frame.render_widget(Paragraph::new(misses), misses_area);

    let status = match view.status {
        Some(msg) => Line::from(Span::styled(
            format!(" {msg}"),
            Style::default().fg(Color::Red),
        )),
        None => {
            let hint = if !view.suggestions.is_empty() {
                if view.sel.is_some() {
                    " ↑↓: move · →/enter: accept · esc: clear pick"
                } else {
                    " ↑↓: pick · →/enter: accept · esc: give up"
                }
            } else {
                " enter: guess · tab/+: more · ctrl+r: replay · esc: give up"
            };
            Line::from(Span::styled(hint, dim))
        }
    };
    frame.render_widget(Paragraph::new(status), status_area);

    let revealing = view.reveal.is_some();
    let (prompt_style, input_style) = if revealing {
        (dim, dim)
    } else {
        (
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
            Style::default(),
        )
    };
    let mut input_spans = vec![
        Span::styled(">", prompt_style),
        Span::styled(format!(" {}", view.input), input_style),
    ];
    if !revealing {
        input_spans.push(Span::styled("█", Style::default().fg(Color::Cyan)));
    }
    frame.render_widget(Paragraph::new(Line::from(input_spans)), input_area);

    if !view.suggestions.is_empty() {
        let accent = Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD);
        let rows: Vec<Line> = view
            .suggestions
            .iter()
            .enumerate()
            .map(|(i, cand)| {
                if view.sel == Some(i) {
                    Line::from(vec![
                        Span::styled(" ▸ ", accent),
                        Span::styled(cand.display.clone(), accent),
                    ])
                } else {
                    Line::from(vec![
                        Span::raw("   "),
                        Span::styled(cand.display.clone(), dim),
                    ])
                }
            })
            .collect();
        frame.render_widget(Paragraph::new(rows), sugg_area);
    }
}

/// The escalation ladder and the current level's countdown bar.
fn draw_play(frame: &mut Frame, round: &Round, area: Rect, level_start: Instant) {
    let [_, ladder_area, _, gauge_area, _] = Layout::vertical([
        Constraint::Fill(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Fill(1),
    ])
    .areas(area);
    let [_, gauge_area, _] = Layout::horizontal([
        Constraint::Fill(1),
        Constraint::Max(60),
        Constraint::Fill(1),
    ])
    .areas(gauge_area);

    // Step segments: filled block for consumed levels, arrow for the
    // current one, dot for pending.
    let idx = round.level_idx();
    let dim = Style::default().add_modifier(Modifier::DIM);
    let accent = Style::default()
        .fg(Color::Cyan)
        .add_modifier(Modifier::BOLD);
    let mut spans = Vec::with_capacity(LEVELS.len() * 3);
    for (i, secs) in LEVELS.iter().enumerate() {
        if i > 0 {
            spans.push(Span::raw("  "));
        }
        let (mark, mark_style, label_style) = if i < idx {
            ("█", dim, dim)
        } else if i == idx {
            ("▶", accent, accent)
        } else {
            ("·", dim, Style::default())
        };
        spans.push(Span::styled(mark, mark_style));
        spans.push(Span::styled(format!(" {secs}s"), label_style));
    }
    frame.render_widget(
        Paragraph::new(Line::from(spans)).alignment(Alignment::Center),
        ladder_area,
    );

    let level_len = Duration::from_secs(round.level_secs());
    let remaining = level_len.saturating_sub(level_start.elapsed());
    let ratio = (remaining.as_secs_f64() / level_len.as_secs_f64()).clamp(0.0, 1.0);
    let bar_color = if ratio > 0.5 {
        Color::Green
    } else if ratio > 0.25 {
        Color::Yellow
    } else {
        Color::Red
    };
    let gauge = Gauge::default()
        .style(Style::default().fg(Color::DarkGray))
        .use_unicode(true)
        .gauge_style(Style::default().fg(bar_color))
        .ratio(ratio)
        .label(format!("{:.1}s", remaining.as_secs_f64()));
    frame.render_widget(gauge, gauge_area);
}

/// The answer, shown once the round is over until a key is pressed.
fn draw_reveal(frame: &mut Frame, round: &Round, area: Rect, reveal: Reveal) {
    let track = round.track();
    let dim = Style::default().add_modifier(Modifier::DIM);

    let mut lines = vec![Line::from(Span::styled(
        track.title.clone(),
        Style::default().add_modifier(Modifier::BOLD),
    ))];
    if let Some(artist) = &track.artist {
        lines.push(Line::from(Span::styled(artist.clone(), dim)));
    }
    lines.push(Line::from(Span::styled(
        format!(
            "{}s · {:.1}s elapsed",
            round.level_secs(),
            reveal.elapsed.as_secs_f64()
        ),
        dim,
    )));
    lines.push(Line::default());
    lines.push(Line::from(Span::styled("press any key", dim)));

    let (mark, mark_style) = if reveal.end == RoundEnd::Solved {
        ("✓ ", Style::default().fg(Color::Green))
    } else {
        ("✗ ", Style::default().fg(Color::Red))
    };
    let title = Line::from(vec![
        Span::raw(" "),
        Span::styled(mark, mark_style),
        Span::styled("the answer ", dim),
    ])
    .centered();

    // Size the frame to its content, capped by the available area.
    let width = (lines.iter().map(Line::width).max().unwrap_or(0) as u16 + 6).max(24);
    let height = lines.len() as u16 + 2;
    let [_, frame_area, _] = Layout::vertical([
        Constraint::Fill(1),
        Constraint::Max(height),
        Constraint::Fill(1),
    ])
    .areas(area);
    let [_, frame_area, _] = Layout::horizontal([
        Constraint::Fill(1),
        Constraint::Max(width),
        Constraint::Fill(1),
    ])
    .areas(frame_area);

    let block = Block::bordered()
        .border_style(dim)
        .padding(Padding::horizontal(1))
        .title_top(title);
    frame.render_widget(
        Paragraph::new(lines)
            .alignment(Alignment::Center)
            .block(block),
        frame_area,
    );
}
