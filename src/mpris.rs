//! Non-blocking MPRIS bridge for GNOME Medialine / playerctl.
//! UI thread never talks to D-Bus directly — a worker thread does.

use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use souvlaki::{
    MediaControlEvent, MediaControls, MediaMetadata, MediaPlayback, MediaPosition, PlatformConfig,
};

#[derive(Debug, Clone, PartialEq)]
pub struct Snapshot {
    pub title: String,
    pub artist: String,
    pub album: String,
    /// Absolute path — used as unique art URL so clients notice track changes
    /// (souvlaki hardcodes mpris:trackid to "/").
    pub path: String,
    pub cover: String,
    pub duration: Option<Duration>,
    pub position: Duration,
    pub playing: bool,
    pub paused: bool,
    pub has_track: bool,
    pub volume: f32,
}

pub struct MprisBridge {
    tx: Option<Sender<Snapshot>>,
    worker: Option<JoinHandle<()>>,
    pub events: Receiver<MediaControlEvent>,
}

impl Drop for MprisBridge {
    fn drop(&mut self) {
        self.shutdown();
    }
}

impl MprisBridge {
    pub fn start() -> Option<Self> {
        let (snap_tx, snap_rx) = mpsc::channel::<Snapshot>();
        let (ev_tx, ev_rx) = mpsc::channel::<MediaControlEvent>();

        let mut controls = MediaControls::new(PlatformConfig {
            dbus_name: "coda",
            display_name: "coda",
            hwnd: None,
        })
        .ok()?;

        let ev_tx2 = ev_tx.clone();
        if controls
            .attach(move |e| {
                let _ = ev_tx2.send(e);
            })
            .is_err()
        {
            return None;
        }

        let worker = thread::Builder::new()
            .name("coda-mpris".into())
            .spawn(move || {
                let mut last: Option<Snapshot> = None;
                loop {
                    // Block until the UI has something new. Dropping the sender ends the thread.
                    let first = match snap_rx.recv() {
                        Ok(s) => s,
                        Err(_) => {
                            let _ = controls.detach();
                            return;
                        }
                    };
                    let mut newest = first;
                    loop {
                        match snap_rx.try_recv() {
                            Ok(s) => newest = s,
                            Err(TryRecvError::Empty) => break,
                            Err(TryRecvError::Disconnected) => {
                                let _ = controls.detach();
                                return;
                            }
                        }
                    }
                    let s = newest;

                    let meta_changed = last.as_ref().is_none_or(|l| {
                        l.title != s.title
                            || l.artist != s.artist
                            || l.album != s.album
                            || l.path != s.path
                            || l.cover != s.cover
                            || l.has_track != s.has_track
                            || l.duration != s.duration
                    });
                    let play_changed = last
                        .as_ref()
                        .is_none_or(|l| l.playing != s.playing || l.paused != s.paused);
                    let volume_changed = last
                        .as_ref()
                        .is_none_or(|l| (l.volume - s.volume).abs() > 0.005);
                    let pos_changed = last.as_ref().is_none_or(|l| {
                        l.position.as_millis().abs_diff(s.position.as_millis()) >= 500
                    });

                    if meta_changed {
                        let cover = if s.has_track && !s.cover.is_empty() {
                            Some(s.cover.as_str())
                        } else {
                            None
                        };
                        let meta = if s.has_track {
                            MediaMetadata {
                                title: Some(s.title.as_str()),
                                artist: Some(s.artist.as_str()),
                                album: Some(s.album.as_str()),
                                cover_url: cover,
                                duration: s.duration,
                            }
                        } else {
                            MediaMetadata::default()
                        };
                        let _ = controls.set_metadata(meta);
                    }

                    if meta_changed || play_changed || pos_changed {
                        let progress = Some(MediaPosition(s.position));
                        let playback = if !s.has_track {
                            MediaPlayback::Stopped
                        } else if s.paused {
                            MediaPlayback::Paused { progress }
                        } else if s.playing {
                            MediaPlayback::Playing { progress }
                        } else {
                            MediaPlayback::Stopped
                        };
                        let _ = controls.set_playback(playback);
                    }
                    if volume_changed {
                        let _ = controls.set_volume(f64::from(s.volume));
                    }
                    last = Some(s);
                }
            })
            .ok()?;

        Some(Self {
            tx: Some(snap_tx),
            worker: Some(worker),
            events: ev_rx,
        })
    }

    pub fn publish(&self, snap: Snapshot) {
        if let Some(tx) = &self.tx {
            let _ = tx.send(snap);
        }
    }

    /// Drop the sender so the worker exits, then join it (bounded wait).
    pub fn shutdown(&mut self) {
        self.tx.take();
        if let Some(handle) = self.worker.take() {
            let (done_tx, done_rx) = mpsc::channel();
            thread::spawn(move || {
                let _ = handle.join();
                let _ = done_tx.send(());
            });
            let _ = done_rx.recv_timeout(Duration::from_millis(800));
        }
    }
}
