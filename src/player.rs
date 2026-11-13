use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::{Context, Result};
use rodio::{Decoder, OutputStream, OutputStreamHandle, Sink};

struct Shared {
    sink: Option<Sink>,
    current: Option<PathBuf>,
    volume: f32,
    paused: bool,
}

/// One lock, one snapshot. The UI loop should not query the sink field by field.
#[derive(Clone, Copy)]
pub struct Pulse {
    pub empty: bool,
    pub paused: bool,
    pub has_track: bool,
    pub playing: bool,
    pub position: Duration,
    pub volume: f32,
}

impl Default for Pulse {
    fn default() -> Self {
        Self {
            empty: true,
            paused: false,
            has_track: false,
            playing: false,
            position: Duration::ZERO,
            volume: 0.0,
        }
    }
}

pub struct Player {
    _stream: OutputStream,
    handle: OutputStreamHandle,
    inner: Arc<Mutex<Shared>>,
}

impl Drop for Player {
    fn drop(&mut self) {
        self.stop();
    }
}

impl Player {
    pub fn new(volume: f32) -> Result<Self> {
        let (stream, handle) =
            OutputStream::try_default().context("no audio output (PipeWire/ALSA)")?;
        let volume = quantize_volume(volume);
        Ok(Self {
            _stream: stream,
            handle,
            inner: Arc::new(Mutex::new(Shared {
                sink: None,
                current: None,
                volume,
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
        g.paused = false;
        Ok(())
    }

    pub fn stop(&self) {
        let mut g = self.inner.lock().unwrap();
        if let Some(sink) = g.sink.take() {
            sink.stop();
        }
        g.current = None;
        g.paused = false;
    }

    pub fn toggle_pause(&self) {
        let mut g = self.inner.lock().unwrap();
        let Some(sink) = g.sink.as_ref() else {
            return;
        };
        if g.paused {
            sink.play();
            g.paused = false;
        } else {
            sink.pause();
            g.paused = true;
        }
    }

    pub fn pulse(&self) -> Pulse {
        let g = self.inner.lock().unwrap();
        let empty = g.sink.as_ref().map(|s| s.empty()).unwrap_or(true);
        let position = g
            .sink
            .as_ref()
            .map(|s| s.get_pos())
            .unwrap_or(Duration::ZERO);
        let has_track = g.sink.is_some() && g.current.is_some();
        Pulse {
            empty,
            paused: g.paused,
            has_track,
            playing: has_track && !empty && !g.paused,
            position,
            volume: g.volume,
        }
    }

    pub fn set_volume(&self, vol: f32) {
        let vol = quantize_volume(vol);
        let mut g = self.inner.lock().unwrap();
        g.volume = vol;
        if let Some(sink) = g.sink.as_ref() {
            sink.set_volume(vol);
        }
    }

    pub fn adjust_volume(&self, delta: f32) {
        let current = self.inner.lock().unwrap().volume;
        self.set_volume(current + delta);
    }

    /// Seek without decoding the skipped region on the UI thread.
    pub fn seek_to(&self, mut pos: Duration, duration: Option<Duration>) -> Result<()> {
        if let Some(limit) = duration {
            if pos > limit {
                pos = limit;
            }
        }
        let (path, paused, vol, seeked) = {
            let g = self.inner.lock().unwrap();
            let Some(path) = g.current.clone() else {
                return Ok(());
            };
            let seeked = g
                .sink
                .as_ref()
                .and_then(|sink| sink.try_seek(pos).ok())
                .is_some();
            (path, g.paused, g.volume, seeked)
        };
        if seeked {
            return Ok(());
        }

        let file = File::open(&path)?;
        let source = Decoder::new(BufReader::new(file))?;
        let sink = Sink::try_new(&self.handle)?;
        sink.set_volume(vol);
        sink.append(source);
        let seeked = sink.try_seek(pos).is_ok();
        if paused {
            sink.pause();
        }
        let mut g = self.inner.lock().unwrap();
        if let Some(old) = g.sink.take() {
            old.stop();
        }
        g.sink = Some(sink);
        g.paused = paused;
        if !seeked {
            anyhow::bail!("seek unsupported for this file");
        }
        Ok(())
    }

    pub fn seek_by(
        &self,
        delta: Duration,
        forward: bool,
        duration: Option<Duration>,
    ) -> Result<()> {
        let pos = self.pulse().position;
        let target = if forward {
            pos.saturating_add(delta)
        } else {
            pos.saturating_sub(delta)
        };
        self.seek_to(target, duration)
    }
}

fn quantize_volume(vol: f32) -> f32 {
    if !vol.is_finite() {
        return 0.0;
    }
    ((vol.clamp(0.0, 1.0) * 100.0).round() / 100.0).clamp(0.0, 1.0)
}
