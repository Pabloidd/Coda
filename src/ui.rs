use std::path::{Path, PathBuf};
use std::time::Duration;

use ratatui::layout::{Constraint, Direction, Layout, Position, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{
    Block, BorderType, Borders, Clear, List, ListItem, ListState, Padding, Paragraph, Wrap,
};
use ratatui::Frame;
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use crate::config::RepeatMode;
use crate::i18n::{Lang, T};
use crate::library::{Library, Track};
use crate::playlist::Queue;
use crate::theme::{Palette, Theme};

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

    pub fn prev(self) -> Self {
        match self {
            Self::Library => Self::Playlists,
            Self::Queue => Self::Library,
            Self::Playlists => Self::Queue,
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
    pub theme: Theme,
    pub focus: Focus,
    pub input_mode: InputMode,
    pub input_buf: &'a str,
    pub filter: &'a str,
    /// Indexes into `library.tracks`. Unused when `filter` is empty.
    pub filter_idx: &'a [usize],
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

pub struct Shell {
    pub now: Rect,
    pub progress: Rect,
    pub library: Rect,
    pub queue: Rect,
    pub playlists: Option<Rect>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Click {
    Library(usize),
    Queue(usize),
    Playlist(usize),
    Seek(f64),
    Panel(Focus),
}

pub fn shell(area: Rect) -> Shell {
    let root = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(6),
            Constraint::Min(0),
            Constraint::Length(2),
        ])
        .split(area);
    let now = root[0];
    let body = root[1];
    let show_pl = area.height >= 24 && area.width >= 64;
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(58), Constraint::Percentage(42)])
        .split(body);
    let (library, queue, playlists) = if show_pl {
        let rows = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(64), Constraint::Percentage(36)])
            .split(cols[1]);
        (cols[0], rows[0], Some(rows[1]))
    } else {
        (cols[0], cols[1], None)
    };

    let progress = Rect {
        x: now.x.saturating_add(1 + PAD_LEFT),
        y: now.y.saturating_add(4),
        width: now.width.saturating_sub(2 + PAD_LEFT + PAD_RIGHT),
        height: 1,
    };
    Shell {
        now,
        progress,
        library,
        queue,
        playlists,
    }
}

pub fn click_target(area: Rect, col: u16, row: u16, s: &UiState<'_>) -> Click {
    let shell = shell(area);
    if in_rect(shell.progress, col, row) && shell.progress.width > 0 {
        let ratio = (col.saturating_sub(shell.progress.x)) as f64 / shell.progress.width as f64;
        return Click::Seek(ratio.clamp(0.0, 1.0));
    }
    if let Some(index) = row_in_list(shell.library, col, row, lib_len(s), s.lib_sel) {
        return Click::Library(index);
    }
    if in_rect(shell.library, col, row) {
        return Click::Panel(Focus::Library);
    }
    let qlen = s.queue.play_len();
    if let Some(index) = row_in_list(
        shell.queue,
        col,
        row,
        qlen,
        s.queue_sel.min(qlen.saturating_sub(1)),
    ) {
        return Click::Queue(index);
    }
    if in_rect(shell.queue, col, row) {
        return Click::Panel(Focus::Queue);
    }
    if let Some(rect) = shell.playlists {
        if let Some(index) = row_in_list(rect, col, row, s.playlists.len(), s.pl_sel) {
            return Click::Playlist(index);
        }
        if in_rect(rect, col, row) {
            return Click::Panel(Focus::Playlists);
        }
    }
    Click::Panel(s.focus)
}

pub fn draw(frame: &mut Frame, app: &crate::app::App) {
    let s = app.ui_state();
    let p = s.theme.palette();
    let area = frame.area();
    let shell = shell(area);
    let footer = shell_footer(area);

    paint_canvas(frame, area, &p);

    draw_now(frame, shell.now, &s, &p);
    draw_library(frame, shell.library, &s, &p);
    draw_queue(frame, shell.queue, &s, &p);
    if let Some(rect) = shell.playlists {
        draw_playlists(frame, rect, &s, &p);
    }
    draw_hints(frame, footer, &s, &p);

    if s.show_help {
        draw_help(frame, area, s.lang, &p);
    }
}

fn shell_footer(area: Rect) -> Rect {
    Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(6),
            Constraint::Min(0),
            Constraint::Length(2),
        ])
        .split(area)[2]
}

fn draw_now(frame: &mut Frame, area: Rect, s: &UiState<'_>, p: &Palette) {
    let inner_w = text_width(area.width);
    let (glyph, label, accent) = if s.player_playing {
        ("▶", T::playing(s.lang), p.accent_bright)
    } else if s.player_paused {
        ("⏸", T::paused(s.lang), p.yellow)
    } else {
        ("■", T::stopped(s.lang), p.red)
    };

    let current = s.queue.current();
    let title = current
        .map(|t| t.title.as_ref())
        .filter(|t: &&str| !t.is_empty())
        .unwrap_or("—");
    let artist = current
        .map(|t| t.artist.as_ref())
        .filter(|a: &&str| !a.is_empty() && *a != "Unknown")
        .unwrap_or("");
    let album = current
        .map(|t| t.album.as_ref())
        .filter(|a: &&str| !a.is_empty())
        .unwrap_or("");

    let shuffle_style = if s.queue.shuffle {
        Style::default().fg(p.accent_bright)
    } else {
        Style::default().fg(p.muted)
    };
    let repeat_label = match s.repeat {
        RepeatMode::Off => "○",
        RepeatMode::All => "⟳",
        RepeatMode::One => "①",
    };
    let repeat_style = if s.repeat == RepeatMode::Off {
        Style::default().fg(p.muted)
    } else {
        Style::default().fg(p.accent_bright)
    };

    let line1 = Line::from(vec![
        Span::styled(
            format!(" {} ", T::app_title(s.lang)),
            Style::default()
                .fg(p.badge_fg)
                .bg(p.accent)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" "),
        Span::styled(
            format!("{glyph} {label}"),
            Style::default().fg(accent).add_modifier(Modifier::BOLD),
        ),
        Span::raw("  "),
        Span::styled("⇄", shuffle_style),
        Span::raw(" "),
        Span::styled(repeat_label, repeat_style),
        Span::raw("  "),
        Span::styled(
            format!("{:.0}%", s.volume * 100.0),
            Style::default().fg(p.muted),
        ),
    ]);

    let line2 = Line::from(Span::styled(
        fit(title, inner_w.max(1)),
        Style::default().fg(p.text).add_modifier(Modifier::BOLD),
    ));

    let next = next_label(s);
    let meta = if artist.is_empty() && album.is_empty() {
        s.music_dir.display().to_string()
    } else if album.is_empty() {
        artist.to_string()
    } else if artist.is_empty() {
        album.to_string()
    } else {
        format!("{artist}  ·  {album}")
    };
    let line3 = if let Some(next) = next {
        let tag = format!("{} ", T::up_next(s.lang));
        let reserve = tag.chars().count() + 1;
        let room = inner_w.saturating_sub(reserve);
        let meta_w = room / 2;
        let next_w = room.saturating_sub(meta_w);
        Line::from(vec![
            Span::styled(fit(&meta, meta_w), Style::default().fg(p.muted)),
            Span::raw(" "),
            Span::styled(tag, Style::default().fg(p.border)),
            Span::styled(fit(&next, next_w), Style::default().fg(p.text)),
        ])
    } else {
        Line::from(Span::styled(
            fit(&meta, inner_w),
            Style::default().fg(p.muted),
        ))
    };

    let dur = current.and_then(|t| t.duration);
    let ratio = match dur {
        Some(d) if d.as_secs_f64() > 0.0 => {
            (s.position.as_secs_f64() / d.as_secs_f64()).clamp(0.0, 1.0)
        }
        _ => 0.0,
    };
    let times = format!(
        " {} / {} ",
        fmt_duration(s.position),
        dur.map(fmt_duration).unwrap_or_else(|| "--:--".into())
    );
    let bar_w = inner_w.saturating_sub(times.chars().count()).max(1);
    let filled = (ratio * bar_w as f64).round() as usize;
    let filled = filled.min(bar_w);
    let line4 = Line::from(vec![
        Span::styled("━".repeat(filled), Style::default().fg(p.accent)),
        Span::styled(
            "─".repeat(bar_w - filled),
            Style::default().fg(p.gauge_track),
        ),
        Span::styled(times, Style::default().fg(p.muted)),
    ]);

    frame.render_widget(
        Paragraph::new(vec![line1, line2, line3, line4]).block(glass_block(
            String::new(),
            false,
            p,
        )),
        area,
    );
}

fn next_label(s: &UiState<'_>) -> Option<String> {
    let index = s.queue.index?;
    let pos = s.queue.play_pos_of_index(index);
    let next_pos = if s.repeat == RepeatMode::One {
        pos
    } else if pos + 1 < s.queue.play_len() {
        pos + 1
    } else if s.repeat == RepeatMode::All && s.queue.play_len() > 1 {
        0
    } else {
        return None;
    };
    let next = s.queue.track_at_play(next_pos)?;
    Some(next.title.to_string())
}

fn draw_library(frame: &mut Frame, area: Rect, s: &UiState<'_>, p: &Palette) {
    let len = lib_len(s);
    let sel = if len == 0 { 0 } else { s.lib_sel.min(len - 1) };
    let title = if s.filter.is_empty() {
        format!(
            " {}  {}/{} ",
            T::library(s.lang),
            if len == 0 { 0 } else { sel + 1 },
            len
        )
    } else {
        format!(
            " {}  {}/{}  [{}] ",
            T::library(s.lang),
            if len == 0 { 0 } else { sel + 1 },
            len,
            s.filter
        )
    };
    let inner_w = text_width(area.width);
    let inner_h = area.height.saturating_sub(2) as usize;
    let (start, end) = window_range(len, sel, inner_h.max(1));
    let focused = s.focus == Focus::Library;

    let items: Vec<ListItem> = if len == 0 {
        vec![ListItem::new(Span::styled(
            format!("  {}", T::empty_library(s.lang)),
            Style::default().fg(p.muted),
        ))]
    } else {
        (start..end)
            .map(|i| {
                let track = lib_track(s, i);
                let prev = i.checked_sub(1).map(|p| lib_track(s, p));
                let playing = s.queue.current().is_some_and(|c| c.path == track.path);
                let selected = focused && i == sel;
                track_row(track, prev, playing, selected, true, inner_w, p)
            })
            .collect()
    };
    render_list(frame, area, items, title, focused, len, sel, start, p);
}

fn draw_queue(frame: &mut Frame, area: Rect, s: &UiState<'_>, p: &Palette) {
    let len = s.queue.play_len();
    let sel = if len == 0 {
        0
    } else {
        s.queue_sel.min(len - 1)
    };
    let title = format!(
        " {}  {}/{} ",
        T::queue(s.lang),
        if len == 0 { 0 } else { sel + 1 },
        len
    );
    let inner_w = text_width(area.width);
    let inner_h = area.height.saturating_sub(2) as usize;
    let (start, end) = window_range(len, sel, inner_h.max(1));
    let focused = s.focus == Focus::Queue;

    let items: Vec<ListItem> = if len == 0 {
        vec![ListItem::new(Span::styled(
            format!("  {}", T::empty_queue(s.lang)),
            Style::default().fg(p.muted),
        ))]
    } else {
        (start..end)
            .filter_map(|i| {
                let track = s.queue.track_at_play(i)?;
                let playing = s.queue.index == s.queue.track_index_at_play(i);
                let selected = focused && i == sel;
                let num_w = len.to_string().len().max(2);
                Some(queue_row(track, i, playing, selected, inner_w, num_w, p))
            })
            .collect()
    };
    render_list(frame, area, items, title, focused, len, sel, start, p);
}

fn draw_playlists(frame: &mut Frame, area: Rect, s: &UiState<'_>, p: &Palette) {
    let len = s.playlists.len();
    let sel = if len == 0 { 0 } else { s.pl_sel.min(len - 1) };
    let title = format!(
        " {}  {}/{} ",
        T::playlists(s.lang),
        if len == 0 { 0 } else { sel + 1 },
        len
    );
    let inner_h = area.height.saturating_sub(2) as usize;
    let (start, end) = window_range(len, sel, inner_h.max(1));
    let focused = s.focus == Focus::Playlists;

    let items: Vec<ListItem> = if len == 0 {
        vec![ListItem::new(Span::styled(
            format!("  {}", T::empty_playlists(s.lang)),
            Style::default().fg(p.muted),
        ))]
    } else {
        s.playlists[start..end]
            .iter()
            .enumerate()
            .map(|(offset, path)| {
                let i = start + offset;
                let name = path.file_stem().and_then(|x| x.to_str()).unwrap_or("?");
                let selected = focused && i == sel;
                let row_bg = if selected { p.sel_bg } else { p.panel };
                let style = if selected {
                    Style::default().bg(row_bg).fg(p.sel_fg)
                } else {
                    Style::default().fg(p.text).bg(row_bg)
                };
                ListItem::new(Line::from(vec![
                    Span::styled(
                        if selected { "> " } else { "  " },
                        Style::default().fg(p.accent).bg(row_bg),
                    ),
                    Span::styled(name.to_string(), style),
                ]))
                .style(style)
            })
            .collect()
    };
    render_list(frame, area, items, title, focused, len, sel, start, p);
}

fn render_list(
    frame: &mut Frame,
    area: Rect,
    items: Vec<ListItem<'_>>,
    title: String,
    focused: bool,
    len: usize,
    sel: usize,
    start: usize,
    p: &Palette,
) {
    let mut state = ListState::default();
    if len > 0 {
        state.select(Some(sel.saturating_sub(start)));
    }
    frame.render_stateful_widget(
        List::new(items)
            .block(glass_block(title, focused, p))
            .highlight_style(Style::default())
            .highlight_symbol(""),
        area,
        &mut state,
    );
}

fn track_row<'a>(
    track: &'a Track,
    prev: Option<&'a Track>,
    playing: bool,
    selected: bool,
    grouped: bool,
    width: usize,
    p: &Palette,
) -> ListItem<'a> {
    let new_group = !grouped
        || prev.is_none_or(|prev| prev.artist != track.artist || prev.album != track.album);
    let marker = if playing { "> " } else { "  " };
    let marker_style = Style::default().fg(if playing { p.accent } else { p.muted });
    let (title_fg, row_bg) = if selected {
        (p.sel_fg, p.sel_bg)
    } else if playing {
        (p.accent_bright, p.panel)
    } else {
        (p.text, p.panel)
    };
    let base = Style::default().fg(title_fg).bg(row_bg);
    let marker_style = marker_style.bg(row_bg);
    let dur = dur_cell(
        &track
            .duration
            .map(fmt_duration)
            .unwrap_or_else(|| "--:--".into()),
    );
    let artist = if track.artist.is_empty() || track.artist.as_ref() == "Unknown" {
        track.album.as_ref()
    } else {
        track.artist.as_ref()
    };
    let (head, title_w) = library_head(artist, track.title.as_ref(), &dur, width, new_group);
    let mut spans = vec![Span::styled(marker, marker_style)];
    if let Some(head) = head {
        spans.push(Span::styled(head, Style::default().fg(p.artist).bg(row_bg)));
        spans.push(Span::styled("  ", base));
    }
    spans.push(Span::styled(pad(track.title.as_ref(), title_w), base));
    spans.push(Span::styled(
        format!(" {dur}"),
        Style::default().fg(p.muted).bg(row_bg),
    ));
    ListItem::new(Line::from(spans)).style(base)
}

/// Artist in front of the title, only on the first row of a group.
/// The name keeps its full length until the title would have nowhere to sit.
fn library_head(
    artist: &str,
    title: &str,
    dur: &str,
    width: usize,
    show_artist: bool,
) -> (Option<String>, usize) {
    let body = width.saturating_sub(2 + 1 + cols(dur));
    if !show_artist || artist.is_empty() || body < 24 {
        return (None, body);
    }
    let title_floor = 16.min(title.chars().count()).min(body / 3).max(8);
    let max_artist = body.saturating_sub(title_floor + 2);
    let shown = if artist.chars().count() <= max_artist {
        artist.to_string()
    } else {
        fit(artist, max_artist)
    };
    let title_w = body.saturating_sub(cols(&shown) + 2);
    (Some(shown), title_w)
}

fn queue_row<'a>(
    track: &'a Track,
    pos: usize,
    playing: bool,
    selected: bool,
    width: usize,
    num_w: usize,
    p: &Palette,
) -> ListItem<'a> {
    let (title_fg, row_bg) = if selected {
        (p.sel_fg, p.sel_bg)
    } else if playing {
        (p.accent_bright, p.panel)
    } else {
        (p.text, p.panel)
    };
    let base = Style::default().fg(title_fg).bg(row_bg);
    let dur = dur_cell(
        &track
            .duration
            .map(fmt_duration)
            .unwrap_or_else(|| "--:--".into()),
    );
    let prefix = if playing {
        format!("{:>num_w$} ", ">")
    } else {
        format!("{:>num_w$} ", pos + 1)
    };
    let title_w = width.saturating_sub(cols(&prefix) + cols(&dur) + 1);
    ListItem::new(Line::from(vec![
        Span::styled(
            prefix,
            Style::default()
                .fg(if playing { p.accent } else { p.muted })
                .bg(row_bg),
        ),
        Span::styled(pad(track.title.as_ref(), title_w), base),
        Span::styled(format!(" {dur}"), Style::default().fg(p.muted).bg(row_bg)),
    ]))
    .style(base)
}

fn glass_block(title: String, focused: bool, p: &Palette) -> Block<'static> {
    let title_style = if focused {
        Style::default()
            .fg(p.accent_bright)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(p.muted)
    };
    let border = if focused { p.accent } else { p.edge };
    Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(border))
        .padding(Padding::new(PAD_LEFT, PAD_RIGHT, 0, 0))
        .style(Style::default().bg(p.panel).fg(p.text))
        .title(Span::styled(title, title_style))
}

fn paint_canvas(frame: &mut Frame, area: Rect, p: &Palette) {
    let buf = frame.buffer_mut();
    for y in area.y..area.bottom() {
        for x in area.x..area.right() {
            let Some(cell) = buf.cell_mut(Position { x, y }) else {
                continue;
            };
            cell.set_symbol(" ");
            cell.set_fg(p.muted);
            cell.set_bg(p.canvas);
            cell.modifier = Modifier::empty();
        }
    }
}

fn draw_hints(frame: &mut Frame, area: Rect, s: &UiState<'_>, p: &Palette) {
    let body = match s.input_mode {
        InputMode::Filter => Paragraph::new(Line::from(vec![
            Span::styled(
                format!(" {} ", T::filter_prompt(s.lang).trim()),
                Style::default()
                    .fg(p.badge_fg)
                    .bg(p.accent)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" "),
            Span::styled(format!("{}_", s.input_buf), Style::default().fg(p.text)),
        ])),
        InputMode::SavePlaylist => Paragraph::new(Line::from(vec![
            Span::styled(
                format!(
                    " {} ",
                    T::save_playlist_prompt(s.lang)
                        .trim_end_matches(": ")
                        .trim()
                ),
                Style::default()
                    .fg(p.badge_fg)
                    .bg(p.accent)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" "),
            Span::styled(format!("{}_", s.input_buf), Style::default().fg(p.text)),
        ])),
        InputMode::Normal if !s.status.is_empty() => Paragraph::new(Span::styled(
            s.status.to_string(),
            Style::default().fg(p.yellow),
        )),
        InputMode::Normal => {
            let hints = match s.focus {
                Focus::Library => T::hints_library(s.lang),
                Focus::Queue => T::hints_queue(s.lang),
                Focus::Playlists => T::hints_playlists(s.lang),
            };
            Paragraph::new(hints).style(Style::default().fg(p.muted))
        }
    };
    frame.render_widget(
        body.block(
            Block::default()
                .borders(Borders::TOP)
                .border_style(Style::default().fg(p.edge))
                .padding(Padding::new(PAD_LEFT, PAD_RIGHT, 0, 0))
                .style(Style::default().bg(p.panel).fg(p.text)),
        ),
        area,
    );
}

fn draw_help(frame: &mut Frame, area: Rect, lang: Lang, p: &Palette) {
    let popup = centered(area, 72, 86);
    frame.render_widget(Clear, popup);
    let mut lines: Vec<Line> = Vec::new();
    lines.push(Line::from(Span::styled(
        T::help_banner(lang),
        Style::default()
            .fg(p.accent_bright)
            .add_modifier(Modifier::BOLD),
    )));
    lines.push(Line::from(""));
    lines.extend(T::help_lines(lang).into_iter().map(|l| {
        if let Some((k, rest)) = l.split_once("  ") {
            Line::from(vec![
                Span::styled(format!("{k:<12}"), Style::default().fg(p.accent)),
                Span::styled(rest.trim_start(), Style::default().fg(p.text)),
            ])
        } else if l.is_empty() {
            Line::from("")
        } else {
            Line::from(Span::styled(l, Style::default().fg(p.muted)))
        }
    }));
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        T::help_dismiss(lang),
        Style::default().fg(p.yellow),
    )));
    frame.render_widget(
        Paragraph::new(lines)
            .wrap(Wrap { trim: false })
            .block(glass_block(format!(" {} ", T::help(lang)), true, p)),
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

fn in_rect(rect: Rect, col: u16, row: u16) -> bool {
    col >= rect.x
        && col < rect.x.saturating_add(rect.width)
        && row >= rect.y
        && row < rect.y.saturating_add(rect.height)
}

fn row_in_list(rect: Rect, col: u16, row: u16, len: usize, sel: usize) -> Option<usize> {
    if len == 0 || rect.width < 3 || rect.height < 3 {
        return None;
    }
    if col <= rect.x || col >= rect.x + rect.width - 1 {
        return None;
    }
    if row <= rect.y || row >= rect.y + rect.height - 1 {
        return None;
    }
    let inner_h = rect.height.saturating_sub(2) as usize;
    let (start, end) = window_range(len, sel, inner_h.max(1));
    let index = start + (row - rect.y - 1) as usize;
    (index < end).then_some(index)
}

fn window_range(len: usize, sel: usize, visible: usize) -> (usize, usize) {
    if len == 0 || visible == 0 {
        return (0, 0);
    }
    let visible = visible.max(1);
    if len <= visible {
        return (0, len);
    }
    let start = sel
        .saturating_sub(visible / 2)
        .min(len.saturating_sub(visible));
    (start, (start + visible).min(len))
}

fn lib_len(s: &UiState<'_>) -> usize {
    if s.filter.is_empty() {
        s.library.tracks.len()
    } else {
        s.filter_idx.len()
    }
}

fn lib_track<'a>(s: &'a UiState<'a>, index: usize) -> &'a Track {
    if s.filter.is_empty() {
        &s.library.tracks[index]
    } else {
        &s.library.tracks[s.filter_idx[index]]
    }
}

/// Right-aligned time. Short values share one column, so the edge stays straight.
fn dur_cell(dur: &str) -> String {
    let slot = cols(dur).max(5);
    let missing = slot - cols(dur);
    let mut out = " ".repeat(missing);
    out.push_str(dur);
    out
}

const PAD_LEFT: u16 = 1;
const PAD_RIGHT: u16 = 3;

fn text_width(area_width: u16) -> usize {
    usize::from(
        area_width
            .saturating_sub(2)
            .saturating_sub(PAD_LEFT)
            .saturating_sub(PAD_RIGHT),
    )
}

fn cols(text: &str) -> usize {
    UnicodeWidthStr::width(text)
}

fn fit(text: &str, width: usize) -> String {
    if width == 0 {
        return String::new();
    }
    if cols(text) <= width {
        return text.to_string();
    }
    let mut out = String::new();
    let mut used = 0;
    for ch in text.chars() {
        let w = UnicodeWidthChar::width(ch).unwrap_or(0);
        if used + w + 1 > width {
            break;
        }
        out.push(ch);
        used += w;
    }
    out.push('…');
    out
}

fn pad(text: &str, width: usize) -> String {
    let clipped = fit(text, width);
    let missing = width.saturating_sub(cols(&clipped));
    let mut out = clipped;
    out.extend(std::iter::repeat(' ').take(missing));
    out
}

#[cfg(test)]
mod tests {
    use super::{cols, fit, library_head, pad};

    #[test]
    fn wide_brackets_do_not_push_the_time_past_the_row() {
        let title = "Opening『CHASE』";
        let fitted = fit(title, 10);
        assert!(cols(&fitted) <= 10);
        let (head, title_w) = library_head("Artist", title, "2:40", 40, true);
        let head = head.unwrap_or_default();
        let row = format!("  {head}  {} {dur}", pad(title, title_w), dur = "2:40");
        assert!(cols(&row) <= 40, "row is {} cols", cols(&row));
    }

    #[test]
    fn long_artist_is_not_cut_at_a_fixed_column() {
        let artist = "Ashnikko, Arcane & League of Legends";
        let (head, title_w) = library_head(artist, "Paint The Town Blue", "1:55", 90, true);
        let head = head.expect("artist");
        assert_eq!(head, artist);
        assert!(title_w >= 16);
    }

    #[test]
    fn continuation_row_gives_the_width_to_the_title() {
        let (head, title_w) = library_head("Eminem", "Mockingbird", "4:10", 90, false);
        assert!(head.is_none());
        assert_eq!(title_w, 90 - 2 - 1 - cols("4:10"));
    }
}

fn fmt_duration(d: Duration) -> String {
    let secs = d.as_secs();
    let hours = secs / 3600;
    let mins = (secs % 3600) / 60;
    let rem = secs % 60;
    if hours > 0 {
        format!("{hours}:{mins:02}:{rem:02}")
    } else {
        format!("{mins}:{rem:02}")
    }
}
