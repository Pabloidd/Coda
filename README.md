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

Ready binaries are on the [release page](https://github.com/Pabloidd/Coda/releases/latest). Download the file for your system. GitHub also lists "Source code (zip)" and "Source code (tar.gz)" — those are the code, not a program you can run.

| System | File |
| --- | --- |
| Linux x86_64 | `coda-v0.2.0-linux-x86_64` |
| macOS Apple Silicon | `coda-v0.2.0-macos-aarch64` |
| Windows x86_64 | `coda-v0.2.0-windows-x86_64.exe` |

```bash
chmod +x coda-v0.2.0-linux-x86_64
install -m 755 coda-v0.2.0-linux-x86_64 ~/.local/bin/coda
coda
```

On Windows, run `coda-v0.2.0-windows-x86_64.exe`. On macOS the same `install` line works with the `macos-aarch64` file.

The Linux build needs `libasound.so.2`, `libdbus-1.so.3`, and glibc 2.35 or newer (Ubuntu 22.04, Fedora 35, and later). Fedora packages: `alsa-lib` and `dbus`. Debian and Ubuntu: `libasound2` and `libdbus-1-3`.

`SHA256SUMS` on the release page is the checksum of each file.

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
