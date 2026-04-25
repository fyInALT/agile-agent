use std::path::PathBuf;
use std::sync::{Arc, RwLock, RwLockReadGuard};
use std::time::SystemTime;

use crate::ast::document::Bundle;
use crate::ast::parser::{DslParser, YamlParser};
use crate::ext::error::DslError;
use crate::ext::traits::Fs;

/// Hot-reloadable DSL bundle manager.
pub struct DslReloader {
    parser: YamlParser,
    fs: Box<dyn Fs>,
    dir: PathBuf,
    last_modified: Option<SystemTime>,
    current_bundle: Arc<RwLock<Bundle>>,
}

impl DslReloader {
    /// Create a new reloader, parsing the initial bundle.
    pub fn new(dir: PathBuf, fs: Box<dyn Fs>) -> Result<Self, DslError> {
        let parser = YamlParser::new();
        let bundle = parser.parse_bundle(&dir, fs.as_ref()).map_err(|e| {
            DslError::Parse(e)
        })?;
        Ok(Self {
            parser,
            fs,
            dir,
            last_modified: None,
            current_bundle: Arc::new(RwLock::new(bundle)),
        })
    }

    /// Check if the underlying files have changed and reload if so.
    /// Returns `true` if a reload occurred.
    pub fn check_and_reload(&mut self) -> Result<bool, DslError> {
        let trees_dir = self.dir.join("trees");
        let current_modified = match self.fs.modified(&trees_dir) {
            Ok(t) => t,
            Err(_) => return Ok(false),
        };

        let changed = self
            .last_modified
            .map(|last| current_modified > last)
            .unwrap_or(true);

        if !changed {
            return Ok(false);
        }

        match self.parser.parse_bundle(&self.dir, self.fs.as_ref()) {
            Ok(new_bundle) => {
                self.last_modified = Some(current_modified);
                *self.current_bundle.write().unwrap() = new_bundle;
                Ok(true)
            }
            Err(_) => {
                // Graceful failure: keep old bundle, update baseline so we don't retry forever
                self.last_modified = Some(current_modified);
                Ok(false)
            }
        }
    }

    /// Read access to the current bundle.
    pub fn current(&self) -> RwLockReadGuard<'_, Bundle> {
        self.current_bundle.read().unwrap()
    }
}
