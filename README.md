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

The player is a single file on the [release page](https://github.com/Pabloidd/Coda/releases/tag/v0.1.0): **coda-v0.1.0-linux-x86_64**.

GitHub also puts "Source code (zip)" and "Source code (tar.gz)" on that page. Those are the code, not a program you can run. Download the file named `coda-v0.1.0-linux-x86_64`.

```bash
chmod +x coda-v0.1.0-linux-x86_64
install -m 755 coda-v0.1.0-linux-x86_64 ~/.local/bin/coda
coda
```

It needs `libasound.so.2`, `libdbus-1.so.3`, and glibc 2.35 or newer (Ubuntu 22.04, Fedora 35, and later). Fedora packages: `alsa-lib` and `dbus`. Debian and Ubuntu: `libasound2` and `libdbus-1-3`.

`coda-v0.1.0-linux-x86_64.sha256` is the checksum of that same file. Newer versions keep the same name with a new tag, on the [Releases](https://github.com/Pabloidd/Coda/releases) page.

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
