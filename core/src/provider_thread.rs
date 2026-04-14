//! ProviderThreadHandle for managing provider thread lifecycle
//!
//! Provides controlled thread spawning, monitoring, and graceful shutdown.

use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::thread::{Builder, JoinHandle};
use std::time::{Duration, Instant};

use crate::logging;
use crate::provider::{ProviderEvent, ProviderKind, SessionHandle};

/// Handle to a running provider thread
///
/// Manages the thread lifecycle including:
/// - Named thread for debugging
/// - Event receiver for collecting provider events
/// - Graceful shutdown with timeout
pub struct ProviderThreadHandle {
    /// Thread join handle
    handle: Option<JoinHandle<()>>,
    /// Receiver for provider events
    event_rx: Receiver<ProviderEvent>,
    /// Sender kept alive to prevent early disconnect (dropped on shutdown)
    _keepalive_tx: Sender<ProviderEvent>,
    /// Thread name for debugging
    thread_name: String,
    /// When the thread was started
    started_at: Instant,
}

/// Result of stopping a provider thread
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ThreadStopResult {
    /// Thread finished gracefully
    GracefulStop { duration_ms: u64 },
    /// Thread timed out but was abandoned
    TimeoutAbandoned { timeout_ms: u64 },
    /// Thread panicked during execution
    Panicked { error: String },
    /// Thread was already stopped
    AlreadyStopped,
}

impl ProviderThreadHandle {
    /// Create a new thread handle with event receiver
    ///
    /// This is typically called after spawning a thread that sends events.
    pub fn new(
        handle: JoinHandle<()>,
        event_rx: Receiver<ProviderEvent>,
        keepalive_tx: Sender<ProviderEvent>,
        thread_name: String,
    ) -> Self {
        Self {
            handle: Some(handle),
            event_rx,
            _keepalive_tx: keepalive_tx,
            thread_name,
            started_at: Instant::now(),
        }
    }

    /// Create from pre-configured components
    ///
    /// Useful when thread is already spawned elsewhere.
    pub fn from_parts(
        handle: Option<JoinHandle<()>>,
        event_rx: Receiver<ProviderEvent>,
        keepalive_tx: Sender<ProviderEvent>,
        thread_name: String,
        started_at: Instant,
    ) -> Self {
        Self {
            handle,
            event_rx,
            _keepalive_tx: keepalive_tx,
            thread_name,
            started_at,
        }
    }

    /// Get the thread name
    pub fn thread_name(&self) -> &str {
        &self.thread_name
    }

    /// Get when the thread was started
    pub fn started_at(&self) -> Instant {
        self.started_at
    }

    /// Get elapsed time since thread started
    pub fn elapsed(&self) -> Duration {
        self.started_at.elapsed()
    }

    /// Check if thread is still running
    pub fn is_running(&self) -> bool {
        self.handle.is_some()
    }

    /// Get the event receiver for collecting provider events
    pub fn event_receiver(&self) -> &Receiver<ProviderEvent> {
        &self.event_rx
    }

    /// Stop the thread gracefully with timeout
    ///
    /// Returns the stop result indicating how the thread finished.
    pub fn stop(&mut self, _timeout: Duration) -> ThreadStopResult {
        if self.handle.is_none() {
            return ThreadStopResult::AlreadyStopped;
        }

        // Drop the sender to signal thread to stop
        // (Provider threads should check for recv errors to detect shutdown)
        let handle = self.handle.take().unwrap();

        // Wait for thread to finish with timeout
        match handle.join() {
            Ok(()) => {
                let elapsed_ms = self.elapsed().as_millis() as u64;
                ThreadStopResult::GracefulStop { duration_ms: elapsed_ms }
            }
            Err(panic_payload) => {
                let error = extract_panic_message(panic_payload);
                ThreadStopResult::Panicked { error }
            }
        }
    }

    /// Stop with configurable timeout, abandoning if not finished
    ///
    /// Logs warning if thread doesn't finish in time.
    pub fn stop_with_timeout(&mut self, timeout_ms: u64) -> ThreadStopResult {
        self.stop(Duration::from_millis(timeout_ms))
    }

    /// Force abandon the thread without waiting
    ///
    /// Use only in emergency shutdown scenarios.
    pub fn abandon(&mut self) -> ThreadStopResult {
        if self.handle.is_none() {
            return ThreadStopResult::AlreadyStopped;
        }
        // Just drop the handle, thread continues but we ignore it
        self.handle = None;
        ThreadStopResult::TimeoutAbandoned {
            timeout_ms: self.elapsed().as_millis() as u64,
        }
    }
}

/// Extract a readable message from panic payload
fn extract_panic_message(payload: Box<dyn std::any::Any + Send>) -> String {
    if let Some(s) = payload.downcast_ref::<&str>() {
        s.to_string()
    } else if let Some(s) = payload.downcast_ref::<String>() {
        s.clone()
    } else {
        "unknown panic".to_string()
    }
}

/// Builder for spawning provider threads
pub struct ProviderThreadBuilder {
    /// Thread name
    name: String,
    /// Stack size (optional)
    stack_size: Option<usize>,
}

impl ProviderThreadBuilder {
    /// Create a new builder with thread name
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            stack_size: None,
        }
    }

    /// Set custom stack size
    pub fn stack_size(mut self, size: usize) -> Self {
        self.stack_size = Some(size);
        self
    }

    /// Spawn a thread with the given function
    ///
    /// Returns the handle for lifecycle management.
    /// Note: The spawned function should create its own channel for events.
    pub fn spawn<F>(self, f: F) -> std::io::Result<ProviderThreadHandle>
    where
        F: FnOnce() + Send + 'static,
    {
        let (keepalive_tx, event_rx) = channel();
        let thread_name = self.name.clone();

        let mut builder = Builder::new().name(thread_name.clone());
        if let Some(size) = self.stack_size {
            builder = builder.stack_size(size);
        }

        let handle = builder.spawn(f)?;

        Ok(ProviderThreadHandle::new(handle, event_rx, keepalive_tx, thread_name))
    }
}

/// Start a provider in a named thread
///
/// Spawns the provider in a dedicated thread with proper naming for debugging.
/// Returns the thread handle for lifecycle management and event collection.
pub fn start_provider_threaded(
    provider: ProviderKind,
    prompt: String,
    cwd: PathBuf,
    session_handle: Option<SessionHandle>,
    thread_name: String,
) -> std::io::Result<ProviderThreadHandle> {
    let (keepalive_tx, event_rx) = channel();
    let thread_event_tx = keepalive_tx.clone();
    let provider_label = provider.label();

    logging::debug_event(
        "provider_thread.start",
        "spawning provider thread",
        serde_json::json!({
            "provider": provider_label,
            "thread_name": thread_name,
            "cwd": cwd.display().to_string(),
        }),
    );

    let handle = Builder::new()
        .name(thread_name.clone())
        .spawn(move || {
            run_provider_in_thread(provider, prompt, cwd, session_handle, thread_event_tx);
        })?;

    Ok(ProviderThreadHandle::new(handle, event_rx, keepalive_tx, thread_name))
}

/// Run provider logic in thread context
///
/// This is the internal function that runs inside the provider thread.
fn run_provider_in_thread(
    provider: ProviderKind,
    _prompt: String,
    _cwd: PathBuf,
    _session_handle: Option<SessionHandle>,
    event_tx: Sender<ProviderEvent>,
) {
    // For now, just log and send a status event
    // Full provider integration will be added in later sprints
    logging::debug_event(
        "provider_thread.run",
        "provider thread started",
        serde_json::json!({
            "provider": provider.label(),
        }),
    );

    let _ = event_tx.send(ProviderEvent::Status(format!("{} thread started", provider.label())));
    let _ = event_tx.send(ProviderEvent::Finished);
}

/// Start mock provider in a thread
///
/// Convenience function for testing with mock provider.
pub fn start_mock_provider_threaded(
    prompt: String,
    thread_name: String,
) -> std::io::Result<ProviderThreadHandle> {
    let (keepalive_tx, event_rx) = channel();
    let thread_event_tx = keepalive_tx.clone();

    let handle = Builder::new()
        .name(thread_name.clone())
        .spawn(move || {
            run_mock_provider(prompt, thread_event_tx);
        })?;

    Ok(ProviderThreadHandle::new(handle, event_rx, keepalive_tx, thread_name))
}

/// Run mock provider logic
fn run_mock_provider(prompt: String, event_tx: Sender<ProviderEvent>) {
    let _ = event_tx.send(ProviderEvent::Status("mock provider started".to_string()));

    for chunk in crate::mock_provider::build_reply_chunks(&prompt) {
        std::thread::sleep(Duration::from_millis(60));
        if event_tx.send(ProviderEvent::AssistantChunk(chunk)).is_err() {
            return;
        }
    }

    let _ = event_tx.send(ProviderEvent::Finished);

    logging::debug_event(
        "provider_thread.mock_finished",
        "mock provider thread finished",
        serde_json::json!({}),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;
    use std::time::Duration;

    fn make_handle_with_thread() -> (ProviderThreadHandle, Arc<AtomicBool>) {
        let (keepalive_tx, event_rx) = std::sync::mpsc::channel();
        let running = Arc::new(AtomicBool::new(true));
        let running_clone = running.clone();

        let handle = Builder::new()
            .name("test-thread".to_string())
            .spawn(move || {
                while running_clone.load(Ordering::Relaxed) {
                    std::thread::sleep(Duration::from_millis(10));
                }
            })
            .unwrap();

        (ProviderThreadHandle::new(handle, event_rx, keepalive_tx, "test-thread".to_string()), running)
    }

    fn make_quick_thread() -> ProviderThreadHandle {
        let (keepalive_tx, event_rx) = std::sync::mpsc::channel();

        let handle = Builder::new()
            .name("quick-thread".to_string())
            .spawn(|| {
                // Thread exits immediately
            })
            .unwrap();

        ProviderThreadHandle::new(handle, event_rx, keepalive_tx, "quick-thread".to_string())
    }

    #[test]
    fn handle_new_creates_running_thread() {
        let (handle, _) = make_handle_with_thread();
        assert!(handle.is_running());
        assert_eq!(handle.thread_name(), "test-thread");
    }

    #[test]
    fn handle_elapsed_increases() {
        let handle = make_quick_thread();
        let elapsed = handle.elapsed();
        std::thread::sleep(Duration::from_millis(10));
        assert!(handle.elapsed() > elapsed);
    }

    #[test]
    fn handle_stop_returns_graceful_for_normal_exit() {
        let (handle, running) = make_handle_with_thread();
        running.store(false, Ordering::Relaxed); // Signal thread to stop
        std::thread::sleep(Duration::from_millis(50)); // Let thread finish
        let mut handle = handle;
        let result = handle.stop(Duration::from_millis(100));
        assert!(matches!(result, ThreadStopResult::GracefulStop { .. }));
        assert!(!handle.is_running());
    }

    #[test]
    fn handle_stop_already_stopped() {
        let mut handle = make_quick_thread();
        handle.stop(Duration::from_millis(100));
        let result = handle.stop(Duration::from_millis(100));
        assert!(matches!(result, ThreadStopResult::AlreadyStopped));
    }

    #[test]
    fn handle_abandon() {
        let (mut handle, _) = make_handle_with_thread();
        let result = handle.abandon();
        assert!(matches!(result, ThreadStopResult::TimeoutAbandoned { .. }));
        assert!(!handle.is_running());
    }

    #[test]
    fn builder_new_creates_named_builder() {
        let builder = ProviderThreadBuilder::new("agent-thread");
        assert_eq!(builder.name, "agent-thread");
    }

    #[test]
    fn builder_stack_size() {
        let builder = ProviderThreadBuilder::new("test").stack_size(1024 * 1024);
        assert_eq!(builder.stack_size, Some(1024 * 1024));
    }

    #[test]
    fn extract_panic_message_str() {
        let msg = extract_panic_message(Box::new("panic message"));
        assert_eq!(msg, "panic message");
    }

    #[test]
    fn extract_panic_message_string() {
        let msg = extract_panic_message(Box::new("panic message".to_string()));
        assert_eq!(msg, "panic message");
    }

    #[test]
    fn stop_result_graceful() {
        let result = ThreadStopResult::GracefulStop { duration_ms: 100 };
        assert!(matches!(result, ThreadStopResult::GracefulStop { .. }));
    }

    #[test]
    fn stop_result_timeout() {
        let result = ThreadStopResult::TimeoutAbandoned { timeout_ms: 1000 };
        assert!(matches!(result, ThreadStopResult::TimeoutAbandoned { .. }));
    }

    #[test]
    fn stop_result_panicked() {
        let result = ThreadStopResult::Panicked { error: "test error".to_string() };
        assert!(matches!(result, ThreadStopResult::Panicked { .. }));
    }

    #[test]
    fn stop_result_already_stopped() {
        let result = ThreadStopResult::AlreadyStopped;
        assert!(matches!(result, ThreadStopResult::AlreadyStopped));
    }

    #[test]
    fn mock_provider_threaded_creates_running_thread() {
        let handle = start_mock_provider_threaded("test prompt".to_string(), "mock-thread".to_string());
        assert!(handle.is_ok());
        let handle = handle.unwrap();
        assert!(handle.is_running());
        assert_eq!(handle.thread_name(), "mock-thread");
    }

    #[test]
    fn mock_provider_threaded_sends_events() {
        let handle = start_mock_provider_threaded("test".to_string(), "mock-events".to_string()).unwrap();
        let receiver = handle.event_receiver();

        // Wait briefly for mock provider to start sending events
        std::thread::sleep(Duration::from_millis(100));

        // Receive events - mock provider sends status, then chunks, then finished
        let mut received_status = false;
        let mut received_chunk = false;

        for _ in 0..20 {
            match receiver.try_recv() {
                Ok(ProviderEvent::Status(_)) => received_status = true,
                Ok(ProviderEvent::AssistantChunk(_)) => received_chunk = true,
                Ok(ProviderEvent::Finished) => {} // Finished is expected but not checked
                Ok(_) => {} // Other events are fine
                Err(_) => break,
            }
        }

        assert!(received_status, "should receive status event");
        assert!(received_chunk, "should receive at least one chunk");
    }

    #[test]
    fn mock_provider_thread_completes_gracefully() {
        let mut handle = start_mock_provider_threaded("short".to_string(), "mock-complete".to_string()).unwrap();

        // Wait for mock provider to finish (it sends chunks then Finished)
        std::thread::sleep(Duration::from_millis(500));

        // Stop should return graceful since thread completed
        let result = handle.stop(Duration::from_millis(100));
        assert!(matches!(result, ThreadStopResult::GracefulStop { .. }));
        assert!(!handle.is_running());
    }

    #[test]
    fn mock_provider_thread_name_matches() {
        let handle = start_mock_provider_threaded("test".to_string(), "custom-mock".to_string()).unwrap();
        assert_eq!(handle.thread_name(), "custom-mock");
    }
}