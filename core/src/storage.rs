use std::path::PathBuf;

use anyhow::Context;
use anyhow::Result;

pub fn app_data_root() -> Result<PathBuf> {
    let data_dir = dirs::data_local_dir().context("local data directory is unavailable")?;
    Ok(data_dir.join("agile-agent"))
}
