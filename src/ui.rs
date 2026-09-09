use std::path::{Path, PathBuf};
use std::time::Duration;

use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Gauge, List, ListItem, ListState, Paragraph, Wrap};
use ratatui::Frame;

use crate::config::RepeatMode;
use crate::i18n::{Lang, T};
use crate::library::Library;
use crate::playlist::Queue;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    Library,
    Queue,
    Playlists,
}

impl Focus {
    pub fn next(self) -> Self {
        match self {
            Self::Library => Self::Queue,
            Self::Queue => Self::Playlists,
            Self::Playlists => Self::Library,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputMode {
    Normal,
    Filter,
    SavePlaylist,
}

pub struct UiState<'a> {
    pub lang: Lang,
    pub focus: Focus,
    pub input_mode: InputMode,
    pub input_buf: &'a str,
    pub filter: &'a str,
    pub library: &'a Library,
    pub queue: &'a Queue,
    pub lib_sel: usize,
    pub queue_sel: usize,
    pub pl_sel: usize,
    pub playlists: &'a [PathBuf],
    pub player_playing: bool,
    pub player_paused: bool,
    pub position: Duration,
    pub volume: f32,
    pub repeat: RepeatMode,
    pub status: &'a str,
    pub music_dir: &'a Path,
    pub show_help: bool,
}

// Desktop-aligned palette (kitty / Caelestia greens + yellow / red / white).
fn green() -> Color {
    Color::Rgb(0x24, 0xBD, 0x5C)
}
fn green_bright() -> Color {
    Color::Rgb(0x8a, 0xf7, 0x97)
}
fn green_dim() -> Color {
    Color::Rgb(0x1B, 0x4E, 0x2A)
}
fn bg_sel() -> Color {
    Color::Rgb(0x0f, 0x3d, 0x22)
}
fn muted() -> Color {
    Color::Rgb(0x6a, 0x8a, 0x74)
}
fn text() -> Color {
    Color::Rgb(0xe8, 0xf0, 0xea)
}
fn yellow() -> Color {
    Color::Rgb(0xe8, 0xc5, 0x47)
}
fn red() -> Color {
    Color::Rgb(0xe8, 0x5d, 0x5d)
}
fn panel_bg() -> Color {
    Color::Rgb(0x0a, 0x12, 0x0e)
}

pub fn draw(frame: &mut Frame, app: &crate::app::App) {
    let s = app.ui_state();
    let area = frame.area();

    // Soft backdrop so empty gaps aren't terminal-default gray.
    frame.render_widget(
        Block::default().style(Style::default().bg(panel_bg()).fg(text())),
        area,
    );

    let root = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(6), // now playing + progress
            Constraint::Min(8),    // lists
            Constraint::Length(3), // key hints
        ])
        .split(area);

    draw_now(frame, root[0], &s);
    draw_body(frame, root[1], &s);
    draw_hints(frame, root[2], &s);

    if s.show_help {
        draw_help(frame, area, s.lang);
    }
}

fn draw_now(frame: &mut Frame, area: Rect, s: &UiState<'_>) {
    let parts = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Length(3)])
        .split(area);

    let (glyph, label, accent) = if s.player_playing {
        ("▶", T::playing(s.lang), green_bright())
    } else if s.player_paused {
        ("⏸", T::paused(s.lang), yellow())
    } else {
        ("■", T::stopped(s.lang), red())
    };

    let current = s.queue.current();
    let title = current
        .map(|t| t.title.as_str())
        .filter(|t| !t.is_empty())
        .unwrap_or("—");
    let artist = current
        .map(|t| t.artist.as_str())
        .filter(|a| !a.is_empty() && *a != "Unknown")
        .unwrap_or("");
    let album = current
        .map(|t| t.album.as_str())
        .filter(|a| !a.is_empty())
        .unwrap_or("");

    let shuffle = if s.queue.shuffle { "⇄" } else { "" };
    let repeat = match s.repeat {
        RepeatMode::Off => "",
        RepeatMode::All => "⟳",
        RepeatMode::One => "①",
    };
    let flags = [shuffle, repeat]
        .into_iter()
        .filter(|x| !x.is_empty())
        .collect::<Vec<_>>()
        .join(" ");

    let line1 = Line::from(vec![
        Span::styled(
            format!(" {} ", T::app_title(s.lang)),
            Style::default()
                .fg(Color::Rgb(0x04, 0x14, 0x0a))
                .bg(green())
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" "),
        Span::styled(
            format!("{glyph} {label}"),
            Style::default().fg(accent).add_modifier(Modifier::BOLD),
        ),
        Span::raw("  "),
        Span::styled(title, Style::default().fg(text()).add_modifier(Modifier::BOLD)),
        Span::raw("  "),
        Span::styled(flags, Style::default().fg(yellow())),
    ]);

    let mut meta_spans = Vec::new();
    if !artist.is_empty() {
        meta_spans.push(Span::styled(artist, Style::default().fg(green_bright())));
    }
    if !album.is_empty() {
        if !meta_spans.is_empty() {
            meta_spans.push(Span::styled("  ·  ", Style::default().fg(muted())));
        }
        meta_spans.push(Span::styled(album, Style::default().fg(muted())));
    }
    if meta_spans.is_empty() {
        meta_spans.push(Span::styled(
            s.music_dir.display().to_string(),
            Style::default().fg(muted()),
        ));
    }
    let line2 = Line::from(meta_spans);

    frame.render_widget(
        Paragraph::new(vec![line1, line2]).block(
            Block::default()
                .borders(Borders::LEFT | Borders::RIGHT | Borders::TOP)
                .border_style(Style::default().fg(green_dim())),
        ),
        parts[0],
    );

    let dur = current.and_then(|t| t.duration);
    let pos = s.position;
    let ratio = match dur {
        Some(d) if d.as_secs_f64() > 0.0 => (pos.as_secs_f64() / d.as_secs_f64()).clamp(0.0, 1.0),
        _ => 0.0,
    };
    let label = format!(
        " {} / {}   vol {:.0}% ",
        fmt_duration(pos),
        dur.map(fmt_duration).unwrap_or_else(|| "--:--".into()),
        s.volume * 100.0
    );
    let gauge = Gauge::default()
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(green_dim()))
                .title(Span::styled(" progress ", Style::default().fg(muted()))),
        )
        .gauge_style(Style::default().fg(green()).bg(Color::Rgb(0x12, 0x1c, 0x16)))
        .ratio(ratio)
        .label(Span::styled(label, Style::default().fg(text())));
    frame.render_widget(gauge, parts[1]);
}

fn draw_body(frame: &mut Frame, area: Rect, s: &UiState<'_>) {
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
        .split(area);

    draw_library(frame, cols[0], s);

    let right = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(62), Constraint::Percentage(38)])
        .split(cols[1]);
    draw_queue(frame, right[0], s);
    draw_playlists(frame, right[1], s);
}

fn list_block(title: String, focused: bool) -> Block<'static> {
    let border = if focused { green() } else { green_dim() };
    let title_style = if focused {
        Style::default().fg(green_bright()).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(muted())
    };
    Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border))
        .title(Span::styled(title, title_style))
}

/// Visible window around selection so long lists actually scroll on screen.
fn window_range(len: usize, sel: usize, visible: usize) -> (usize, usize) {
    if len == 0 || visible == 0 {
        return (0, 0);
    }
    let visible = visible.max(1);
    if len <= visible {
        return (0, len);
    }
    let start = sel.saturating_sub(visible / 2).min(len.saturating_sub(visible));
    (start, (start + visible).min(len))
}

fn draw_library(frame: &mut Frame, area: Rect, s: &UiState<'_>) {
    let filtered = s.library.filtered(s.filter);
    let len = filtered.len();
    let sel = if len == 0 { 0 } else { s.lib_sel.min(len - 1) };
    let title = if s.filter.is_empty() {
        format!(" {}  {}/{} ", T::library(s.lang), if len == 0 { 0 } else { sel + 1 }, len)
    } else {
        format!(
            " {}  {}/{}  [{}] ",
            T::library(s.lang),
            if len == 0 { 0 } else { sel + 1 },
            len,
            s.filter
        )
    };

    let inner_h = area.height.saturating_sub(2) as usize;
    let (start, end) = window_range(len, sel, inner_h.max(1));

    let items: Vec<ListItem> = if len == 0 {
        vec![ListItem::new(Span::styled(
            format!("  {}", T::empty_library(s.lang)),
            Style::default().fg(muted()),
        ))]
    } else {
        filtered[start..end]
            .iter()
            .enumerate()
            .map(|(offset, t)| {
                let i = start + offset;
                let playing = s.queue.current().map(|c| c.path == t.path).unwrap_or(false);
                let prefix = if playing {
                    "▶ "
                } else if i == sel && s.focus == Focus::Library {
                    "▸ "
                } else {
                    "  "
                };
                let style = if i == sel && s.focus == Focus::Library {
                    Style::default()
                        .bg(bg_sel())
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD)
                } else if playing {
                    Style::default().fg(green_bright())
                } else {
                    Style::default().fg(text())
                };
                ListItem::new(format!("{prefix}{}", t.display_name())).style(style)
            })
            .collect()
    };

    let mut state = ListState::default();
    if len > 0 {
        state.select(Some(sel.saturating_sub(start)));
    }
    let list = List::new(items)
        .block(list_block(title, s.focus == Focus::Library))
        .highlight_symbol("");
    frame.render_stateful_widget(list, area, &mut state);
}

fn draw_queue(frame: &mut Frame, area: Rect, s: &UiState<'_>) {
    let len = s.queue.tracks.len();
    let sel = if len == 0 {
        0
    } else {
        s.queue_sel.min(len - 1)
    };
    let title = format!(
        " {}  {}/{}  {} ",
        T::queue(s.lang),
        if len == 0 { 0 } else { sel + 1 },
        len,
        T::temp_playlist(s.lang)
    );

    let inner_h = area.height.saturating_sub(2) as usize;
    let (start, end) = window_range(len, sel, inner_h.max(1));

    let items: Vec<ListItem> = if len == 0 {
        vec![ListItem::new(Span::styled(
            "  —",
            Style::default().fg(muted()),
        ))]
    } else {
        s.queue.tracks[start..end]
            .iter()
            .enumerate()
            .map(|(offset, t)| {
                let i = start + offset;
                let playing = s.queue.index == Some(i);
                let prefix = if playing {
                    "▶ "
                } else if i == sel && s.focus == Focus::Queue {
                    "▸ "
                } else {
                    "  "
                };
                let style = if i == sel && s.focus == Focus::Queue {
                    Style::default()
                        .bg(bg_sel())
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD)
                } else if playing {
                    Style::default().fg(green_bright())
                } else {
                    Style::default().fg(text())
                };
                ListItem::new(format!("{prefix}{}", t.display_name())).style(style)
            })
            .collect()
    };

    let mut state = ListState::default();
    if len > 0 {
        state.select(Some(sel.saturating_sub(start)));
    }
    frame.render_stateful_widget(
        List::new(items).block(list_block(title, s.focus == Focus::Queue)),
        area,
        &mut state,
    );
}

fn draw_playlists(frame: &mut Frame, area: Rect, s: &UiState<'_>) {
    let len = s.playlists.len();
    let sel = if len == 0 {
        0
    } else {
        s.pl_sel.min(len - 1)
    };
    let title = format!(
        " {}  {}/{} ",
        T::playlists(s.lang),
        if len == 0 { 0 } else { sel + 1 },
        len
    );

    let inner_h = area.height.saturating_sub(2) as usize;
    let (start, end) = window_range(len, sel, inner_h.max(1));

    let items: Vec<ListItem> = if len == 0 {
        vec![ListItem::new(Span::styled(
            "  —",
            Style::default().fg(muted()),
        ))]
    } else {
        s.playlists[start..end]
            .iter()
            .enumerate()
            .map(|(offset, p)| {
                let i = start + offset;
                let name = p.file_stem().and_then(|x| x.to_str()).unwrap_or("?");
                let prefix = if i == sel && s.focus == Focus::Playlists {
                    "▸ "
                } else {
                    "  "
                };
                let style = if i == sel && s.focus == Focus::Playlists {
                    Style::default()
                        .bg(bg_sel())
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(text())
                };
                ListItem::new(format!("{prefix}{name}")).style(style)
            })
            .collect()
    };

    let mut state = ListState::default();
    if len > 0 {
        state.select(Some(sel.saturating_sub(start)));
    }
    frame.render_stateful_widget(
        List::new(items).block(list_block(title, s.focus == Focus::Playlists)),
        area,
        &mut state,
    );
}

fn draw_hints(frame: &mut Frame, area: Rect, s: &UiState<'_>) {
    let body = match s.input_mode {
        InputMode::Filter => Paragraph::new(Line::from(vec![
            Span::styled(
                " filter ",
                Style::default()
                    .fg(panel_bg())
                    .bg(yellow())
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" "),
            Span::styled(format!("{}_", s.input_buf), Style::default().fg(text())),
        ])),
        InputMode::SavePlaylist => Paragraph::new(Line::from(vec![
            Span::styled(
                " save ",
                Style::default()
                    .fg(panel_bg())
                    .bg(green())
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" "),
            Span::styled(format!("{}_", s.input_buf), Style::default().fg(text())),
        ])),
        InputMode::Normal if s.status.is_empty() => Paragraph::new(T::footer_hints(s.lang))
            .style(Style::default().fg(muted()))
            .alignment(Alignment::Left),
        InputMode::Normal => Paragraph::new(vec![
            Line::from(Span::styled(
                T::footer_hints(s.lang),
                Style::default().fg(muted()),
            )),
            Line::from(Span::styled(s.status, Style::default().fg(yellow()))),
        ]),
    };

    frame.render_widget(
        body.block(
            Block::default()
                .borders(Borders::TOP)
                .border_style(Style::default().fg(green_dim()))
                .title(Span::styled(
                    " keys · ? help ",
                    Style::default().fg(green()),
                )),
        ),
        area,
    );
}

fn draw_help(frame: &mut Frame, area: Rect, lang: Lang) {
    let popup = centered(area, 70, 84);
    frame.render_widget(Clear, popup);
    let mut lines: Vec<Line> = Vec::new();
    lines.push(Line::from(Span::styled(
        T::help_banner(lang),
        Style::default().fg(green_bright()).add_modifier(Modifier::BOLD),
    )));
    lines.push(Line::from(""));
    lines.extend(T::help_lines(lang).into_iter().map(|l| {
        if let Some((k, rest)) = l.split_once("  ") {
            Line::from(vec![
                Span::styled(format!("{k:<12}"), Style::default().fg(green())),
                Span::styled(rest.trim_start(), Style::default().fg(text())),
            ])
        } else if l.is_empty() {
            Line::from("")
        } else {
            Line::from(Span::styled(l, Style::default().fg(muted())))
        }
    }));
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        T::help_dismiss(lang),
        Style::default().fg(yellow()),
    )));
    frame.render_widget(
        Paragraph::new(lines).wrap(Wrap { trim: false }).block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(green()))
                .title(Span::styled(
                    format!(" {} ", T::help(lang)),
                    Style::default().fg(green_bright()).add_modifier(Modifier::BOLD),
                ))
                .style(Style::default().bg(panel_bg())),
        ),
        popup,
    );
}

fn centered(area: Rect, percent_x: u16, percent_y: u16) -> Rect {
    let popup = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(area)[1];
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup)[1]
}

fn fmt_duration(d: Duration) -> String {
    let secs = d.as_secs();
    format!("{}:{:02}", secs / 60, secs % 60)
}
