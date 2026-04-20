//! Storage path utilities

use std::path::PathBuf;

use anyhow::Context;
use anyhow::Result;

/// Get the application data root directory
///
/// Returns the path to the agile-agent data directory in the user's
/// local data directory (e.g., ~/.local/share/agile-agent on Linux).
pub fn app_data_root() -> Result<PathBuf> {
    let data_dir = dirs::data_local_dir().context("local data directory is unavailable")?;
    Ok(data_dir.join("agile-agent"))
}