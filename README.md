# coda

Minimal terminal music player for local files.

**coda** (noun, music) — a concluding passage. Short name, light process, stops when the terminal closes.

## Features

- Scans `~/Music` by default (`mp3` / `flac` / `ogg` / `opus` / `m4a` / `wav` / …)
- Temporary queue + permanent `.m3u` playlists
- Play / pause, next / prev, seek ±5s, shuffle, repeat, volume
- TUI (ratatui), language toggle EN ↔ RU, themes (`forest` / `dawn`)
- Single instance
- MPRIS metadata for DE widgets (no global hotkeys in v1)

## Run

Needs system deps (Fedora): `dbus-devel alsa-lib-devel pkgconf-pkg-config gcc`.

```bash
cd ~/workspace/Projects/coda
cargo build --release
cp -f target/release/coda ~/.local/bin/coda
# or: cargo run --release
```

No Nix required. Desktop entry: `~/.local/share/applications/coda.desktop` → Ptyxis + `~/.local/bin/coda`.

Themes: `t` cycles `forest` ↔ `dawn` (persisted in config).

## Config

| Path | Purpose |
|------|---------|
| `~/.config/coda/config.toml` | music dir, language, theme, volume, repeat |
| `~/.local/share/coda/playlists/*.m3u` | saved playlists |

## Keys

Press `?` inside the app for the full list.

Highlights: `Space` play/pause · `n`/`p` next/prev · `-`/`=` volume · `s` shuffle · `r` repeat · `S` save queue · `/` filter · `t` theme · `L` language · `q` quit
