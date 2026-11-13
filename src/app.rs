use std::io::{self, Stdout};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::Result;
use crossterm::event::{
    self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind, KeyModifiers,
    MouseButton, MouseEventKind,
};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::Rect;
use ratatui::Terminal;

/// Set by SIGINT / panic path so the main loop can exit cleanly.
static WANT_QUIT: AtomicBool = AtomicBool::new(false);

use crate::config::{Config, RepeatMode};
use crate::i18n::T;
use crate::library::{Library, Track};
use crate::mpris::{MprisBridge, Snapshot};
use crate::player::{Player, Pulse};
use crate::playlist::{self, Queue};
use crate::ui::{self, Click, Focus, InputMode, UiState};

struct Resume {
    path: PathBuf,
    pos: Duration,
}

struct MprisStamp {
    path: PathBuf,
    playing: bool,
    paused: bool,
    has_track: bool,
    vol: u16,
    bucket: u64,
    cover: String,
}

pub struct App {
    pub config: Config,
    pub library: Library,
    pub queue: Queue,
    pub player: Player,
    pub focus: Focus,
    pub input_mode: InputMode,
    pub input_buf: String,
    pub filter: String,
    filter_backup: String,
    filter_idx: Vec<usize>,
    match_buf: String,
    pub lib_sel: usize,
    pub queue_sel: usize,
    pub pl_sel: usize,
    pub saved_playlists: Vec<PathBuf>,
    pub show_help: bool,
    pub status: String,
    status_seen: String,
    status_at: Option<Instant>,
    screen: Rect,
    last_click: Option<(u16, u16, Instant)>,
    pub should_quit: bool,
    pending_delete: Option<PathBuf>,
    track_heard: bool,
    resume: Option<Resume>,
    cover_url: String,
    cover_key: Option<(Arc<str>, Arc<str>)>,
    pulse: Pulse,
    mpris_stamp: Option<MprisStamp>,
    mpris: Option<MprisBridge>,
}

impl App {
    pub fn new() -> Result<Self> {
        let config = Config::load()?;
        let library = Library::scan(&config.music_dir)?;
        let player = Player::new(config.volume)?;
        let saved_playlists = playlist::list_saved_playlists().unwrap_or_default();
        let mpris = MprisBridge::start();
        let show_help = !config.help_seen;

        let mut queue = Queue::default();
        let mut queue_sel = 0;
        let mut resume = None;
        if let Some((tracks, index, pos)) = playlist::load_session(&library.tracks) {
            queue.replace(tracks);
            let _ = queue.play_at(index);
            queue_sel = queue.play_pos_of_index(queue.index.unwrap_or(0));
            if pos > Duration::from_secs(2) {
                if let Some(track) = queue.current() {
                    resume = Some(Resume {
                        path: track.path.clone(),
                        pos,
                    });
                }
            }
        }

        Ok(Self {
            config,
            library,
            queue,
            player,
            focus: Focus::Library,
            input_mode: InputMode::Normal,
            input_buf: String::new(),
            filter: String::new(),
            filter_backup: String::new(),
            filter_idx: Vec::new(),
            match_buf: String::new(),
            lib_sel: 0,
            queue_sel,
            pl_sel: 0,
            saved_playlists,
            show_help,
            status: String::new(),
            status_seen: String::new(),
            status_at: None,
            screen: Rect::default(),
            last_click: None,
            should_quit: false,
            pending_delete: None,
            track_heard: false,
            resume,
            cover_url: String::new(),
            cover_key: None,
            pulse: Pulse::default(),
            mpris_stamp: None,
            mpris,
        })
    }

    pub fn run(&mut self) -> Result<()> {
        install_quit_hooks();

        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend)?;
        let mut guard = TerminalGuard {
            terminal: Some(&mut terminal),
            active: true,
        };

        let tick = Duration::from_millis(50);
        let mut dirty = true;
        let mut drawn_pos = Duration::ZERO;
        while !self.should_quit && !WANT_QUIT.load(Ordering::Relaxed) {
            if let Ok(size) = guard.terminal.as_ref().unwrap().size() {
                self.screen = Rect {
                    x: 0,
                    y: 0,
                    width: size.width,
                    height: size.height,
                };
            }
            if self.status != self.status_seen {
                self.status_seen.clone_from(&self.status);
                self.status_at = Some(Instant::now());
            }
            if self.pending_delete.is_none()
                && !self.status.is_empty()
                && self
                    .status_at
                    .is_some_and(|at| at.elapsed() >= Duration::from_secs(4))
            {
                self.status.clear();
                self.status_seen.clear();
                self.status_at = None;
                dirty = true;
            }

            self.pulse = self.player.pulse();
            if self.pulse.has_track {
                if !self.pulse.empty {
                    self.track_heard = true;
                } else if self.track_heard && !self.pulse.paused {
                    self.track_heard = false;
                    self.on_track_ended();
                    self.pulse = self.player.pulse();
                    dirty = true;
                }
            }

            let pending: Vec<souvlaki::MediaControlEvent> = self
                .mpris
                .as_ref()
                .map(|m| std::iter::from_fn(|| m.events.try_recv().ok()).collect())
                .unwrap_or_default();
            if !pending.is_empty() {
                for ev in pending {
                    self.handle_mpris(ev);
                }
                self.pulse = self.player.pulse();
                dirty = true;
            }

            self.publish_mpris();
            let pos_tick = self.pulse.playing
                && self.pulse.position.abs_diff(drawn_pos) >= Duration::from_millis(200);
            if dirty || pos_tick {
                guard
                    .terminal
                    .as_mut()
                    .unwrap()
                    .draw(|f| ui::draw(f, self))?;
                dirty = false;
                drawn_pos = self.pulse.position;
            }

            if event::poll(tick)? {
                loop {
                    match event::read()? {
                        Event::Key(key) if key.kind == KeyEventKind::Press => {
                            self.handle_key(key.code, key.modifiers);
                            dirty = true;
                        }
                        Event::Mouse(m) => {
                            self.handle_mouse(m.kind, m.column, m.row);
                            dirty = true;
                        }
                        _ => {}
                    }
                    if self.should_quit || WANT_QUIT.load(Ordering::Relaxed) {
                        break;
                    }
                    if !event::poll(Duration::from_millis(0))? {
                        break;
                    }
                }
            }
        }

        self.shutdown_playback();
        guard.restore();
        Ok(())
    }

    fn shutdown_playback(&mut self) {
        let pos = self.player.pulse().position;
        let _ = playlist::save_session(&self.library.tracks, &self.queue, pos);
        self.player.stop();
        if let Some(mut m) = self.mpris.take() {
            m.shutdown();
        }
        self.config.volume = self.player.pulse().volume;
        let _ = self.config.save();
    }

    fn handle_mpris(&mut self, ev: souvlaki::MediaControlEvent) {
        use souvlaki::MediaControlEvent::*;
        match ev {
            Toggle => self.toggle_play(),
            Play => {
                if self.pulse.paused {
                    self.player.toggle_pause();
                } else if !self.pulse.has_track {
                    self.play_current_or_selected();
                }
            }
            Pause => {
                if self.pulse.playing {
                    self.player.toggle_pause();
                }
            }
            Next => self.next_track(),
            Previous => self.prev_track(),
            Stop => {
                self.player.stop();
                self.track_heard = false;
            }
            SeekBy(dir, delta) => {
                let forward = dir == souvlaki::SeekDirection::Forward;
                self.seek_by(delta, forward);
            }
            SetPosition(souvlaki::MediaPosition(pos)) => {
                let dur = self.queue.current().and_then(|t| t.duration);
                if self.player.seek_to(pos, dur).is_err() {
                    self.status = T::seek_failed(self.config.lang).into();
                }
            }
            SetVolume(vol) => {
                self.player.set_volume(vol as f32);
                self.note_volume();
            }
            Quit => self.should_quit = true,
            _ => {}
        }
    }

    fn seek_by(&mut self, delta: Duration, forward: bool) {
        let dur = self.queue.current().and_then(|t| t.duration);
        if self.player.seek_by(delta, forward, dur).is_err() {
            self.status = T::seek_failed(self.config.lang).into();
        }
    }

    fn publish_mpris(&mut self) {
        if self.mpris.is_none() {
            return;
        }
        let path = self.queue.current().map(|t| t.path.as_path());
        let vol = (self.pulse.volume * 100.0).round() as u16;
        let bucket = (self.pulse.position.as_millis() as u64) / 500;
        if let Some(stamp) = &self.mpris_stamp {
            let same_path = match path {
                Some(p) => stamp.path.as_path() == p,
                None => stamp.path.as_os_str().is_empty(),
            };
            if same_path
                && stamp.playing == self.pulse.playing
                && stamp.paused == self.pulse.paused
                && stamp.has_track == self.pulse.has_track
                && stamp.vol == vol
                && stamp.bucket == bucket
                && stamp.cover == self.cover_url
            {
                return;
            }
        }

        let (title, artist, album, duration, path_buf) = match self.queue.current() {
            Some(t) => (
                t.title.to_string(),
                t.artist.to_string(),
                t.album.to_string(),
                t.duration,
                t.path.clone(),
            ),
            None => (
                String::new(),
                String::new(),
                String::new(),
                None,
                PathBuf::new(),
            ),
        };
        let snap = Snapshot {
            title,
            artist,
            album,
            path: path_buf.display().to_string(),
            cover: self.cover_url.clone(),
            duration,
            position: self.pulse.position,
            playing: self.pulse.playing,
            paused: self.pulse.paused,
            has_track: self.pulse.has_track,
            volume: self.pulse.volume,
        };
        if let Some(m) = &self.mpris {
            m.publish(snap);
        }
        self.mpris_stamp = Some(MprisStamp {
            path: path_buf,
            playing: self.pulse.playing,
            paused: self.pulse.paused,
            has_track: self.pulse.has_track,
            vol,
            bucket,
            cover: self.cover_url.clone(),
        });
    }

    fn handle_mouse(&mut self, kind: MouseEventKind, col: u16, row: u16) {
        if self.input_mode != InputMode::Normal || self.show_help {
            return;
        }
        match kind {
            MouseEventKind::ScrollUp => self.move_sel(-1),
            MouseEventKind::ScrollDown => self.move_sel(1),
            MouseEventKind::Down(MouseButton::Left) => {
                let hit = {
                    let state = self.ui_state();
                    ui::click_target(self.screen, col, row, &state)
                };
                let now = Instant::now();
                let double = self.last_click.is_some_and(|(c, r, at)| {
                    c == col && r == row && now.duration_since(at) < Duration::from_millis(400)
                });
                self.last_click = Some((col, row, now));
                match hit {
                    Click::Seek(ratio) => {
                        if let Some(dur) = self.queue.current().and_then(|t| t.duration) {
                            let pos = dur.mul_f64(ratio.clamp(0.0, 1.0));
                            if self.player.seek_to(pos, Some(dur)).is_err() {
                                self.status = T::seek_failed(self.config.lang).into();
                            }
                        }
                    }
                    Click::Library(index) => {
                        self.focus = Focus::Library;
                        self.lib_sel = index;
                        if double {
                            self.activate_selected();
                        }
                    }
                    Click::Queue(index) => {
                        self.focus = Focus::Queue;
                        self.queue_sel = index;
                        if double {
                            self.activate_selected();
                        }
                    }
                    Click::Playlist(index) => {
                        self.focus = Focus::Playlists;
                        self.pl_sel = index;
                        if double {
                            self.activate_selected();
                        }
                    }
                    Click::Panel(focus) => self.focus = focus,
                }
            }
            _ => {}
        }
    }

    fn cycle_focus(&mut self, back: bool) {
        let hide_playlists = ui::shell(self.screen).playlists.is_none();
        for _ in 0..3 {
            self.focus = if back {
                self.focus.prev()
            } else {
                self.focus.next()
            };
            if !(hide_playlists && self.focus == Focus::Playlists) {
                break;
            }
        }
    }

    fn jump_to_current(&mut self) {
        if self.queue.index.is_some() {
            self.focus = Focus::Queue;
            self.follow_current();
        }
    }

    fn follow_current(&mut self) {
        if let Some(index) = self.queue.index {
            self.queue_sel = self.queue.play_pos_of_index(index);
        }
    }

    fn lib_len(&self) -> usize {
        if self.filter.is_empty() {
            self.library.tracks.len()
        } else {
            self.filter_idx.len()
        }
    }

    fn lib_track(&self, index: usize) -> Option<&Arc<Track>> {
        if self.filter.is_empty() {
            self.library.tracks.get(index)
        } else {
            self.filter_idx
                .get(index)
                .and_then(|&i| self.library.tracks.get(i))
        }
    }

    fn recompute_filter(&mut self) {
        self.filter_idx.clear();
        if self.filter.is_empty() {
            self.filter_idx.shrink_to_fit();
            return;
        }
        let needle = self.filter.to_lowercase();
        let mut buf = std::mem::take(&mut self.match_buf);
        for (i, track) in self.library.tracks.iter().enumerate() {
            if track.matches_lower(&needle, &mut buf) {
                self.filter_idx.push(i);
            }
        }
        self.match_buf = buf;
    }

    fn handle_key(&mut self, code: KeyCode, mods: KeyModifiers) {
        if self.show_help {
            if matches!(code, KeyCode::Char('q' | 'Q' | 'й' | 'Й')) {
                self.should_quit = true;
                return;
            }
            if matches!(
                code,
                KeyCode::Char('?')
                    | KeyCode::Char(',')
                    | KeyCode::Esc
                    | KeyCode::Enter
                    | KeyCode::Char(' ')
            ) {
                self.show_help = false;
                if !self.config.help_seen {
                    self.config.help_seen = true;
                    let _ = self.config.save();
                }
            }
            return;
        }

        match self.input_mode {
            InputMode::Filter => {
                self.handle_filter_input(code);
                return;
            }
            InputMode::SavePlaylist => {
                self.handle_save_input(code);
                return;
            }
            InputMode::Normal => {}
        }

        let deleting = matches!(code, KeyCode::Char('d' | 'D' | 'в' | 'В'));
        if !deleting {
            self.pending_delete = None;
        }

        match code {
            KeyCode::Char('c') if mods.contains(KeyModifiers::CONTROL) => self.should_quit = true,
            KeyCode::Char('q' | 'Q' | 'й' | 'Й') => self.should_quit = true,
            KeyCode::Char('?' | ',') => self.show_help = true,
            KeyCode::BackTab => self.cycle_focus(true),
            KeyCode::Tab => self.cycle_focus(false),
            KeyCode::Char('\'' | 'э' | 'Э') => self.jump_to_current(),
            KeyCode::Char(' ') => self.toggle_play(),
            KeyCode::Char('n' | 'N' | 'т' | 'Т') => self.next_track(),
            KeyCode::Char('p' | 'P' | 'з' | 'З') if !mods.contains(KeyModifiers::CONTROL) => {
                self.prev_track()
            }
            KeyCode::Char('s' | 'ы') => self.toggle_shuffle(),
            KeyCode::Char('S' | 'Ы') => {
                self.input_mode = InputMode::SavePlaylist;
                self.input_buf.clear();
            }
            KeyCode::Char('r' | 'к') => self.cycle_repeat(),
            KeyCode::Char('R' | 'К') | KeyCode::F(5) => self.rescan(),
            KeyCode::Char('a' | 'A' | 'ф' | 'Ф') => self.add_selected_to_queue(),
            KeyCode::Char('c' | 'с') => self.clear_queue(),
            KeyCode::Char('x' | 'X' | 'ч' | 'Ч') | KeyCode::Delete
                if self.focus == Focus::Queue =>
            {
                self.remove_selected_queue();
            }
            KeyCode::Char('l' | 'д') => self.load_selected_playlist(),
            KeyCode::Char('d' | 'D' | 'в' | 'В') => self.delete_selected_playlist(),
            KeyCode::Char('/' | '.') => {
                self.input_mode = InputMode::Filter;
                self.filter_backup.clone_from(&self.filter);
                self.input_buf.clone_from(&self.filter);
                self.focus = Focus::Library;
            }
            KeyCode::Char('L' | 'Д') => {
                self.config.lang = self.config.lang.toggle();
                let _ = self.config.save();
            }
            KeyCode::Char('t' | 'T' | 'е' | 'Е') => {
                self.config.theme = self.config.theme.next();
                let _ = self.config.save();
                self.status = T::theme(self.config.lang, self.config.theme.label());
            }
            KeyCode::Char('-' | '_') => {
                self.player.adjust_volume(-0.05);
                self.note_volume();
            }
            KeyCode::Char('=' | '+') => {
                self.player.adjust_volume(0.05);
                self.note_volume();
            }
            KeyCode::Left => self.seek_by(Duration::from_secs(5), false),
            KeyCode::Right => self.seek_by(Duration::from_secs(5), true),
            KeyCode::Up if mods.contains(KeyModifiers::SHIFT) => self.move_selected_queue(-1),
            KeyCode::Down if mods.contains(KeyModifiers::SHIFT) => self.move_selected_queue(1),
            KeyCode::Up | KeyCode::Char('k' | 'л' | 'Л') => self.move_sel(-1),
            KeyCode::Down | KeyCode::Char('j' | 'о' | 'О') => self.move_sel(1),
            KeyCode::Enter => self.activate_selected(),
            KeyCode::PageUp => self.move_sel(-10),
            KeyCode::PageDown => self.move_sel(10),
            KeyCode::Home => self.set_sel(0),
            KeyCode::End => self.set_sel(usize::MAX),
            _ => {}
        }
    }

    fn note_volume(&mut self) {
        let pct = (self.player.pulse().volume * 100.0).round() as u8;
        self.status = T::volume(self.config.lang, pct);
    }

    fn handle_filter_input(&mut self, code: KeyCode) {
        match code {
            KeyCode::Esc => {
                self.filter.clone_from(&self.filter_backup);
                self.recompute_filter();
                self.lib_sel = self.lib_sel.min(self.lib_len().saturating_sub(1));
                self.input_mode = InputMode::Normal;
                self.input_buf.clear();
            }
            KeyCode::Enter => {
                self.input_mode = InputMode::Normal;
                self.input_buf.clear();
            }
            KeyCode::Backspace => {
                self.input_buf.pop();
                self.apply_live_filter();
            }
            KeyCode::Char(c) => {
                self.input_buf.push(c);
                self.apply_live_filter();
            }
            _ => {}
        }
    }

    fn apply_live_filter(&mut self) {
        self.filter.clone_from(&self.input_buf);
        self.recompute_filter();
        self.lib_sel = 0;
    }

    fn handle_save_input(&mut self, code: KeyCode) {
        match code {
            KeyCode::Esc => {
                self.input_mode = InputMode::Normal;
                self.input_buf.clear();
            }
            KeyCode::Enter => {
                let name = self.input_buf.trim().to_string();
                if !name.is_empty() {
                    let pl = playlist::Playlist::temporary(self.queue.tracks.clone());
                    match pl.save_m3u(&name) {
                        Ok(path) => {
                            self.status = T::saved(self.config.lang, &path.display().to_string());
                            self.saved_playlists =
                                playlist::list_saved_playlists().unwrap_or_default();
                        }
                        Err(e) => {
                            self.status = T::save_failed(self.config.lang, &e.to_string());
                        }
                    }
                }
                self.input_mode = InputMode::Normal;
                self.input_buf.clear();
            }
            KeyCode::Backspace => {
                self.input_buf.pop();
            }
            KeyCode::Char(c) => self.input_buf.push(c),
            _ => {}
        }
    }

    fn move_sel(&mut self, delta: isize) {
        match self.focus {
            Focus::Library => {
                let len = self.lib_len();
                self.lib_sel = move_index(self.lib_sel, delta, len);
            }
            Focus::Queue => {
                let len = self.queue.play_len();
                self.queue_sel = move_index(self.queue_sel, delta, len);
            }
            Focus::Playlists => {
                let len = self.saved_playlists.len();
                self.pl_sel = move_index(self.pl_sel, delta, len);
            }
        }
    }

    fn set_sel(&mut self, idx: usize) {
        match self.focus {
            Focus::Library => {
                let len = self.lib_len();
                self.lib_sel = if len == 0 { 0 } else { idx.min(len - 1) };
            }
            Focus::Queue => {
                let len = self.queue.play_len();
                self.queue_sel = if len == 0 { 0 } else { idx.min(len - 1) };
            }
            Focus::Playlists => {
                let len = self.saved_playlists.len();
                self.pl_sel = if len == 0 { 0 } else { idx.min(len - 1) };
            }
        }
    }

    fn activate_selected(&mut self) {
        match self.focus {
            Focus::Library => self.play_view_from_selection(),
            Focus::Queue => {
                if let Some(track_i) = self.queue.track_index_at_play(self.queue_sel) {
                    if let Some(t) = self.queue.play_at(track_i).cloned() {
                        let _ = self.play_track(&t);
                        self.follow_current();
                    }
                }
            }
            Focus::Playlists => self.load_selected_playlist(),
        }
    }

    fn play_view_from_selection(&mut self) {
        let len = self.lib_len();
        if len == 0 {
            self.status = T::empty_library(self.config.lang).into();
            return;
        }
        let idx = self.lib_sel.min(len - 1);
        let tracks = self.view_tracks();
        self.queue.replace(tracks);
        self.queue_sel = idx;
        if let Some(t) = self.queue.play_at(idx).cloned() {
            let _ = self.play_track(&t);
        }
    }

    fn view_tracks(&self) -> Vec<Arc<Track>> {
        if self.filter.is_empty() {
            self.library.tracks.clone()
        } else {
            self.filter_idx
                .iter()
                .filter_map(|&i| self.library.tracks.get(i).cloned())
                .collect()
        }
    }

    fn add_selected_to_queue(&mut self) {
        if self.focus != Focus::Library {
            return;
        }
        let Some(track) = self.lib_track(self.lib_sel).cloned() else {
            return;
        };
        self.status = T::added(self.config.lang, &track.display_name());
        self.queue.append(track);
    }

    fn clear_queue(&mut self) {
        self.queue.clear();
        self.queue_sel = 0;
        self.player.stop();
        self.track_heard = false;
        self.cover_url.clear();
        self.cover_key = None;
        clear_cover_files();
        self.status = T::queue_cleared(self.config.lang).into();
    }

    fn remove_selected_queue(&mut self) {
        let Some(track_i) = self.queue.track_index_at_play(self.queue_sel) else {
            return;
        };
        let was_current = self.queue.index == Some(track_i);
        let play_pos = self.queue_sel;
        if self.queue.remove_at(track_i).is_none() {
            return;
        }
        if self.queue.tracks.is_empty() {
            self.clear_queue();
            return;
        }
        self.status = T::track_removed(self.config.lang).into();
        if was_current {
            let pos = play_pos.min(self.queue.play_len().saturating_sub(1));
            if let Some(index) = self.queue.track_index_at_play(pos) {
                if let Some(track) = self.queue.select_index(index).cloned() {
                    self.queue_sel = pos;
                    let _ = self.play_track(&track);
                    self.follow_current();
                }
            }
        } else {
            self.queue_sel = play_pos.min(self.queue.play_len().saturating_sub(1));
        }
    }

    fn move_selected_queue(&mut self, delta: isize) {
        if self.focus != Focus::Queue {
            return;
        }
        if let Some(to) = self.queue.move_play_pos(self.queue_sel, delta) {
            self.queue_sel = to;
        }
    }

    fn toggle_play(&mut self) {
        if self.pulse.has_track {
            self.player.toggle_pause();
        } else {
            self.play_current_or_selected();
        }
    }

    fn play_current_or_selected(&mut self) {
        if let Some(t) = self.queue.current().cloned() {
            let _ = self.play_track(&t);
            return;
        }
        if !self.queue.tracks.is_empty() {
            if let Some(t) = self.queue.play_at(0).cloned() {
                let _ = self.play_track(&t);
            }
            return;
        }
        self.play_view_from_selection();
    }

    fn play_track(&mut self, track: &Track) -> Result<()> {
        match self.player.play_file(&track.path) {
            Ok(()) => {
                // The sink is queued immediately, so an empty sink later means the track ended.
                self.track_heard = true;
                if self
                    .resume
                    .as_ref()
                    .is_some_and(|resume| resume.path == track.path)
                {
                    if let Some(resume) = self.resume.take() {
                        let _ = self.player.seek_to(resume.pos, track.duration);
                    }
                } else {
                    self.resume = None;
                }
                self.refresh_cover(track);
                Ok(())
            }
            Err(e) => {
                self.track_heard = false;
                self.status = T::play_error(self.config.lang, &e.to_string());
                Err(e)
            }
        }
    }

    fn refresh_cover(&mut self, track: &Track) {
        let key = if track.album.is_empty() {
            None
        } else {
            Some((Arc::clone(&track.artist), Arc::clone(&track.album)))
        };
        if key.is_some() && key == self.cover_key && !self.cover_url.is_empty() {
            return;
        }
        self.cover_key = key;
        if let Some((bytes, ext)) = crate::library::read_cover(&track.path) {
            if let Some(url) = store_cover(&bytes, ext) {
                self.cover_url = url;
                return;
            }
        }
        self.cover_url.clear();
        clear_cover_files();
    }

    fn on_track_ended(&mut self) {
        for _ in 0..8 {
            let repeat_one = self.config.repeat == RepeatMode::One;
            let repeat_all = self.config.repeat == RepeatMode::All;
            let Some(track) = self.queue.advance(repeat_one, repeat_all).cloned() else {
                self.player.stop();
                self.track_heard = false;
                return;
            };
            self.follow_current();
            if self.play_track(&track).is_ok() {
                return;
            }
        }
        self.player.stop();
        self.track_heard = false;
    }

    fn next_track(&mut self) {
        let repeat_all = self.config.repeat != RepeatMode::Off;
        if let Some(t) = self.queue.advance(false, repeat_all).cloned() {
            let _ = self.play_track(&t);
            self.follow_current();
        }
    }

    fn prev_track(&mut self) {
        if self.pulse.position > Duration::from_secs(3) {
            if let Some(t) = self.queue.current().cloned() {
                let _ = self.play_track(&t);
            }
            return;
        }
        if let Some(t) = self.queue.retreat().cloned() {
            let _ = self.play_track(&t);
            self.follow_current();
        }
    }

    fn toggle_shuffle(&mut self) {
        let on = !self.queue.shuffle;
        self.queue.set_shuffle(on);
        self.status = if on {
            T::shuffle_on(self.config.lang)
        } else {
            T::shuffle_off(self.config.lang)
        }
        .into();
    }

    fn cycle_repeat(&mut self) {
        self.config.repeat = self.config.repeat.next();
        let _ = self.config.save();
        self.status = match self.config.repeat {
            RepeatMode::Off => T::repeat_off(self.config.lang),
            RepeatMode::All => T::repeat_all(self.config.lang),
            RepeatMode::One => T::repeat_one(self.config.lang),
        }
        .into();
    }

    fn load_selected_playlist(&mut self) {
        let Some(path) = self.saved_playlists.get(self.pl_sel).cloned() else {
            return;
        };
        match playlist::Playlist::from_m3u(&path, &self.library.tracks) {
            Ok(pl) => {
                let name = pl.name.clone();
                self.queue.replace(pl.tracks);
                self.queue_sel = 0;
                self.follow_current();
                self.focus = Focus::Queue;
                self.status = T::loaded(self.config.lang, &name);
                if let Some(t) = self.queue.current().cloned() {
                    let _ = self.play_track(&t);
                }
            }
            Err(e) => self.status = T::load_failed(self.config.lang, &e.to_string()),
        }
    }

    fn delete_selected_playlist(&mut self) {
        if self.focus != Focus::Playlists {
            self.pending_delete = None;
            return;
        }
        let Some(path) = self.saved_playlists.get(self.pl_sel).cloned() else {
            return;
        };
        if self.pending_delete.as_deref() != Some(path.as_path()) {
            let name = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("playlist");
            self.status = T::delete_confirm(self.config.lang, name);
            self.pending_delete = Some(path);
            return;
        }
        self.pending_delete = None;
        match playlist::delete_playlist(&path) {
            Ok(()) => {
                let name = path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("playlist");
                self.status = T::deleted(self.config.lang, name);
                self.saved_playlists = playlist::list_saved_playlists().unwrap_or_default();
                if self.pl_sel > 0 && self.pl_sel >= self.saved_playlists.len() {
                    self.pl_sel -= 1;
                }
            }
            Err(e) => self.status = T::delete_failed(self.config.lang, &e.to_string()),
        }
    }

    fn rescan(&mut self) {
        match Library::rescan(&self.config.music_dir) {
            Ok(lib) => {
                let n = lib.tracks.len();
                self.library = lib;
                self.lib_sel = 0;
                self.recompute_filter();
                self.status = T::scanned(self.config.lang, n);
            }
            Err(e) => self.status = T::load_failed(self.config.lang, &e.to_string()),
        }
    }

    pub fn ui_state(&self) -> UiState<'_> {
        UiState {
            lang: self.config.lang,
            theme: self.config.theme,
            focus: self.focus,
            input_mode: self.input_mode,
            input_buf: &self.input_buf,
            filter: &self.filter,
            filter_idx: &self.filter_idx,
            library: &self.library,
            queue: &self.queue,
            lib_sel: self.lib_sel,
            queue_sel: self.queue_sel,
            pl_sel: self.pl_sel,
            playlists: &self.saved_playlists,
            player_playing: self.pulse.playing,
            player_paused: self.pulse.paused,
            position: self.pulse.position,
            volume: self.pulse.volume,
            repeat: self.config.repeat,
            status: &self.status,
            music_dir: &self.config.music_dir,
            show_help: self.show_help,
        }
    }
}

fn file_url(path: &Path) -> String {
    let mut url = String::from("file://");
    for byte in path.to_string_lossy().as_bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'/' | b'-' | b'_' | b'.' | b'~' => {
                url.push(*byte as char);
            }
            _ => url.push_str(&format!("%{byte:02X}")),
        }
    }
    url
}

fn store_cover(bytes: &[u8], ext: &str) -> Option<String> {
    let dir = dirs::cache_dir()?.join("coda");
    std::fs::create_dir_all(&dir).ok()?;
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let path = dir.join(format!("cover-{stamp}.{ext}"));
    std::fs::write(&path, bytes).ok()?;
    clear_cover_files_except(&path);
    Some(file_url(&path))
}

fn clear_cover_files() {
    clear_cover_files_except(Path::new(""));
}

fn clear_cover_files_except(keep: &Path) {
    let Some(dir) = dirs::cache_dir().map(|d| d.join("coda")) else {
        return;
    };
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path == keep {
            continue;
        }
        let name = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
        if name.starts_with("cover-") {
            let _ = std::fs::remove_file(path);
        }
    }
}

fn move_index(cur: usize, delta: isize, len: usize) -> usize {
    if len == 0 {
        return 0;
    }
    let cur = cur as isize;
    let next = cur + delta;
    if next < 0 {
        0
    } else if next as usize >= len {
        len - 1
    } else {
        next as usize
    }
}

struct TerminalGuard<'a> {
    terminal: Option<&'a mut Terminal<CrosstermBackend<Stdout>>>,
    active: bool,
}

impl TerminalGuard<'_> {
    fn restore(&mut self) {
        if !self.active {
            return;
        }
        self.active = false;
        let _ = disable_raw_mode();
        if let Some(t) = self.terminal.as_mut() {
            let _ = execute!(t.backend_mut(), LeaveAlternateScreen, DisableMouseCapture);
            let _ = t.show_cursor();
        }
    }
}

impl Drop for TerminalGuard<'_> {
    fn drop(&mut self) {
        self.restore();
    }
}

fn restore_terminal_best_effort() {
    let _ = disable_raw_mode();
    let mut out = io::stdout();
    let _ = execute!(out, LeaveAlternateScreen, DisableMouseCapture);
}

fn install_quit_hooks() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        let default = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            restore_terminal_best_effort();
            default(info);
        }));
        let _ = ctrlc::set_handler(|| {
            WANT_QUIT.store(true, Ordering::Relaxed);
        });
    });
}
