use std::path::PathBuf;

use anyhow::Result;
use clap::Parser;

use fragment::config::Config;
use fragment::library;
use fragment::session;
use fragment::stats::Stats;

/// Guess the song from a snippet that keeps growing.
#[derive(Parser, Debug)]
#[command(version, about)]
struct Cli {
    /// Music library directory (same as --dir; the flag wins if both are given).
    #[arg(value_name = "DIR")]
    dir_arg: Option<PathBuf>,

    /// Music library directory.
    #[arg(long, value_name = "DIR")]
    dir: Option<PathBuf>,

    /// Number of rounds per session.
    #[arg(long, value_name = "N", value_parser = clap::value_parser!(u32).range(1..=99))]
    rounds: Option<u32>,

    /// Print lifetime stats and exit.
    #[arg(long)]
    stats: bool,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    if cli.stats {
        Stats::load().unwrap_or_default().print();
        return Ok(());
    }

    let cfg = Config::load(cli.dir.or(cli.dir_arg), cli.rounds)?;

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

    // Loaded before the session so the scoreboard can show all-time
    // numbers including the game just played.
    let mut stats = Stats::load().unwrap_or_default();
    let session = match session::run(&cfg, tracks, &stats) {
        Ok(session) => session,
        Err(err) => {
            eprintln!("fragment: {err:#}");
            std::process::exit(1);
        }
    };

    stats.record(&session);
    if let Err(err) = stats.save() {
        eprintln!("fragment: could not save stats: {err}");
    }

    if session.aborted() {
        std::process::exit(130);
    }
    Ok(())
}
