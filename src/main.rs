use std::path::PathBuf;

use anyhow::Result;
use clap::Parser;
use rand::seq::IndexedRandom;

use fragment::config::Config;
use fragment::game::Round;
use fragment::library;
use fragment::player::Player;
use fragment::tui;
use fragment::tui::guess::{Header, RoundEnd, play_round};
use fragment::types::Track;

/// Guess the song from a snippet that keeps growing.
#[derive(Parser, Debug)]
#[command(version, about)]
struct Cli {
    /// Music library directory.
    #[arg(long, value_name = "DIR")]
    dir: Option<PathBuf>,

    /// Number of rounds per session.
    #[arg(long, value_name = "N")]
    rounds: Option<u32>,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let cfg = Config::load(cli.dir, cli.rounds)?;

    if !cfg.music_dir.exists() {
        eprintln!(
            "fragment: music directory not found: {}",
            cfg.music_dir.display()
        );
        std::process::exit(1);
    }

    let tracks = match library::scan(&cfg.music_dir) {
        Ok(tracks) if !tracks.is_empty() => tracks,
        Ok(_) => {
            eprintln!(
                "fragment: no audio files found in {}",
                cfg.music_dir.display()
            );
            std::process::exit(1);
        }
        Err(err) => {
            eprintln!("fragment: {err}");
            std::process::exit(1);
        }
    };

    let player = match Player::spawn() {
        Ok(player) => player,
        Err(err) => {
            eprintln!("fragment: {err}");
            std::process::exit(1);
        }
    };

    let track = tracks
        .choose(&mut rand::rng())
        .expect("track list is non-empty")
        .clone();

    let mut term = tui::init()?;
    let mut round = Round::new(track);
    let header = Header {
        round: 1,
        total: 1,
        score: 0,
        streak: 0,
    };
    let result = play_round(&mut term, &mut round, &player, &header);
    tui::restore();

    let end = match result {
        Ok(end) => end,
        Err(err) => {
            let _ = player.stop();
            return Err(err);
        }
    };

    let track = display_track(round.track());
    match end {
        RoundEnd::Solved => println!(
            "✓ {track} guessed at {}s in {:.1}s",
            round.level_secs(),
            round.elapsed().as_secs_f64()
        ),
        RoundEnd::GaveUp | RoundEnd::Exhausted => println!("✗ it was {track}"),
        RoundEnd::Quit => {
            // Drop reaps mpv before exit skips destructors.
            drop(player);
            std::process::exit(130);
        }
    }
    Ok(())
}

/// "title — artist", falling back to the bare title.
fn display_track(track: &Track) -> String {
    match &track.artist {
        Some(artist) => format!("{} — {artist}", track.title),
        None => track.title.clone(),
    }
}
