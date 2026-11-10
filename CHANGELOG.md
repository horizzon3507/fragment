# Changelog

We follow [Semantic Versioning](https://semver.org/) and [Keep a Changelog](https://keepachangelog.com/). fragment is a single CLI surface.

<details>
<summary>To see more about versioning, expand this.</summary>

Every version string starts with `v` (required), e.g. `v0.2.0-alpha`, `v0.1.0-alpha`.

Here the installable surface is **CLI** (`fragment` in the terminal).

| Part | What you install | Example |
| --- | --- | --- |
| **CLI** | `fragment` in the terminal | `v0.2.0-alpha` |

With one surface there is no `m` in the tag and no per-surface sections — just the version notes.

Each release heading is the version and date (`## v0.2.0-alpha · 08/11/2026`); under it, a short summary ends with a plain sentence like: "This version was made for CLI with an alpha release channel on 08/11/2026 (v0.2.0-alpha)."

### What the channel suffix means

| Suffix | In plain words |
| --- | --- |
| **-alpha** | Very early. Expect missing pieces and lots of bugs. |
| **-beta** | Mostly there, but still rough. Fine to try; not the "official" install. |
| **-stable** | Ready for daily use. This is what we put on GitHub Releases and the AUR. |

We only call something **stable** when we mean it.

</details>

## v0.2.0-alpha · 08/11/2026

The consolidated cut: a complete local Heardle in your terminal — library scan, escalating snippets through mpv, fuzzy guesses with autocomplete, sessions with streaks and a scoreboard with charts, persistent stats and a config file. This version was made for CLI with an alpha release channel on 08/11/2026 (v0.2.0-alpha).

- Snippet playback through a persistent `mpv --idle` process driven over its JSON IPC socket: loads paused, waits for `file-loaded`, verifies the resolved path, then seeks to a random offset and plays. Every IPC reply is checked so playback errors reach the status line.
- Recursive library scan (`--dir` or positional `fragment <dir>`, default `~/Music`): tags via lofty, `"Artist - Title.ext"` filename fallback, deduplicated candidate index.
- Escalating snippet ladder — 1s doubling through 2s, 4s, 8s and 16s up to 30s. Level growth is always confirmed: `Tab`/`+` grows on demand and the timer only freezes at zero waiting for the same key — it never steals the round.
- Fuzzy guess matching on normalized text (lowercased, `(feat. …)`-style brackets removed, punctuation and accents stripped): exact match or Jaro-Winkler ≥ 0.85 against the title or `"artist title"`.
- Title autocomplete under the input — normalized substring, title prefixes first. `↑`/`↓` pick with wraparound, `Enter` submits the highlighted or top suggestion, `→` fills the input for editing.
- `Ctrl+R` replays the current snippet from the same offset.
- N-round sessions (`--rounds`, default 5) with streak tracking, a reveal screen and a final scoreboard: per-round score bars, all-time wins per level, a recent-scores sparkline and an all-time stats line. `fragment --stats` prints the same numbers to stdout.
- Scoring per level base (1000 down to 100), a −10-points-per-second time penalty capped at half the base, and a +10%-per-streak multiplier.
- XDG config file at `~/.config/fragment/config.toml` for `music_dir` and `rounds`; CLI flags win over the file, the file wins over defaults.
- Persistent JSON stats at `~/.local/share/fragment/stats.json`: games, rounds won/played, best streak, average guess time, wins by level and recent scores — atomic writes with `.bak` recovery.
- Various other small tweaks.
