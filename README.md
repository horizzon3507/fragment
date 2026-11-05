# fragment

Guess the song from a snippet that keeps growing — local music, terminal game.

fragment is a Heardle-style guessing game for your own music library. It is fully local-first: no accounts, no streaming, no network. A ratatui terminal UI plays short snippets through mpv, you type the title, and the snippet doubles every time you miss.

## Requirements

- **mpv** on `PATH` — playback runs through a persistent `mpv --idle` process driven over its JSON IPC socket.
- **A local music library** — mp3, flac, ogg, opus, m4a, mp4, aac, wav, aiff, ape, wv. Title/artist tags are read with [lofty](https://github.com/Serial-ATA/lofty); files without tags fall back to `"Artist - Title.ext"` filename parsing.
- A Unix-like system (the IPC socket is a Unix socket) and a Rust toolchain to build.

## Install

```sh
cargo install --path .
```

or build and grab the binary yourself:

```sh
cargo build --release   # → target/release/fragment
```

## Usage

```sh
fragment [DIR] [--dir DIR] [--rounds N] [--stats]
```

| Flag | Default | Description |
| --- | --- | --- |
| `DIR` | — | Positional music library directory (same as `--dir`; the flag wins if both are given) |
| `--dir DIR` | XDG music dir | Music library directory, scanned recursively (`~/Music` on most systems, `$HOME/Music` fallback) |
| `--rounds N` | `5` | Rounds per session (1–99) |
| `--stats` | — | Print lifetime stats to stdout and exit |

So `fragment ~/Music/rock` plays only from that directory.

## How it plays

Each round picks a random track and a random start offset, then plays a **1s** snippet. Each level's countdown gives you as many seconds to guess as the snippet is long; a wrong guess, a timeout, or `Tab`/`+` doubles the snippet:

**1s → 2s → 4s → 8s → 16s → 30s**

Once the last level is spent — or you give up — the reveal shows the title, artist, level reached and your elapsed time. After N rounds you get a scoreboard: per-round results, score-per-round and wins-per-level charts, a recent-scores sparkline, total score, best streak and all-time stats.

| Key | Action |
| --- | --- |
| `Enter` | Submit guess — or accept the highlighted suggestion (blank Enter clears the prompt) |
| `Tab` / `+` | Skip to the next snippet length |
| `Ctrl+R` | Replay the current snippet from the same offset |
| `↑` / `↓` | Move the suggestion highlight |
| `→` | Accept a suggestion (the highlight, or the top row) |
| `Esc` | Clear the suggestion pick, then give up and reveal the answer |
| `Ctrl+C` | Quit the session |
| `Backspace` | Edit your guess |

As you type, matching library tracks appear under the input — title prefixes rank first, then `Artist - Title` matches.

## Scoring

| Level | Snippet | Base points |
| --- | --- | --- |
| 1 | 1s | 1000 |
| 2 | 2s | 800 |
| 3 | 4s | 600 |
| 4 | 8s | 400 |
| 5 | 16s | 250 |
| 6 | 30s | 100 |

```
score = (base − 10 × seconds elapsed) × (1 + 0.10 × streak)
```

The time penalty is capped at 50% of the base, and `streak` is your run of consecutive solves this session — counted as the streak the round started with. Unsolved rounds score 0 and reset the streak.

## Guessing rules

Guesses are normalized before comparison: lowercased, bracketed segments like `(feat. …)` or `[remaster]` removed, punctuation dropped, accents stripped, whitespace collapsed. A guess wins on an exact match or a Jaro-Winkler similarity of at least **0.85** against either the title or the combined `"artist title"` string.

## Config

Optional config file at `$XDG_CONFIG_HOME/fragment/config.toml` (Linux: `~/.config/fragment/config.toml`):

```toml
music_dir = "~/Music"
rounds = 10
```

Precedence: **CLI flags > config file > defaults**. Both keys are optional, `~` expands to `$HOME`, and unknown keys are ignored so newer files keep loading.

## Stats

Lifetime stats persist to `$XDG_DATA_HOME/fragment/stats.json` (Linux: `~/.local/share/fragment/stats.json`): games played, rounds won, rounds played, best streak, average guess time across solved rounds, wins per snippet level, and the last 30 game totals. `fragment --stats` prints them to stdout; the scoreboard shows the same numbers as charts. A corrupt file is moved aside to `stats.json.bak` instead of crashing or losing data.

## License

MIT
