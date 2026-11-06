use anyhow::Result;
use ratatui::Frame;
use ratatui::crossterm::event::{self, Event, KeyEventKind};
use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Bar, BarChart, BarGroup, Block, Paragraph, Sparkline};

use crate::game::LEVELS;
use crate::session::{RoundResult, Session};
use crate::stats::Stats;
use crate::tui::Tui;

/// Render the final scoreboard (round history, charts, totals, best
/// streak) and wait for a keypress. `stats` already includes this
/// session's results.
pub fn show(term: &mut Tui, session: &Session, stats: &Stats) -> Result<()> {
    loop {
        term.draw(|frame| draw(frame, session, stats))?;
        // Any keypress dismisses the board; other events just redraw.
        if let Event::Key(key) = event::read()?
            && key.kind == KeyEventKind::Press
        {
            return Ok(());
        }
    }
}

fn draw(frame: &mut Frame, session: &Session, stats: &Stats) {
    let area = frame.area();
    let dim = Style::default().add_modifier(Modifier::DIM);

    // The charts need ~9 rows to read; the sparkline ~3. Below that the
    // board falls back to the plain list so history is never squeezed.
    let charts_h = if area.height >= 27 { 9 } else { 0 };
    let spark_h = if area.height >= 31 { 3 } else { 0 };
    // title, blank, totals, blank, stats line, blank, footer.
    let fixed = 7 + charts_h + spark_h;
    let hist_rows = usize::from(area.height.saturating_sub(fixed)).clamp(1, 10);

    let constraints = [
        Constraint::Length(1),                    // title
        Constraint::Length(hist_rows as u16 + 1), // history + blank
        Constraint::Length(1),                    // totals
        Constraint::Length(1),                    // blank
        Constraint::Length(charts_h),             // bar charts
        Constraint::Length(spark_h),              // sparkline
        Constraint::Length(1),                    // stats line
        Constraint::Length(1),                    // blank
        Constraint::Length(1),                    // footer
    ];
    let used: u16 = constraints
        .iter()
        .map(|c| match c {
            Constraint::Length(n) => *n,
            _ => 0,
        })
        .sum();
    let [_, content, _] = Layout::vertical([
        Constraint::Fill(1),
        Constraint::Length(used.min(area.height)),
        Constraint::Fill(1),
    ])
    .areas(area);
    let c = Layout::vertical(constraints).split(content);

    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            "session over",
            Style::default().add_modifier(Modifier::BOLD),
        )))
        .alignment(Alignment::Center),
        c[0],
    );

    // Keep the most recent rounds when the terminal is short.
    let history: Vec<Line> = session
        .results()
        .iter()
        .skip(session.results().len().saturating_sub(hist_rows))
        .map(round_line)
        .collect();
    frame.render_widget(Paragraph::new(history).alignment(Alignment::Center), c[1]);

    frame.render_widget(
        Paragraph::new(totals_line(session)).alignment(Alignment::Center),
        c[2],
    );

    if charts_h > 0 {
        let [scores_area, levels_area] =
            Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)])
                .areas(c[4]);
        draw_score_chart(frame, session, scores_area, dim);
        draw_levels_chart(frame, stats, levels_area, dim);
    }

    if spark_h > 0 && !stats.recent_scores.is_empty() {
        let data: Vec<u64> = stats.recent_scores.iter().map(|s| u64::from(*s)).collect();
        frame.render_widget(
            Sparkline::default()
                .block(Block::bordered().title("recent scores").border_style(dim))
                .data(&data)
                .style(Style::default().fg(Color::Yellow)),
            c[5],
        );
    }

    let won_pct = if stats.rounds_played > 0 {
        format!(
            " ({:.0}%)",
            stats.rounds_won as f64 / stats.rounds_played as f64 * 100.0
        )
    } else {
        String::new()
    };
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            format!(
                "all-time · {} games · {}/{} won{} · streak {} · avg {:.1}s",
                stats.games,
                stats.rounds_won,
                stats.rounds_played,
                won_pct,
                stats.best_streak,
                stats.avg_guess_time,
            ),
            dim,
        )))
        .alignment(Alignment::Center),
        c[6],
    );

    frame.render_widget(
        Paragraph::new(Line::from(Span::styled("press any key to exit", dim)))
            .alignment(Alignment::Center),
        c[8],
    );
}

/// Per-round score bars, green for solved and red for missed.
fn draw_score_chart(frame: &mut Frame, session: &Session, area: Rect, dim: Style) {
    // Bars get unreadable past ~24; keep the most recent.
    let results = session.results();
    let skip = results.len().saturating_sub(24);
    let bars: Vec<Bar> = results
        .iter()
        .enumerate()
        .skip(skip)
        .map(|(i, result)| {
            Bar::default()
                .value(u64::from(result.score()))
                .label(Line::from(format!("{}", i + 1)))
                .style(if result.solved() {
                    Style::default().fg(Color::Green)
                } else {
                    Style::default().fg(Color::Red)
                })
        })
        .collect();
    frame.render_widget(
        BarChart::default()
            .block(Block::bordered().title("score per round").border_style(dim))
            .data(BarGroup::default().bars(&bars))
            .bar_width(3)
            .bar_gap(1)
            .value_style(Style::default().fg(Color::White)),
        area,
    );
}

/// All-time wins at each escalation level.
fn draw_levels_chart(frame: &mut Frame, stats: &Stats, area: Rect, dim: Style) {
    let labels = ["1s", "2s", "4s", "8s", "16s", "30s"];
    let data: Vec<(&str, u64)> = labels
        .iter()
        .zip(stats.wins_by_level.iter())
        .map(|(l, w)| (*l, *w))
        .collect();
    frame.render_widget(
        BarChart::default()
            .block(Block::bordered().title("wins per level").border_style(dim))
            .data(&data[..])
            .bar_width(5)
            .bar_gap(2)
            .bar_style(Style::default().fg(Color::Cyan))
            .value_style(Style::default().fg(Color::White)),
        area,
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
