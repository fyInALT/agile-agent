//! Append-only event log — `events.jsonl` persistence and replay.

use agent_protocol::events::Event;
use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use tokio::io::AsyncWriteExt;

/// Minimum free disk space before entering read-only mode (100MB).
const MIN_FREE_SPACE_BYTES: u64 = 100 * 1024 * 1024;

/// Append-only JSONL event log.
pub struct EventLog {
    path: PathBuf,
    max_size_bytes: u64,
    read_only: std::sync::atomic::AtomicBool,
}

impl EventLog {
    /// Open or create the event log at `path` with default 100MB limit.
    pub async fn open(path: impl AsRef<Path>) -> Result<Self> {
        Self::open_with_max_size(path, 100).await
    }

    /// Open or create the event log at `path` with a custom size limit in MB.
    pub async fn open_with_max_size(path: impl AsRef<Path>, max_event_log_mb: u64) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        // Ensure parent directory exists.
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent).await.with_context(|| {
                format!("create event log parent directory {}", parent.display())
            })?;
        }
        // Touch the file if it doesn't exist.
        if !path.exists() {
            tokio::fs::write(&path, b"")
                .await
                .with_context(|| format!("create event log {}", path.display()))?;
        }
        Ok(Self {
            path,
            max_size_bytes: max_event_log_mb * 1024 * 1024,
            read_only: std::sync::atomic::AtomicBool::new(false),
        })
    }

    /// Append a single event atomically (newline-delimited JSON).
    /// Returns Ok(()) even in read-only mode (event is silently dropped).
    pub async fn append(&self, event: &Event) -> Result<()> {
        if self.read_only.load(std::sync::atomic::Ordering::Relaxed) {
            tracing::warn!("Event log is in read-only mode (disk full). Dropping event seq={}", event.seq);
            return Ok(());
        }
        if let Err(e) = self.check_disk_space().await {
            tracing::warn!("Disk space check failed: {}. Entering read-only mode.", e);
            self.read_only.store(true, std::sync::atomic::Ordering::Relaxed);
            return Ok(());
        }
        self.maybe_rotate().await?;
        let line = serde_json::to_string(event).context("serialize event")?;
        let mut bytes = line.into_bytes();
        bytes.push(b'\n');
        let mut file = tokio::fs::OpenOptions::new()
            .append(true)
            .create(true)
            .open(&self.path)
            .await
            .with_context(|| format!("open event log {}", self.path.display()))?;
        file.write_all(&bytes)
            .await
            .with_context(|| format!("append to event log {}", self.path.display()))?;
        file.sync_all()
            .await
            .with_context(|| format!("sync event log {}", self.path.display()))?;
        Ok(())
    }

    /// Check available disk space on the event log's filesystem.
    async fn check_disk_space(&self) -> Result<()> {
        let parent = self.path.parent().unwrap_or_else(|| std::path::Path::new("."));
        let stat = nix::sys::statfs::statfs(parent)
            .with_context(|| format!("statfs for disk space check on {}", parent.display()))?;
        let available = stat.blocks_available() * stat.block_size() as u64;
        if available < MIN_FREE_SPACE_BYTES {
            anyhow::bail!(
                "insufficient disk space: {} bytes available (min {})",
                available,
                MIN_FREE_SPACE_BYTES
            );
        }
        Ok(())
    }

    /// Return whether the event log is in read-only mode.
    pub fn is_read_only(&self) -> bool {
        self.read_only.load(std::sync::atomic::Ordering::Relaxed)
    }

    /// Replay all events with `seq >= from_seq` in order.
    pub async fn replay_from(&self, from_seq: u64) -> Result<Vec<Event>> {
        let bytes = tokio::fs::read(&self.path)
            .await
            .with_context(|| format!("read event log {}", self.path.display()))?;
        let mut events = Vec::new();
        for (line_no, line) in bytes.split(|&b| b == b'\n').enumerate() {
            if line.is_empty() {
                continue;
            }
            match serde_json::from_slice::<Event>(line) {
                Ok(event) => {
                    if event.seq >= from_seq {
                        events.push(event);
                    }
                }
                Err(e) => {
                    tracing::warn!(
                        "skipping corrupted event log line {}: {}",
                        line_no + 1,
                        e
                    );
                }
            }
        }
        events.sort_by_key(|e| e.seq);
        Ok(events)
    }

    /// Replay events in a specific seq range `[start_seq, end_seq)`.
    pub async fn replay_range(&self, start_seq: u64, end_seq: u64) -> Result<Vec<Event>> {
        let all = self.replay_from(start_seq).await?;
        Ok(all.into_iter().filter(|e| e.seq < end_seq).collect())
    }

    /// Detect missing sequence numbers in the log.
    pub async fn detect_gaps(&self) -> Result<Vec<u64>> {
        let mut events = self.replay_from(1).await?;
        events.sort_by_key(|e| e.seq);
        if events.is_empty() {
            return Ok(Vec::new());
        }
        let mut gaps = Vec::new();
        let mut expected = events[0].seq;
        for event in events {
            while expected < event.seq {
                gaps.push(expected);
                expected += 1;
            }
            expected = event.seq.saturating_add(1);
        }
        Ok(gaps)
    }

    async fn maybe_rotate(&self) -> Result<()> {
        match tokio::fs::metadata(&self.path).await {
            Ok(meta) if meta.len() > self.max_size_bytes => {
                self.rotate().await?;
            }
            _ => {}
        }
        Ok(())
    }

    async fn rotate(&self) -> Result<()> {
        for i in (1..=5).rev() {
            let src = if i == 1 {
                self.path.clone()
            } else {
                self.path.with_extension(format!("jsonl.{}", i - 1))
            };
            let dst = self.path.with_extension(format!("jsonl.{}", i));
            if tokio::fs::metadata(&src).await.is_ok() {
                let _ = tokio::fs::remove_file(&dst).await;
                if let Err(e) = tokio::fs::rename(&src, &dst).await {
                    tracing::warn!("Failed to rotate event log {:?} -> {:?}: {}", src, dst, e);
                }
            }
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use agent_protocol::events::{Event, EventPayload, ErrorData};

    fn make_event(seq: u64) -> Event {
        Event {
            seq,
            payload: EventPayload::Error(ErrorData {
                message: format!("event-{}", seq),
                source: None,
            }),
        }
    }

    #[tokio::test]
    async fn append_produces_valid_jsonl() {
        let tmp = tempfile::tempdir().unwrap();
        let log = EventLog::open(tmp.path().join("events.jsonl")).await.unwrap();

        let event = make_event(1);
        log.append(&event).await.unwrap();

        let bytes = tokio::fs::read(tmp.path().join("events.jsonl")).await.unwrap();
        let lines: Vec<&[u8]> = bytes.split(|&b| b == b'\n').filter(|l| !l.is_empty()).collect();
        assert_eq!(lines.len(), 1);

        let parsed: Event = serde_json::from_slice(lines[0]).unwrap();
        assert_eq!(parsed.seq, 1);
    }

    #[tokio::test]
    async fn replay_from_filters_correctly() {
        let tmp = tempfile::tempdir().unwrap();
        let log = EventLog::open(tmp.path().join("events.jsonl")).await.unwrap();

        for seq in 1..=5 {
            log.append(&make_event(seq)).await.unwrap();
        }

        let replayed = log.replay_from(3).await.unwrap();
        assert_eq!(replayed.len(), 3);
        assert_eq!(replayed[0].seq, 3);
        assert_eq!(replayed[2].seq, 5);
    }

    #[tokio::test]
    async fn replay_corrupted_line_skipped() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("events.jsonl");
        tokio::fs::write(&path, b"not-json\n")
            .await
            .unwrap();
        let log = EventLog::open(&path).await.unwrap();

        let replayed = log.replay_from(1).await.unwrap();
        assert!(replayed.is_empty());
    }

    #[tokio::test]
    async fn replay_range_exclusive_end() {
        let tmp = tempfile::tempdir().unwrap();
        let log = EventLog::open(tmp.path().join("events.jsonl")).await.unwrap();

        for seq in 1..=5 {
            log.append(&make_event(seq)).await.unwrap();
        }

        let replayed = log.replay_range(2, 4).await.unwrap();
        assert_eq!(replayed.len(), 2);
        assert_eq!(replayed[0].seq, 2);
        assert_eq!(replayed[1].seq, 3);
    }

    #[tokio::test]
    async fn log_durability() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("events.jsonl");
        let log = EventLog::open(&path).await.unwrap();

        let event = make_event(1);
        log.append(&event).await.unwrap();

        // Drop log and read file directly to prove durability
        drop(log);
        let bytes = tokio::fs::read(&path).await.unwrap();
        let lines: Vec<&[u8]> = bytes.split(|&b| b == b'\n').filter(|l| !l.is_empty()).collect();
        assert_eq!(lines.len(), 1);
        let parsed: Event = serde_json::from_slice(lines[0]).unwrap();
        assert_eq!(parsed.seq, 1);
    }

    #[tokio::test]
    async fn log_before_broadcast() {
        let tmp = tempfile::tempdir().unwrap();
        let log = EventLog::open(tmp.path().join("events.jsonl")).await.unwrap();

        let event = make_event(1);
        log.append(&event).await.unwrap();

        // Event must be on disk before any consumer reads it
        let replayed = log.replay_from(1).await.unwrap();
        assert_eq!(replayed.len(), 1);
        assert_eq!(replayed[0].seq, 1);
    }

    #[tokio::test]
    async fn replay_order() {
        let tmp = tempfile::tempdir().unwrap();
        let log = EventLog::open(tmp.path().join("events.jsonl")).await.unwrap();

        log.append(&make_event(3)).await.unwrap();
        log.append(&make_event(1)).await.unwrap();
        log.append(&make_event(2)).await.unwrap();

        let replayed = log.replay_from(1).await.unwrap();
        assert_eq!(replayed.len(), 3);
        // Should be sorted by seq
        assert_eq!(replayed[0].seq, 1);
        assert_eq!(replayed[1].seq, 2);
        assert_eq!(replayed[2].seq, 3);
    }

    #[tokio::test]
    async fn replay_under_2s() {
        let tmp = tempfile::tempdir().unwrap();
        let log = EventLog::open(tmp.path().join("events.jsonl")).await.unwrap();

        for seq in 1..=10000 {
            log.append(&make_event(seq)).await.unwrap();
        }

        let start = std::time::Instant::now();
        let replayed = log.replay_from(1).await.unwrap();
        let elapsed = start.elapsed();

        assert_eq!(replayed.len(), 10000);
        assert!(elapsed.as_secs_f64() < 2.0, "replay took {:?}, expected under 2s", elapsed);
    }

    #[tokio::test]
    async fn gap_detected() {
        let tmp = tempfile::tempdir().unwrap();
        let log = EventLog::open(tmp.path().join("events.jsonl")).await.unwrap();

        log.append(&make_event(1)).await.unwrap();
        log.append(&make_event(2)).await.unwrap();
        log.append(&make_event(4)).await.unwrap();

        let gaps = log.detect_gaps().await.unwrap();
        assert_eq!(gaps, vec![3]);
    }

    #[tokio::test]
    async fn gap_recovery() {
        let tmp = tempfile::tempdir().unwrap();
        let log = EventLog::open(tmp.path().join("events.jsonl")).await.unwrap();

        log.append(&make_event(1)).await.unwrap();
        log.append(&make_event(2)).await.unwrap();
        log.append(&make_event(4)).await.unwrap();

        let gaps = log.detect_gaps().await.unwrap();
        assert_eq!(gaps, vec![3]);

        // Fill the gap
        log.append(&make_event(3)).await.unwrap();
        let gaps = log.detect_gaps().await.unwrap();
        assert!(gaps.is_empty());
    }

    #[tokio::test]
    async fn log_rotation_on_size_exceeded() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("events.jsonl");
        // Pre-seed file with 1 byte so it exceeds 0-byte limit.
        tokio::fs::write(&path, b"x").await.unwrap();
        let log = EventLog::open_with_max_size(&path, 0).await.unwrap();

        for seq in 1..=7 {
            log.append(&make_event(seq)).await.unwrap();
        }

        assert!(path.exists());
        for i in 1..=5 {
            assert!(path.with_extension(format!("jsonl.{}", i)).exists());
        }
        assert!(!path.with_extension("jsonl.6").exists());
    }

    #[tokio::test]
    async fn read_only_mode_drops_events_silently() {
        let tmp = tempfile::tempdir().unwrap();
        let log = EventLog::open(tmp.path().join("events.jsonl")).await.unwrap();

        log.read_only.store(true, std::sync::atomic::Ordering::Relaxed);
        log.append(&make_event(1)).await.unwrap();

        let replayed = log.replay_from(1).await.unwrap();
        assert!(replayed.is_empty());
        assert!(log.is_read_only());
    }
}
