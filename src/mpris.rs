//! Non-blocking MPRIS bridge for Caelestia / playerctl.
//! UI thread never talks to D-Bus directly — a worker thread does.

use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::thread;
use std::time::{Duration, Instant};

use souvlaki::{
    MediaControlEvent, MediaControls, MediaMetadata, MediaPlayback, MediaPosition, PlatformConfig,
};

#[derive(Debug, Clone, PartialEq)]
pub struct Snapshot {
    pub title: String,
    pub artist: String,
    pub album: String,
    pub duration: Option<Duration>,
    pub position: Duration,
    pub playing: bool,
    pub paused: bool,
    pub has_track: bool,
}

pub struct MprisBridge {
    tx: Sender<Snapshot>,
    pub events: Receiver<MediaControlEvent>,
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

        thread::Builder::new()
            .name("coda-mpris".into())
            .spawn(move || {
                let mut last: Option<Snapshot> = None;
                let mut last_push = Instant::now() - Duration::from_secs(10);
                loop {
                    // Coalesce to latest snapshot.
                    let mut newest = None;
                    loop {
                        match snap_rx.try_recv() {
                            Ok(s) => newest = Some(s),
                            Err(TryRecvError::Empty) => break,
                            Err(TryRecvError::Disconnected) => return,
                        }
                    }

                    if let Some(s) = newest {
                        let meta_changed = last
                            .as_ref()
                            .map(|l| {
                                l.title != s.title
                                    || l.artist != s.artist
                                    || l.album != s.album
                                    || l.has_track != s.has_track
                            })
                            .unwrap_or(true);
                        let play_changed = last
                            .as_ref()
                            .map(|l| l.playing != s.playing || l.paused != s.paused)
                            .unwrap_or(true);
                        let due = last_push.elapsed() >= Duration::from_millis(500);

                        if meta_changed {
                            let meta = if s.has_track {
                                MediaMetadata {
                                    title: Some(s.title.as_str()),
                                    artist: Some(s.artist.as_str()),
                                    album: Some(s.album.as_str()),
                                    duration: s.duration,
                                    ..Default::default()
                                }
                            } else {
                                MediaMetadata::default()
                            };
                            let _ = controls.set_metadata(meta);
                        }

                        if meta_changed || play_changed || due {
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
                            last_push = Instant::now();
                        }
                        last = Some(s);
                    }

                    thread::sleep(Duration::from_millis(200));
                }
            })
            .ok()?;

        Some(Self {
            tx: snap_tx,
            events: ev_rx,
        })
    }

    pub fn publish(&self, snap: Snapshot) {
        let _ = self.tx.send(snap);
    }
}
