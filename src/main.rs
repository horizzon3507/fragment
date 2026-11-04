use std::path::PathBuf;

use anyhow::Result;
use clap::Parser;

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
    let _cfg = fragment::config::Config::load(cli.dir, cli.rounds)?;
    Ok(())
}
