use std::fs::File;
use std::path::PathBuf;

use anyhow::{bail, Context, Result};
use fs2::FileExt;

pub struct InstanceLock {
    _file: File,
}

pub fn acquire() -> Result<InstanceLock> {
    let dir = dirs::runtime_dir()
        .or_else(dirs::data_local_dir)
        .context("no runtime/data dir")?
        .join("coda");
    std::fs::create_dir_all(&dir)?;
    let path: PathBuf = dir.join("coda.lock");
    let file = File::create(&path)?;
    if file.try_lock_exclusive().is_err() {
        bail!("coda is already running");
    }
    Ok(InstanceLock { _file: file })
}
