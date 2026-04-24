use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use decision_dsl::ext::{
    Clock, Fs, FsError, Logger, LogLevel, MockClock, NullLogger, PollWatcher, Session,
    SessionError, SessionErrorKind, StdFs, StderrLogger, SystemClock, Watcher, WatcherError,
};

// ── SessionError / SessionErrorKind ─────────────────────────────────────────

#[test]
fn session_error_display() {
    let e = SessionError {
        kind: SessionErrorKind::Timeout,
        message: "took too long".into(),
    };
    assert_eq!(e.to_string(), "session error (Timeout): took too long");
}

#[test]
fn session_error_kind_equality() {
    assert_eq!(SessionErrorKind::Unavailable, SessionErrorKind::Unavailable);
    assert_ne!(SessionErrorKind::Unavailable, SessionErrorKind::Timeout);
}

// ── Clock ───────────────────────────────────────────────────────────────────

#[test]
fn system_clock_returns_instant() {
    let clock = SystemClock;
    let t1 = clock.now();
    std::thread::sleep(Duration::from_millis(5));
    let t2 = clock.now();
    assert!(t2 > t1);
}

#[test]
fn mock_clock_advances() {
    let mut clock = MockClock::new();
    let t1 = clock.now();
    clock.advance(Duration::from_secs(10));
    let t2 = clock.now();
    assert_eq!(t2.duration_since(t1), Duration::from_secs(10));
}

#[test]
fn mock_clock_can_advance_multiple_times() {
    let mut clock = MockClock::new();
    let t0 = clock.now();
    clock.advance(Duration::from_millis(100));
    clock.advance(Duration::from_millis(200));
    let t2 = clock.now();
    assert_eq!(t2.duration_since(t0), Duration::from_millis(300));
}

// ── Logger ──────────────────────────────────────────────────────────────────

#[test]
fn null_logger_does_not_panic() {
    let logger = NullLogger;
    logger.log(LogLevel::Error, "test", "hello");
    logger.log(LogLevel::Trace, "test", "world");
}

#[test]
fn log_level_ordering() {
    assert!(LogLevel::Trace < LogLevel::Debug);
    assert!(LogLevel::Debug < LogLevel::Info);
    assert!(LogLevel::Info < LogLevel::Warn);
    assert!(LogLevel::Warn < LogLevel::Error);
}

#[test]
fn stderr_logger_does_not_panic() {
    let logger = StderrLogger;
    logger.log(LogLevel::Info, "target", "message");
}

// ── Fs / StdFs ──────────────────────────────────────────────────────────────

#[test]
fn fs_error_display_io() {
    let e = FsError::Io("permission denied".into());
    assert_eq!(e.to_string(), "fs error: permission denied");
}

#[test]
fn fs_error_display_not_found() {
    let e = FsError::NotFound(PathBuf::from("/missing"));
    assert_eq!(e.to_string(), "not found: /missing");
}

#[test]
fn fs_error_implements_error() {
    fn assert_err<T: std::error::Error>() {}
    assert_err::<FsError>();
}

#[test]
fn std_fs_read_to_string_existing_file() {
    let fs = StdFs;
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let path = Path::new(manifest_dir).join("Cargo.toml");
    let content = fs.read_to_string(&path).unwrap();
    assert!(content.contains("decision-dsl"));
}

#[test]
fn std_fs_read_to_string_missing_file() {
    let fs = StdFs;
    let path = Path::new("/nonexistent/path/to/file.txt");
    let result = fs.read_to_string(path);
    assert!(result.is_err());
}

#[test]
fn std_fs_read_dir_existing() {
    let fs = StdFs;
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let path = Path::new(manifest_dir).join("src");
    let entries = fs.read_dir(&path).unwrap();
    assert!(!entries.is_empty());
}

#[test]
fn std_fs_read_dir_missing() {
    let fs = StdFs;
    let path = Path::new("/nonexistent/dir");
    let result = fs.read_dir(path);
    assert!(result.is_err());
}

#[test]
fn std_fs_modified_existing() {
    let fs = StdFs;
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let path = Path::new(manifest_dir).join("Cargo.toml");
    let mtime = fs.modified(&path).unwrap();
    assert!(mtime < SystemTime::now());
}

#[test]
fn std_fs_modified_missing() {
    let fs = StdFs;
    let path = Path::new("/nonexistent");
    let result = fs.modified(path);
    assert!(result.is_err());
}

// ── Watcher / PollWatcher ───────────────────────────────────────────────────

#[test]
fn watcher_error_display() {
    let e = WatcherError::Io("broken pipe".into());
    assert_eq!(e.to_string(), "watcher error: broken pipe");
}

#[test]
fn poll_watcher_detects_no_change() {
    let fs = StdFs;
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let path = PathBuf::from(manifest_dir).join("Cargo.toml");
    let mut watcher = PollWatcher::new(path, Box::new(fs)).unwrap();
    // First call returns true because no baseline exists
    let changed1 = watcher.has_changed().unwrap();
    assert!(changed1);
    // Second call should return false (file hasn't changed)
    let changed2 = watcher.has_changed().unwrap();
    assert!(!changed2);
}

// ── Object safety ───────────────────────────────────────────────────────────

#[test]
fn session_is_object_safe() {
    fn _assert_object_safe<T: Session>() {}
}

#[test]
fn clock_is_object_safe() {
    fn _assert_object_safe<T: Clock>() {}
}

#[test]
fn logger_is_object_safe() {
    fn _assert_object_safe<T: Logger>() {}
}

#[test]
fn fs_is_object_safe() {
    fn _assert_object_safe<T: Fs>() {}
}

#[test]
fn watcher_is_object_safe() {
    fn _assert_object_safe<T: Watcher>() {}
}
