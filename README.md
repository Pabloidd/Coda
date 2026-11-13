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

## Download

Linux x86_64 builds are on the [Releases](https://github.com/Pabloidd/Coda/releases) page. Take `coda-v*-linux-x86_64`. The "Source code" archives are the code, not the player.

```bash
chmod +x coda-v0.1.0-linux-x86_64
install -m 755 coda-v0.1.0-linux-x86_64 ~/.local/bin/coda
```

Runtime libraries: `libasound.so.2` and `libdbus-1.so.3`. glibc 2.35 or newer.

## Build from source

Needs a C toolchain, pkg-config, ALSA, and dbus headers (Fedora: `dbus-devel alsa-lib-devel pkgconf-pkg-config gcc`).

```bash
cargo build --release
install -m 755 target/release/coda ~/.local/bin/coda
```

Themes: `t` cycles `forest` ↔ `dawn` (persisted in config).

## Config

| Path | Purpose |
|------|---------|
| `~/.config/coda/config.toml` | music dir, language, theme, volume, repeat |
| `~/.local/share/coda/playlists/*.m3u` | saved playlists |

## Keys

Press `?` inside the app for the full list.

Highlights: `Space` play/pause · `n`/`p` next/prev · `-`/`=` volume · `s` shuffle · `r` repeat · `S` save queue · `/` filter · `t` theme · `L` language · `q` quit
