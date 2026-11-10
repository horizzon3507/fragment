# Changelog

We follow [Semantic Versioning](https://semver.org/) and [Keep a Changelog](https://keepachangelog.com/). fragment is a single CLI surface.

<details>
<summary>To see more about versioning, expand this.</summary>

Every version string starts with `v` (required), e.g. `v0.2.0-alpha`, `v0.1.0-alpha`.

Here the installable surface is **CLI** (`fragment` in the terminal).

| Part | What you install | Example |
| --- | --- | --- |
| **CLI** | `fragment` in the terminal | `v0.1.0-alpha` |

With one surface there is no `m` in the tag and no per-surface sections — just the version notes.

Each release heading is the version and date (`## v0.1.0-alpha · 04/11/2026`); under it, a short summary ends with a plain sentence like: "This version was made for CLI with an alpha release channel on 04/11/2026 (v0.1.0-alpha)."

### What the channel suffix means

| Suffix | In plain words |
| --- | --- |
| **-alpha** | Very early. Expect missing pieces and lots of bugs. |
| **-beta** | Mostly there, but still rough. Fine to try; not the "official" install. |
| **-stable** | Ready for daily use. This is what we put on GitHub Releases and the AUR. |

We only call something **stable** when we mean it.

</details>

## v0.2.0-alpha · 05/11/2026

Sound, actually. The first alpha shipped an mpv IPC call its newer builds reject, so snippets never loaded and the game was silent — this version fixes playback and adds the quality-of-life layer on top. This version was made for CLI with an alpha release channel on 05/11/2026 (v0.2.0-alpha).

- Fix silent playback: mpv 0.41 rejects `loadfile`'s options object (`invalid parameter`), so no file ever loaded. The player now loads paused, waits for mpv's `file-loaded` event before seeking (a seek sent while the file opens is silently dropped), and checks every IPC reply — playback errors now reach the status line instead of vanishing.
- Title autocomplete under the guess input: type to filter the library (normalized substring, title prefixes first), `↑`/`↓` to pick, `→` or `Enter` to accept, `Esc` to clear the pick.
- `Ctrl+R` replays the current snippet from the same offset.
- Positional music dir: `fragment ~/Music/rock` alongside `--dir`.
- Scoreboard charts: per-round score bars, all-time wins per level, a recent-scores sparkline and an all-time stats line. `fragment --stats` prints the same numbers to stdout.
- Stats file gains `wins_by_level` and `recent_scores`; older files load unchanged.
- Various other small tweaks.

## v0.1.0-alpha · 04/11/2026

The first cut: the full local Heardle loop in your terminal. Pick a directory of music, hear a 1-second snippet, type the title before the snippet eats the whole song. This version was made for CLI with an alpha release channel on 04/11/2026 (v0.1.0-alpha).

- Snippet playback through a persistent `mpv --idle` process driven over its JSON IPC socket, with a random track and a random start offset every round.
- Escalating snippet ladder — 1s doubling through 2s, 4s, 8s and 16s up to 30s — on a wrong guess, a timeout or a `Tab`/`+` skip.
- Fuzzy guess matching on normalized text (lowercased, `(feat. …)`-style brackets removed, punctuation and accents stripped): exact match or Jaro-Winkler ≥ 0.85 against the title or `"artist title"`.
- ratatui TUI with a countdown timer bar, the level ladder, a miss log and a per-round reveal screen.
- N-round sessions (`--rounds`, default 5) with streak tracking and a final scoreboard.
- Scoring per level base (1000 down to 100), a −10-points-per-second time penalty capped at half the base, and a +10%-per-streak multiplier.
- Recursive library scan of your music dir (`--dir`, default `~/Music`): tags via lofty, `"Artist - Title.ext"` filename fallback.
- XDG config file at `~/.config/fragment/config.toml` for `music_dir` and `rounds`; CLI flags win over the file, the file wins over defaults.
- Persistent JSON stats at `~/.local/share/fragment/stats.json`: games, rounds won/played, best streak, average guess time.
- Various other small tweaks.
