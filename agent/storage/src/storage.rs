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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_data_root_returns_valid_path() {
        let result = app_data_root();
        // In test environment, dirs::data_local_dir() should return a valid path
        assert!(result.is_ok());
        let path = result.unwrap();
        assert!(path.ends_with("agile-agent"));
    }

    #[test]
    fn app_data_root_is_absolute() {
        let result = app_data_root();
        assert!(result.is_ok());
        let path = result.unwrap();
        assert!(path.is_absolute());
    }

    #[test]
    fn app_data_root_parent_is_data_local() {
        let result = app_data_root();
        assert!(result.is_ok());
        let path = result.unwrap();
        let parent = path.parent();
        assert!(parent.is_some());
        // Parent should match dirs::data_local_dir()
        let expected_parent = dirs::data_local_dir();
        assert!(expected_parent.is_some());
        assert_eq!(parent.unwrap(), expected_parent.unwrap().as_path());
    }
}