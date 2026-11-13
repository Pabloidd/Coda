//! coda — minimal terminal music player.

mod app;
mod config;
mod i18n;
mod library;
mod mpris;
mod player;
mod playlist;
mod single;
mod theme;
mod ui;

use anyhow::Result;
use app::App;

fn main() -> Result<()> {
    let _lock = single::acquire()?;
    let mut app = App::new()?;
    app.run()?;
    Ok(())
}
