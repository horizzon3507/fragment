use anyhow::Result;
use ratatui::Frame;
use ratatui::crossterm::event::{self, Event, KeyEventKind};
use ratatui::layout::{Alignment, Constraint, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::game::LEVELS;
use crate::session::{RoundResult, Session};
use crate::tui::Tui;

/// Render the final scoreboard (round history, totals, best streak) and
/// wait for a keypress.
pub fn show(term: &mut Tui, session: &Session) -> Result<()> {
    loop {
        term.draw(|frame| draw(frame, session))?;
        // Any keypress dismisses the board; other events just redraw.
        if let Event::Key(key) = event::read()?
            && key.kind == KeyEventKind::Press
        {
            return Ok(());
        }
    }
}

fn draw(frame: &mut Frame, session: &Session) {
    // title, blank, history rows, blank, totals, blank, footer
    const FIXED_ROWS: u16 = 6;
    let area = frame.area();
    let visible = usize::from(area.height.saturating_sub(FIXED_ROWS));

    let mut lines =
        Vec::with_capacity(session.results().len().min(visible) + FIXED_ROWS as usize);
    lines.push(Line::from(Span::styled(
        "session over",
        Style::default().add_modifier(Modifier::BOLD),
    )));
    lines.push(Line::default());
    // Keep the most recent rounds when the terminal is short.
    lines.extend(
        session
            .results()
            .iter()
            .skip(session.results().len().saturating_sub(visible))
            .map(round_line),
    );
    lines.push(Line::default());
    lines.push(totals_line(session));
    lines.push(Line::default());
    lines.push(Line::from(Span::styled(
        "press any key to exit",
        Style::default().add_modifier(Modifier::DIM),
    )));

    let [_, board, _] = Layout::vertical([
        Constraint::Fill(1),
        Constraint::Length((lines.len() as u16).min(area.height)),
        Constraint::Fill(1),
    ])
    .areas(area);
    frame.render_widget(
        Paragraph::new(lines).alignment(Alignment::Center),
        board,
    );
}

/// One history row: marker, "title — artist", snippet, elapsed, points.
fn round_line(result: &RoundResult) -> Line<'static> {
    let track = result.track();
    let name = match &track.artist {
        Some(artist) => format!("{} — {artist}", track.title),
        None => track.title.clone(),
    };
    let snippet = LEVELS[result.level_idx().min(LEVELS.len() - 1)];
    let (mark, mark_style, score_style) = if result.solved() {
        (
            "✓ ",
            Style::default().fg(Color::Green),
            Style::default().add_modifier(Modifier::BOLD),
        )
    } else {
        (
            "✗ ",
            Style::default().fg(Color::Red),
            Style::default().add_modifier(Modifier::DIM),
        )
    };
    Line::from(vec![
        Span::styled(mark, mark_style),
        Span::raw(name),
        Span::styled(
            format!(" · {snippet}s · {:.1}s · ", result.elapsed().as_secs_f64()),
            Style::default().add_modifier(Modifier::DIM),
        ),
        Span::styled(format!("+{}", result.score()), score_style),
    ])
}

/// "score N · won X/Y · best streak Z", bolding the score like the header.
fn totals_line(session: &Session) -> Line<'static> {
    let solved = session.results().iter().filter(|r| r.solved()).count();
    Line::from(vec![
        Span::styled(
            format!("score {}", session.total_score()),
            Style::default().add_modifier(Modifier::BOLD),
        ),
        Span::raw(format!(
            "  ·  won {}/{}  ·  best streak {}",
            solved,
            session.rounds_played(),
            session.best_streak()
        )),
    ])
}
