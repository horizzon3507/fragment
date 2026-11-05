use std::path::PathBuf;

use anyhow::Result;
use clap::Parser;

use fragment::config::Config;
use fragment::library;
use fragment::session;

/// Guess the song from a snippet that keeps growing.
#[derive(Parser, Debug)]
#[command(version, about)]
struct Cli {
    /// Music library directory.
    #[arg(long, value_name = "DIR")]
    dir: Option<PathBuf>,

    /// Number of rounds per session.
    #[arg(long, value_name = "N", value_parser = clap::value_parser!(u32).range(1..=99))]
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

    let session = match session::run(&cfg, tracks) {
        Ok(session) => session,
        Err(err) => {
            eprintln!("fragment: {err:#}");
            std::process::exit(1);
        }
    };

    if session.aborted() {
        std::process::exit(130);
    }
    Ok(())
}
