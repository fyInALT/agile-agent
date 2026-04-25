use std::cell::RefCell;
use std::collections::VecDeque;
use std::fmt;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime};

use super::error::{SessionError, SessionErrorKind};

// ── Session ─────────────────────────────────────────────────────────────────

pub trait Session {
    fn send(&mut self, message: &str) -> Result<(), SessionError>;

    fn send_with_hint(&mut self, message: &str, model: &str) -> Result<(), SessionError> {
        let _ = model;
        self.send(message)
    }

    fn is_ready(&self) -> bool;

    fn receive(&mut self) -> Result<String, SessionError>;
}

// ── Clock ───────────────────────────────────────────────────────────────────

pub trait Clock {
    fn now(&self) -> Instant;
}

pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> Instant {
        Instant::now()
    }
}

pub struct MockClock {
    current: Instant,
}

impl MockClock {
    pub fn new() -> Self {
        Self {
            current: Instant::now(),
        }
    }

    pub fn advance(&mut self, d: Duration) {
        self.current += d;
    }
}

impl Clock for MockClock {
    fn now(&self) -> Instant {
        self.current
    }
}

// ── Logger ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum LogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

pub trait Logger {
    fn log(&self, level: LogLevel, target: &str, msg: &str);
}

pub struct NullLogger;

impl Logger for NullLogger {
    fn log(&self, _level: LogLevel, _target: &str, _msg: &str) {}
}

pub struct StderrLogger;

impl Logger for StderrLogger {
    fn log(&self, level: LogLevel, target: &str, msg: &str) {
        eprintln!("[{level:?}] {target}: {msg}");
    }
}

// ── Fs ──────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum FsError {
    Io(String),
    NotFound(PathBuf),
}

impl fmt::Display for FsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FsError::Io(msg) => write!(f, "fs error: {msg}"),
            FsError::NotFound(path) => write!(f, "not found: {}", path.display()),
        }
    }
}

impl std::error::Error for FsError {}

pub trait Fs {
    fn read_to_string(&self, path: &Path) -> Result<String, FsError>;
    fn read_dir(&self, path: &Path) -> Result<Vec<PathBuf>, FsError>;
    fn modified(&self, path: &Path) -> Result<SystemTime, FsError>;
}

pub struct StdFs;

impl Fs for StdFs {
    fn read_to_string(&self, path: &Path) -> Result<String, FsError> {
        std::fs::read_to_string(path).map_err(|e| FsError::Io(e.to_string()))
    }

    fn read_dir(&self, path: &Path) -> Result<Vec<PathBuf>, FsError> {
        std::fs::read_dir(path)
            .map_err(|e| FsError::Io(e.to_string()))?
            .map(|entry| entry.map(|e| e.path()).map_err(|e| FsError::Io(e.to_string())))
            .collect()
    }

    fn modified(&self, path: &Path) -> Result<SystemTime, FsError> {
        std::fs::metadata(path)
            .and_then(|m| m.modified())
            .map_err(|e| FsError::Io(e.to_string()))
    }
}

// ── Watcher ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum WatcherError {
    Io(String),
}

impl fmt::Display for WatcherError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            WatcherError::Io(msg) => write!(f, "watcher error: {msg}"),
        }
    }
}

impl std::error::Error for WatcherError {}

pub trait Watcher {
    fn has_changed(&mut self) -> Result<bool, WatcherError>;
}

pub struct PollWatcher {
    path: PathBuf,
    last_modified: Option<SystemTime>,
    fs: Box<dyn Fs>,
}

impl PollWatcher {
    pub fn new(path: PathBuf, fs: Box<dyn Fs>) -> Result<Self, WatcherError> {
        // Baseline is intentionally None so the first has_changed() call
        // reports true (there was no prior observation).
        let _ = fs.modified(&path).map_err(|e| WatcherError::Io(e.to_string()))?;
        Ok(Self {
            path,
            last_modified: None,
            fs,
        })
    }
}

impl Watcher for PollWatcher {
    fn has_changed(&mut self) -> Result<bool, WatcherError> {
        let current = self
            .fs
            .modified(&self.path)
            .map_err(|e| WatcherError::Io(e.to_string()))?;
        let changed = self
            .last_modified
            .map(|last| current > last)
            .unwrap_or(true);
        self.last_modified = Some(current);
        Ok(changed)
    }
}

// ── MockSession ─────────────────────────────────────────────────────────────

pub struct MockSession {
    replies: RefCell<VecDeque<String>>,
    ready: RefCell<bool>,
    sent_messages: RefCell<Vec<String>>,
}

impl MockSession {
    pub fn new() -> Self {
        Self {
            replies: RefCell::new(VecDeque::new()),
            ready: RefCell::new(false),
            sent_messages: RefCell::new(Vec::new()),
        }
    }

    pub fn with_reply(reply: impl Into<String>) -> Self {
        let s = Self::new();
        s.push_reply(reply);
        s.set_ready(true);
        s
    }

    pub fn push_reply(&self, reply: impl Into<String>) {
        self.replies.borrow_mut().push_back(reply.into());
    }

    pub fn set_ready(&self, ready: bool) {
        *self.ready.borrow_mut() = ready;
    }

    pub fn sent_messages(&self) -> Vec<String> {
        self.sent_messages.borrow().clone()
    }
}

impl Session for MockSession {
    fn send(&mut self, message: &str) -> Result<(), SessionError> {
        self.sent_messages.borrow_mut().push(message.to_string());
        Ok(())
    }

    fn is_ready(&self) -> bool {
        *self.ready.borrow()
    }

    fn receive(&mut self) -> Result<String, SessionError> {
        self.set_ready(false);
        self.replies.borrow_mut().pop_front().ok_or(SessionError {
            kind: SessionErrorKind::UnexpectedFormat,
            message: "no reply queued".into(),
        })
    }
}

// ── CaptureLogger ───────────────────────────────────────────────────────────

pub struct CaptureLogger {
    entries: RefCell<Vec<(LogLevel, String, String)>>,
}

impl CaptureLogger {
    pub fn new() -> Self {
        Self {
            entries: RefCell::new(Vec::new()),
        }
    }

    pub fn entries(&self) -> Vec<(LogLevel, String, String)> {
        self.entries.borrow().clone()
    }
}

impl Logger for CaptureLogger {
    fn log(&self, level: LogLevel, target: &str, msg: &str) {
        self.entries
            .borrow_mut()
            .push((level, target.to_string(), msg.to_string()));
    }
}
