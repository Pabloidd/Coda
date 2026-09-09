use std::io;
use std::path::PathBuf;
use std::time::Duration;

use anyhow::Result;
use crossterm::event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind, KeyModifiers, MouseEventKind};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;

use crate::config::{Config, RepeatMode};
use crate::i18n::T;
use crate::library::{Library, Track};
use crate::mpris::{MprisBridge, Snapshot};
use crate::player::Player;
use crate::playlist::{self, Queue};
use crate::ui::{self, Focus, InputMode, UiState};

pub struct App {
    pub config: Config,
    pub library: Library,
    pub queue: Queue,
    pub player: Player,
    pub focus: Focus,
    pub input_mode: InputMode,
    pub input_buf: String,
    pub filter: String,
    pub lib_sel: usize,
    pub queue_sel: usize,
    pub pl_sel: usize,
    pub saved_playlists: Vec<PathBuf>,
    pub show_help: bool,
    pub status: String,
    pub should_quit: bool,
    mpris: Option<MprisBridge>,
}

impl App {
    pub fn new() -> Result<Self> {
        let config = Config::load()?;
        let library = Library::scan(&config.music_dir)?;
        let player = Player::new(config.volume)?;
        let saved_playlists = playlist::list_saved_playlists().unwrap_or_default();
        let mpris = MprisBridge::start();

        // First launch: open the keymap overlay so controls are obvious.
        let show_help = !config.help_seen;

        Ok(Self {
            config,
            library,
            queue: Queue::default(),
            player,
            focus: Focus::Library,
            input_mode: InputMode::Normal,
            input_buf: String::new(),
            filter: String::new(),
            lib_sel: 0,
            queue_sel: 0,
            pl_sel: 0,
            saved_playlists,
            show_help,
            status: String::new(),
            should_quit: false,
            mpris,
        })
    }

    pub fn run(&mut self) -> Result<()> {
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend)?;

        let tick = Duration::from_millis(50);
        while !self.should_quit {
            if let Some(ref m) = self.mpris {
                let pending: Vec<_> = std::iter::from_fn(|| m.events.try_recv().ok()).collect();
                for ev in pending {
                    self.handle_mpris(ev);
                }
            }

            if self.player.has_track()
                && self.player.is_empty()
                && !self.player.is_paused()
                && self.player.position() > Duration::from_millis(500)
            {
                self.on_track_ended();
            }

            self.publish_mpris();
            terminal.draw(|f| ui::draw(f, self))?;

            if event::poll(tick)? {
                loop {
                    match event::read()? {
                        // Ignore key Repeat — Space would hammer play/pause and freeze the UI.
                        Event::Key(key) if key.kind == KeyEventKind::Press => {
                            self.handle_key(key.code, key.modifiers);
                        }
                        Event::Mouse(m) => self.handle_mouse(m.kind),
                        _ => {}
                    }
                    if !event::poll(Duration::from_millis(0))? {
                        break;
                    }
                }
            }
        }

        self.config.volume = self.player.volume();
        let _ = self.config.save();

        disable_raw_mode()?;
        execute!(
            terminal.backend_mut(),
            LeaveAlternateScreen,
            DisableMouseCapture
        )?;
        Ok(())
    }

    fn handle_mpris(&mut self, ev: souvlaki::MediaControlEvent) {
        use souvlaki::MediaControlEvent::*;
        match ev {
            Toggle => self.toggle_play(),
            Play => {
                if self.player.is_paused() {
                    self.player.toggle_pause();
                } else if !self.player.has_track() {
                    self.play_current_or_selected();
                }
            }
            Pause => {
                if self.player.is_playing() {
                    self.player.toggle_pause();
                }
            }
            Next => self.next_track(),
            Previous => self.prev_track(),
            Stop => self.player.stop(),
            _ => {}
        }
    }

    fn publish_mpris(&self) {
        let Some(ref m) = self.mpris else { return };
        let (title, artist, album, duration) = match self.queue.current() {
            Some(t) => (
                t.title.clone(),
                t.artist.clone(),
                t.album.clone(),
                t.duration,
            ),
            None => (String::new(), String::new(), String::new(), None),
        };
        m.publish(Snapshot {
            title,
            artist,
            album,
            duration,
            position: self.player.position(),
            playing: self.player.is_playing(),
            paused: self.player.is_paused(),
            has_track: self.player.has_track(),
        });
    }

    fn handle_mouse(&mut self, kind: MouseEventKind) {
        if self.input_mode != InputMode::Normal || self.show_help {
            return;
        }
        match kind {
            MouseEventKind::ScrollUp => self.move_sel(-1),
            MouseEventKind::ScrollDown => self.move_sel(1),
            _ => {}
        }
    }

    fn filtered_tracks(&self) -> Vec<&Track> {
        self.library.filtered(&self.filter)
    }

    fn handle_key(&mut self, code: KeyCode, mods: KeyModifiers) {
        if self.show_help {
            if matches!(
                code,
                KeyCode::Char('?')
                    | KeyCode::Char(',') // RU: Shift+/ → ,
                    | KeyCode::Esc
                    | KeyCode::Char('q')
                    | KeyCode::Char('й')
                    | KeyCode::Char('Й')
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

        // Letters: EN + RU (ЙЦУКЕН) so controls work on both layouts.
        match code {
            KeyCode::Char('q' | 'Q' | 'й' | 'Й') => self.should_quit = true,
            KeyCode::Char('?' | ',') => self.show_help = true,
            KeyCode::Tab => self.focus = self.focus.next(),
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
            KeyCode::Char('c' | 'с') => {
                self.queue.clear();
                self.queue_sel = 0;
                self.status = "queue cleared".into();
            }
            KeyCode::Char('l' | 'д') => self.load_selected_playlist(),
            KeyCode::Char('d' | 'D' | 'в' | 'В') => self.delete_selected_playlist(),
            KeyCode::Char('/' | '.') => {
                // RU layout: physical "/" key often types "."
                self.input_mode = InputMode::Filter;
                self.input_buf = self.filter.clone();
                self.focus = Focus::Library;
            }
            KeyCode::Char('L' | 'Д') => {
                self.config.lang = self.config.lang.toggle();
                let _ = self.config.save();
            }
            KeyCode::Char('-' | '_') => {
                self.player.adjust_volume(-0.05);
                self.status = format!("vol {:.0}%", self.player.volume() * 100.0);
            }
            KeyCode::Char('=' | '+') => {
                self.player.adjust_volume(0.05);
                self.status = format!("vol {:.0}%", self.player.volume() * 100.0);
            }
            KeyCode::Left => {
                let dur = self.queue.current().and_then(|t| t.duration);
                let _ = self.player.seek_rel(-5, dur);
            }
            KeyCode::Right => {
                let dur = self.queue.current().and_then(|t| t.duration);
                let _ = self.player.seek_rel(5, dur);
            }
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

    fn handle_filter_input(&mut self, code: KeyCode) {
        match code {
            KeyCode::Esc => {
                self.input_mode = InputMode::Normal;
                self.input_buf.clear();
            }
            KeyCode::Enter => {
                self.filter = self.input_buf.clone();
                self.lib_sel = 0;
                self.input_mode = InputMode::Normal;
            }
            KeyCode::Backspace => {
                self.input_buf.pop();
            }
            KeyCode::Char(c) => self.input_buf.push(c),
            _ => {}
        }
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
                            self.status = format!("saved {}", path.display());
                            self.saved_playlists =
                                playlist::list_saved_playlists().unwrap_or_default();
                        }
                        Err(e) => self.status = format!("save failed: {e}"),
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
                let len = self.filtered_tracks().len();
                self.lib_sel = move_index(self.lib_sel, delta, len);
            }
            Focus::Queue => {
                let len = self.queue.tracks.len();
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
                let len = self.filtered_tracks().len();
                self.lib_sel = if len == 0 { 0 } else { idx.min(len - 1) };
            }
            Focus::Queue => {
                let len = self.queue.tracks.len();
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
            Focus::Library => {
                let tracks = self.filtered_tracks();
                if let Some(t) = tracks.get(self.lib_sel) {
                    let track = (*t).clone();
                    self.queue.replace(vec![track.clone()]);
                    self.queue_sel = 0;
                    let _ = self.play_track(&track);
                }
            }
            Focus::Queue => {
                if let Some(t) = self.queue.play_at(self.queue_sel).cloned() {
                    let _ = self.play_track(&t);
                }
            }
            Focus::Playlists => self.load_selected_playlist(),
        }
    }

    fn add_selected_to_queue(&mut self) {
        if self.focus != Focus::Library {
            return;
        }
        let track = self
            .filtered_tracks()
            .get(self.lib_sel)
            .map(|t| (*t).clone());
        if let Some(t) = track {
            self.status = format!("+ {}", t.display_name());
            self.queue.append(t);
        }
    }

    fn toggle_play(&mut self) {
        if self.player.has_track() {
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
        // play whole filtered library as temp queue
        let tracks: Vec<Track> = self.filtered_tracks().into_iter().cloned().collect();
        if tracks.is_empty() {
            self.status = T::empty_library(self.config.lang).into();
            return;
        }
        let idx = self.lib_sel.min(tracks.len() - 1);
        self.queue.replace(tracks);
        if let Some(t) = self.queue.play_at(idx).cloned() {
            let _ = self.play_track(&t);
        }
    }

    fn play_track(&mut self, track: &Track) -> Result<()> {
        match self.player.play_file(&track.path) {
            Ok(()) => {
                self.status = track.display_name();
                Ok(())
            }
            Err(e) => {
                self.status = format!("play error: {e}");
                Err(e)
            }
        }
    }

    fn on_track_ended(&mut self) {
        let repeat_one = self.config.repeat == RepeatMode::One;
        let repeat_all = self.config.repeat == RepeatMode::All;
        if let Some(t) = self.queue.advance(repeat_one, repeat_all).cloned() {
            let _ = self.play_track(&t);
            if let Some(i) = self.queue.index {
                self.queue_sel = i;
            }
        } else {
            self.player.stop();
        }
    }

    fn next_track(&mut self) {
        let repeat_all = self.config.repeat != RepeatMode::Off;
        if let Some(t) = self.queue.advance(false, repeat_all).cloned() {
            let _ = self.play_track(&t);
            if let Some(i) = self.queue.index {
                self.queue_sel = i;
            }
        }
    }

    fn prev_track(&mut self) {
        // if >3s into track, restart; else previous
        if self.player.position() > Duration::from_secs(3) {
            if let Some(t) = self.queue.current().cloned() {
                let _ = self.play_track(&t);
            }
            return;
        }
        if let Some(t) = self.queue.retreat().cloned() {
            let _ = self.play_track(&t);
            if let Some(i) = self.queue.index {
                self.queue_sel = i;
            }
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
                self.queue.replace(pl.tracks);
                self.queue_sel = 0;
                self.focus = Focus::Queue;
                self.status = format!("loaded {}", pl.name);
                if let Some(t) = self.queue.current().cloned() {
                    let _ = self.play_track(&t);
                }
            }
            Err(e) => self.status = format!("load failed: {e}"),
        }
    }

    fn delete_selected_playlist(&mut self) {
        if self.focus != Focus::Playlists {
            return;
        }
        let Some(path) = self.saved_playlists.get(self.pl_sel).cloned() else {
            return;
        };
        match playlist::delete_playlist(&path) {
            Ok(()) => {
                self.status = format!("deleted {}", path.display());
                self.saved_playlists = playlist::list_saved_playlists().unwrap_or_default();
                if self.pl_sel > 0 && self.pl_sel >= self.saved_playlists.len() {
                    self.pl_sel -= 1;
                }
            }
            Err(e) => self.status = format!("delete failed: {e}"),
        }
    }

    fn rescan(&mut self) {
        match Library::scan(&self.config.music_dir) {
            Ok(lib) => {
                let n = lib.tracks.len();
                self.library = lib;
                self.lib_sel = 0;
                self.status = format!("scanned {n} tracks");
            }
            Err(e) => self.status = format!("scan failed: {e}"),
        }
    }

    pub fn ui_state(&self) -> UiState<'_> {
        UiState {
            lang: self.config.lang,
            focus: self.focus,
            input_mode: self.input_mode,
            input_buf: &self.input_buf,
            filter: &self.filter,
            library: &self.library,
            queue: &self.queue,
            lib_sel: self.lib_sel,
            queue_sel: self.queue_sel,
            pl_sel: self.pl_sel,
            playlists: &self.saved_playlists,
            player_playing: self.player.is_playing(),
            player_paused: self.player.is_paused(),
            position: self.player.position(),
            volume: self.player.volume(),
            repeat: self.config.repeat,
            status: &self.status,
            music_dir: &self.config.music_dir,
            show_help: self.show_help,
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
