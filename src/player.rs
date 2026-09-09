use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use rodio::{Decoder, OutputStream, OutputStreamHandle, Sink, Source};

struct Shared {
    sink: Option<Sink>,
    current: Option<PathBuf>,
    volume: f32,
    /// Elapsed playback before the current running segment.
    base_pos: Duration,
    segment_start: Option<Instant>,
    paused: bool,
}

pub struct Player {
    _stream: OutputStream,
    handle: OutputStreamHandle,
    inner: Arc<Mutex<Shared>>,
}

impl Player {
    pub fn new(volume: f32) -> Result<Self> {
        let (stream, handle) =
            OutputStream::try_default().context("no audio output (PipeWire/ALSA)")?;
        Ok(Self {
            _stream: stream,
            handle,
            inner: Arc::new(Mutex::new(Shared {
                sink: None,
                current: None,
                volume: volume.clamp(0.0, 1.0),
                base_pos: Duration::ZERO,
                segment_start: None,
                paused: false,
            })),
        })
    }

    pub fn play_file(&self, path: &Path) -> Result<()> {
        let file = File::open(path).with_context(|| format!("open {}", path.display()))?;
        let source = Decoder::new(BufReader::new(file))
            .with_context(|| format!("decode {}", path.display()))?;

        let sink = Sink::try_new(&self.handle).context("audio sink")?;
        let mut g = self.inner.lock().unwrap();
        sink.set_volume(g.volume);
        sink.append(source);

        if let Some(old) = g.sink.take() {
            old.stop();
        }
        g.sink = Some(sink);
        g.current = Some(path.to_path_buf());
        g.base_pos = Duration::ZERO;
        g.segment_start = Some(Instant::now());
        g.paused = false;
        Ok(())
    }

    pub fn stop(&self) {
        let mut g = self.inner.lock().unwrap();
        if let Some(sink) = g.sink.take() {
            sink.stop();
        }
        g.current = None;
        g.base_pos = Duration::ZERO;
        g.segment_start = None;
        g.paused = false;
    }

    pub fn toggle_pause(&self) {
        let mut g = self.inner.lock().unwrap();
        if g.sink.is_none() {
            return;
        }
        if g.paused {
            if let Some(sink) = g.sink.as_ref() {
                sink.play();
            }
            g.segment_start = Some(Instant::now());
            g.paused = false;
        } else {
            let extra = g
                .segment_start
                .map(|t| t.elapsed())
                .unwrap_or(Duration::ZERO);
            g.base_pos += extra;
            g.segment_start = None;
            if let Some(sink) = g.sink.as_ref() {
                sink.pause();
            }
            g.paused = true;
        }
    }

    pub fn is_paused(&self) -> bool {
        self.inner.lock().unwrap().paused
    }

    pub fn is_empty(&self) -> bool {
        let g = self.inner.lock().unwrap();
        g.sink.as_ref().map(|s| s.empty()).unwrap_or(true)
    }

    pub fn is_playing(&self) -> bool {
        let g = self.inner.lock().unwrap();
        match g.sink.as_ref() {
            Some(s) => !s.empty() && !g.paused,
            None => false,
        }
    }

    pub fn has_track(&self) -> bool {
        let g = self.inner.lock().unwrap();
        g.sink.is_some() && g.current.is_some()
    }

    pub fn current_path(&self) -> Option<PathBuf> {
        self.inner.lock().unwrap().current.clone()
    }

    pub fn set_volume(&self, vol: f32) {
        let vol = vol.clamp(0.0, 1.0);
        let mut g = self.inner.lock().unwrap();
        g.volume = vol;
        if let Some(sink) = g.sink.as_ref() {
            sink.set_volume(vol);
        }
    }

    pub fn volume(&self) -> f32 {
        self.inner.lock().unwrap().volume
    }

    pub fn adjust_volume(&self, delta: f32) {
        let v = self.volume() + delta;
        self.set_volume(v);
    }

    pub fn position(&self) -> Duration {
        let g = self.inner.lock().unwrap();
        let mut pos = g.base_pos;
        if !g.paused {
            if let Some(start) = g.segment_start {
                pos += start.elapsed();
            }
        }
        pos
    }

    /// Approximate seek by restarting from an offset (best-effort, may hitch briefly).
    pub fn seek_rel(&self, delta: i64, duration: Option<Duration>) -> Result<()> {
        let (path, was_paused, vol, mut pos) = {
            let g = self.inner.lock().unwrap();
            let path = match g.current.clone() {
                Some(p) => p,
                None => return Ok(()),
            };
            let mut pos = g.base_pos;
            if !g.paused {
                if let Some(start) = g.segment_start {
                    pos += start.elapsed();
                }
            }
            (path, g.paused, g.volume, pos)
        };

        let mut secs = pos.as_secs_f64() + delta as f64;
        if secs < 0.0 {
            secs = 0.0;
        }
        if let Some(d) = duration {
            secs = secs.min(d.as_secs_f64());
        }
        pos = Duration::from_secs_f64(secs);

        let file = File::open(&path)?;
        let source = Decoder::new(BufReader::new(file))?;
        // Prefer byte-ish skip when possible; this can hitch briefly on large offsets.
        let skipped = source.skip_duration(pos);

        let sink = Sink::try_new(&self.handle)?;
        sink.set_volume(vol);
        sink.append(skipped);
        if was_paused {
            sink.pause();
        }

        let mut g = self.inner.lock().unwrap();
        if let Some(old) = g.sink.take() {
            old.stop();
        }
        g.sink = Some(sink);
        g.base_pos = pos;
        g.paused = was_paused;
        g.segment_start = if was_paused {
            None
        } else {
            Some(Instant::now())
        };
        Ok(())
    }
}
