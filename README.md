# coda

Minimal terminal music player for local files.

**coda** (noun, music) — a concluding passage. Short name, light process, stops when the terminal closes.

## Features

- Scans `~/Music` by default (`mp3` / `flac` / `ogg` / `opus` / `m4a` / `wav` / …)
- Temporary queue + permanent `.m3u` playlists
- Play / pause, next / prev, seek ±5s, shuffle, repeat, volume
- TUI (ratatui), language toggle EN ↔ RU
- Single instance
- MPRIS metadata for DE widgets (no global hotkeys in v1)

## Run

```bash
cd ~/workspace/Projects/coda
nix-shell
cargo run --release
```

Binary after build: `./target/release/coda`

Optional Hyprland launch bind (opens in kitty):

```
bind = SUPER, M, exec, kitty -e /home/pabloid/workspace/Projects/coda/target/release/coda
```

## Config

| Path | Purpose |
|------|---------|
| `~/.config/coda/config.toml` | music dir, language, volume, repeat |
| `~/.local/share/coda/playlists/*.m3u` | saved playlists |

## Keys

Press `?` inside the app for the full list.

Highlights: `Space` play/pause · `n`/`p` next/prev · `-`/`=` volume · `s` shuffle · `r` repeat · `S` save queue · `/` filter · `L` language · `q` quit
