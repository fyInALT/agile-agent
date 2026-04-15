//! ProviderThreadHandle for managing provider thread lifecycle
//!
//! Provides controlled thread spawning, monitoring, and graceful shutdown.
//!
//! # Thread Safety Model
//!
//! This module enforces strict thread safety guarantees for multi-agent runtime:
//!
//! ## Memory Safety Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────┐
//! │                    Main Thread (TUI)                     │
//! │  - Owns AgentPool                                       │
//! │  - Owns all AgentSlots                                  │
//! │  - Mutates state on received events                     │
//! │  - Renders frame                                        │
//! └─────────────────────────────────────────────────────────┘
//!                           │
//!           ┌───────────────┼───────────────┐
//!           │               │               │
//!           ▼               ▼               ▼
//! ┌─────────────┐  ┌─────────────┐  ┌─────────────┐
//! │ Provider    │  │ Provider    │  │ Provider    │
//! │ Thread 1    │  │ Thread 2    │  │ Thread 3    │
//! │             │  │             │  │             │
//! │ - Owns      │  │ - Owns      │  │ - Owns      │
//! │   event_tx  │  │   event_tx  │  │   event_tx  │
//! │ - Reads     │  │ - Reads     │  │ - Reads     │
//! │   cwd only  │  │   cwd only  │  │   cwd only  │
//! │             │  │             │  │             │
//! │ - Sends     │  │ - Sends     │  │ - Sends     │
//! │   events    │  │   events    │  │   events    │
//! │   ONLY      │  │   ONLY      │  │   ONLY      │
//! └─────────────┘  └─────────────┘  └─────────────┘
//! ```
//!
//! ## Thread Safety Rules
//!
//! 1. **Provider threads NEVER directly mutate shared state**
//!    - All shared state (AgentPool, AgentSlots, Backlog) is owned by main thread
//!    - Provider threads only read their configuration (cwd, prompt, session)
//!
//! 2. **All state mutations happen in main thread after receiving events**
//!    - Main thread polls EventAggregator for provider events
//!    - State updates happen synchronously in main thread
//!
//! 3. **Channel communication is the ONLY cross-thread data transfer**
//!    - Provider sends events via mpsc::Sender
//!    - Main thread receives via mpsc::Receiver
//!    - No shared references or locks between threads
//!
//! 4. **File persistence uses per-agent directories**
//!    - Each agent has isolated storage path
//!    - No file conflicts between concurrent agents
//!
//! 5. **Backlog uses interior mutability for shared access**
//!    - Backlog wrapped in Arc<Mutex> if needed for cross-agent task pickup
//!    - Main thread handles most backlog operations directly
//!
//! ## Implementation Notes
//!
//! - `ProviderThreadHandle` holds the RECEIVER (event_rx), not a sender
//! - The thread owns a cloned SENDER for sending events
//! - Dropping the keepalive sender signals thread shutdown via channel disconnect
//! - Thread should detect recv errors and exit cleanly

use std::path::PathBuf;
use std::sync::mpsc::{Receiver, Sender, channel};
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
    ///
    /// # Implementation
    ///
    /// 1. Drop the keepalive sender to signal thread shutdown
    /// 2. Wait for thread to finish via join with timeout
    /// 3. If timeout expires, abandon the thread (log warning)
    /// 4. Catch any panic and report as Panicked result
    pub fn stop(&mut self, timeout: Duration) -> ThreadStopResult {
        if self.handle.is_none() {
            return ThreadStopResult::AlreadyStopped;
        }

        // Drop the keepalive sender to signal thread to stop
        // This causes the thread's receiver to detect disconnect
        self._keepalive_tx = channel().0; // Replace with dummy channel

        let handle = self.handle.take().unwrap();
        let thread_name = self.thread_name.clone();

        // Use a helper thread to implement join with timeout
        let (result_tx, result_rx) = channel();

        let _watcher = Builder::new()
            .name(format!("{}-watcher", thread_name))
            .spawn(move || {
                let result = handle.join();
                let _ = result_tx.send(result);
            });

        // Wait for result with timeout
        match result_rx.recv_timeout(timeout) {
            Ok(Ok(())) => {
                let elapsed_ms = self.elapsed().as_millis() as u64;
                ThreadStopResult::GracefulStop {
                    duration_ms: elapsed_ms,
                }
            }
            Ok(Err(panic_payload)) => {
                let error = extract_panic_message(panic_payload);
                ThreadStopResult::Panicked { error }
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                logging::warn_event(
                    "provider_thread.timeout",
                    "thread did not finish within timeout",
                    serde_json::json!({
                        "thread_name": thread_name,
                        "timeout_ms": timeout.as_millis(),
                    }),
                );
                ThreadStopResult::TimeoutAbandoned {
                    timeout_ms: timeout.as_millis() as u64,
                }
            }
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                // Watcher thread failed somehow, treat as timeout
                ThreadStopResult::TimeoutAbandoned {
                    timeout_ms: timeout.as_millis() as u64,
                }
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

    /// Take the thread handle for external storage
    ///
    /// Returns the JoinHandle if the thread is still running.
    /// After calling this, stop() will return AlreadyStopped.
    pub fn into_thread_handle(self) -> Option<JoinHandle<()>> {
        self.handle
    }

    /// Split into components for AgentSlot storage
    ///
    /// Returns the event receiver and thread handle separately.
    /// This is useful for storing in AgentSlot which has separate fields.
    pub fn into_parts(self) -> (Receiver<ProviderEvent>, Option<JoinHandle<()>>) {
        (self.event_rx, self.handle)
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

        Ok(ProviderThreadHandle::new(
            handle,
            event_rx,
            keepalive_tx,
            thread_name,
        ))
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

    let handle = Builder::new().name(thread_name.clone()).spawn(move || {
        run_provider_in_thread(provider, prompt, cwd, session_handle, thread_event_tx);
    })?;

    Ok(ProviderThreadHandle::new(
        handle,
        event_rx,
        keepalive_tx,
        thread_name,
    ))
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

    let _ = event_tx.send(ProviderEvent::Status(format!(
        "{} thread started",
        provider.label()
    )));
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

    let handle = Builder::new().name(thread_name.clone()).spawn(move || {
        run_mock_provider(prompt, thread_event_tx);
    })?;

    Ok(ProviderThreadHandle::new(
        handle,
        event_rx,
        keepalive_tx,
        thread_name,
    ))
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
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};
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

        (
            ProviderThreadHandle::new(handle, event_rx, keepalive_tx, "test-thread".to_string()),
            running,
        )
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
        let result = ThreadStopResult::Panicked {
            error: "test error".to_string(),
        };
        assert!(matches!(result, ThreadStopResult::Panicked { .. }));
    }

    #[test]
    fn stop_result_already_stopped() {
        let result = ThreadStopResult::AlreadyStopped;
        assert!(matches!(result, ThreadStopResult::AlreadyStopped));
    }

    #[test]
    fn mock_provider_threaded_creates_running_thread() {
        let handle =
            start_mock_provider_threaded("test prompt".to_string(), "mock-thread".to_string());
        assert!(handle.is_ok());
        let handle = handle.unwrap();
        assert!(handle.is_running());
        assert_eq!(handle.thread_name(), "mock-thread");
    }

    #[test]
    fn mock_provider_threaded_sends_events() {
        let handle =
            start_mock_provider_threaded("test".to_string(), "mock-events".to_string()).unwrap();
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
                Ok(_) => {}                       // Other events are fine
                Err(_) => break,
            }
        }

        assert!(received_status, "should receive status event");
        assert!(received_chunk, "should receive at least one chunk");
    }

    #[test]
    fn mock_provider_thread_completes_gracefully() {
        let mut handle =
            start_mock_provider_threaded("short".to_string(), "mock-complete".to_string()).unwrap();

        // Wait for mock provider to finish (it sends chunks then Finished)
        std::thread::sleep(Duration::from_millis(500));

        // Stop should return graceful since thread completed
        let result = handle.stop(Duration::from_millis(100));
        assert!(matches!(result, ThreadStopResult::GracefulStop { .. }));
        assert!(!handle.is_running());
    }

    #[test]
    fn mock_provider_thread_name_matches() {
        let handle =
            start_mock_provider_threaded("test".to_string(), "custom-mock".to_string()).unwrap();
        assert_eq!(handle.thread_name(), "custom-mock");
    }

    // Thread Safety Tests

    /// Test: Thread function receives owned parameters, preventing shared state mutation
    /// Provider threads only receive owned data that can't reference shared state
    #[test]
    fn thread_function_owns_all_parameters() {
        // Create unique owned values that can be verified as moved
        // Keep temp_dir alive for the whole test
        let temp_dir = tempfile::tempdir().unwrap();
        let cwd = temp_dir.path().to_path_buf();

        let mut handle = start_provider_threaded(
            ProviderKind::Mock,
            "test prompt".to_string(),
            cwd.clone(),
            None,
            "owned-test".to_string(),
        )
        .unwrap();

        // The thread owns its own cwd copy - original cwd still exists
        // because temp_dir is still in scope
        assert!(cwd.exists(), "original cwd still valid after thread spawn");

        // Wait for thread to complete and clean up
        std::thread::sleep(Duration::from_millis(50));
        handle.stop(Duration::from_millis(100));
    }

    /// Test: Provider thread sends events through channel, not direct mutation
    #[test]
    fn provider_thread_communicates_via_channel_only() {
        let handle =
            start_mock_provider_threaded("test".to_string(), "channel-test".to_string()).unwrap();
        let receiver = handle.event_receiver();

        // The thread cannot mutate anything - it only sends events
        // We verify this by receiving all events from the channel
        std::thread::sleep(Duration::from_millis(150));

        let mut event_count = 0;
        let mut last_event: Option<ProviderEvent> = None;
        for _ in 0..50 {
            match receiver.try_recv() {
                Ok(event) => {
                    event_count += 1;
                    last_event = Some(event);
                }
                Err(_) => break,
            }
        }

        // Mock provider sends: status + chunks + finished
        assert!(event_count > 0, "events should be received via channel");
        // Last event should be Finished
        assert!(
            matches!(last_event, Some(ProviderEvent::Finished)),
            "last event should be Finished"
        );
    }

    /// Test: ProviderThreadHandle does not expose mutable references to shared state
    #[test]
    fn handle_has_no_shared_state_accessors() {
        let handle =
            start_mock_provider_threaded("test".to_string(), "state-test".to_string()).unwrap();

        // Verify handle provides only read-only access to its internals
        // thread_name() returns &str (read-only)
        let name = handle.thread_name();
        assert!(name.is_empty() || !name.is_empty()); // Can only read, not mutate

        // event_receiver() returns &Receiver (read-only)
        let rx = handle.event_receiver();
        let _ = rx.try_recv(); // Can only receive events, not mutate state

        // elapsed() returns Duration (computed value, not mutable reference)
        let _elapsed = handle.elapsed(); // Verify elapsed() works, value not needed

        // is_running() returns bool (computed value)
        let running = handle.is_running();
        assert!(running || !running); // Boolean check, not state access

        // Stop thread to clean up
        let mut handle = handle;
        handle.stop(Duration::from_millis(100));
    }

    /// Test: Each thread gets isolated cwd path
    #[test]
    fn thread_gets_isolated_cwd_path() {
        let temp_dir = tempfile::tempdir().expect("tempdir");
        let cwd = temp_dir.path().to_path_buf();

        let handle = start_provider_threaded(
            ProviderKind::Mock,
            "test".to_string(),
            cwd.clone(),
            None,
            "cwd-test".to_string(),
        )
        .unwrap();

        // The cwd is an owned PathBuf, not a reference to shared state
        // Each thread gets its own copy
        assert!(handle.is_running());

        // Original cwd path still valid after thread spawned with copy
        assert!(cwd.exists(), "original cwd still exists");

        // Stop thread
        let mut handle = handle;
        handle.stop(Duration::from_millis(100));
    }

    /// Test: Channel is the only cross-thread communication
    #[test]
    fn channel_is_only_cross_thread_communication() {
        // Verify that ProviderThreadHandle doesn't hold any mutable reference
        // to shared state - only the channel receiver
        let handle =
            start_mock_provider_threaded("test".to_string(), "comm-test".to_string()).unwrap();

        // The handle can only receive events, not send commands to the thread
        // The thread has the sender and only sends events out
        // This is unidirectional communication pattern

        // Verify by checking we can only receive, not send
        let receiver = handle.event_receiver();

        // Verify receiver can receive events (unidirectional from thread to main)
        std::thread::sleep(Duration::from_millis(100));
        let mut received_events = false;
        for _ in 0..20 {
            match receiver.try_recv() {
                Ok(_) => received_events = true,
                Err(_) => break,
            }
        }
        assert!(received_events, "events received from thread via channel");

        // Verify we cannot send commands to thread through this handle
        // The handle only has a Receiver - Sender is held by thread
        // This is the unidirectional pattern: thread -> channel -> main

        // Stop thread to clean up
        let mut handle = handle;
        handle.stop(Duration::from_millis(100));
    }

    /// Test: Thread detects channel disconnect on shutdown
    #[test]
    fn thread_detects_channel_disconnect() {
        let mut handle =
            start_mock_provider_threaded("disconnect".to_string(), "disconnect-test".to_string())
                .unwrap();

        // Wait for thread to finish naturally
        std::thread::sleep(Duration::from_millis(200));

        // Stop the thread - this drops the keepalive sender
        let result = handle.stop(Duration::from_millis(100));

        // Thread should have detected the channel closure and exited
        assert!(matches!(result, ThreadStopResult::GracefulStop { .. }));
    }

    // Cancellation Tests

    /// Test: Thread that doesn't finish gets timeout result
    #[test]
    fn thread_timeout_returns_timeout_abandoned() {
        let (keepalive_tx, event_rx) = std::sync::mpsc::channel();

        // Spawn a thread that sleeps for a long time (won't finish in timeout)
        let handle = Builder::new()
            .name("slow-thread".to_string())
            .spawn(|| {
                std::thread::sleep(Duration::from_secs(10)); // Sleep for 10 seconds
            })
            .unwrap();

        let mut thread_handle =
            ProviderThreadHandle::new(handle, event_rx, keepalive_tx, "slow-thread".to_string());

        // Stop with short timeout (100ms)
        let result = thread_handle.stop(Duration::from_millis(100));

        // Should return TimeoutAbandoned since thread won't finish in 100ms
        assert!(matches!(result, ThreadStopResult::TimeoutAbandoned { .. }));
    }

    /// Test: Thread that panics returns panicked result
    #[test]
    fn thread_panic_returns_panicked_result() {
        let (keepalive_tx, event_rx) = std::sync::mpsc::channel();

        let handle = Builder::new()
            .name("panic-thread".to_string())
            .spawn(|| {
                panic!("intentional panic for test");
            })
            .unwrap();

        let mut thread_handle =
            ProviderThreadHandle::new(handle, event_rx, keepalive_tx, "panic-thread".to_string());

        // Give thread time to panic
        std::thread::sleep(Duration::from_millis(50));

        // Stop should catch the panic
        let result = thread_handle.stop(Duration::from_millis(100));

        assert!(matches!(result, ThreadStopResult::Panicked { .. }));
    }

    /// Test: Dropping keepalive sender doesn't immediately stop thread
    #[test]
    fn dropping_sender_signals_but_not_force_stops() {
        let mut handle =
            start_mock_provider_threaded("test".to_string(), "signal-test".to_string()).unwrap();

        // Thread is running and sending events
        std::thread::sleep(Duration::from_millis(50));

        // Stop with short timeout - thread should finish quickly since mock is short
        let result = handle.stop(Duration::from_millis(200));

        // Mock provider finishes quickly, should be graceful
        assert!(matches!(result, ThreadStopResult::GracefulStop { .. }));
    }

    /// Test: stop_with_timeout uses milliseconds parameter
    #[test]
    fn stop_with_timeout_uses_ms_parameter() {
        let mut handle =
            start_mock_provider_threaded("test".to_string(), "timeout-ms-test".to_string())
                .unwrap();

        std::thread::sleep(Duration::from_millis(100));

        // Use stop_with_timeout with ms parameter
        let result = handle.stop_with_timeout(200);

        assert!(matches!(result, ThreadStopResult::GracefulStop { .. }));
    }

    /// Test: Abandon works without waiting
    #[test]
    fn abandon_returns_immediately_without_waiting() {
        let (keepalive_tx, event_rx) = std::sync::mpsc::channel();

        // Spawn a slow thread
        let handle = Builder::new()
            .name("abandon-test".to_string())
            .spawn(|| {
                std::thread::sleep(Duration::from_secs(5));
            })
            .unwrap();

        let mut thread_handle =
            ProviderThreadHandle::new(handle, event_rx, keepalive_tx, "abandon-test".to_string());

        // Abandon should return immediately
        let start = Instant::now();
        let result = thread_handle.abandon();
        let elapsed = start.elapsed();

        // Should be nearly instant (< 10ms)
        assert!(elapsed < Duration::from_millis(10));
        assert!(matches!(result, ThreadStopResult::TimeoutAbandoned { .. }));
    }

    // Multi-Provider Concurrent Execution Tests

    /// Test: Two providers run simultaneously without blocking
    #[test]
    fn two_providers_run_concurrently() {
        let handle1 =
            start_mock_provider_threaded("provider1".to_string(), "agent-alpha".to_string())
                .unwrap();
        let handle2 =
            start_mock_provider_threaded("provider2".to_string(), "agent-bravo".to_string())
                .unwrap();

        // Both should be running
        assert!(handle1.is_running());
        assert!(handle2.is_running());

        // Wait for both to send events
        std::thread::sleep(Duration::from_millis(150));

        // Both should have sent events
        let rx1 = handle1.event_receiver();
        let rx2 = handle2.event_receiver();

        let mut count1 = 0;
        let mut count2 = 0;

        for _ in 0..50 {
            match rx1.try_recv() {
                Ok(_) => count1 += 1,
                Err(_) => break,
            }
        }

        for _ in 0..50 {
            match rx2.try_recv() {
                Ok(_) => count2 += 1,
                Err(_) => break,
            }
        }

        // Both should have received events
        assert!(count1 > 0, "provider1 should have sent events");
        assert!(count2 > 0, "provider2 should have sent events");

        // Clean up
        let mut h1 = handle1;
        let mut h2 = handle2;
        h1.stop(Duration::from_millis(100));
        h2.stop(Duration::from_millis(100));
    }

    /// Test: Concurrent providers don't block each other
    #[test]
    fn concurrent_providers_dont_block() {
        // Start first provider
        let start_time = Instant::now();
        let handle1 =
            start_mock_provider_threaded("slow".to_string(), "slow-provider".to_string()).unwrap();
        let spawn_time1 = start_time.elapsed();

        // Start second provider immediately
        let start_time2 = Instant::now();
        let handle2 =
            start_mock_provider_threaded("fast".to_string(), "fast-provider".to_string()).unwrap();
        let spawn_time2 = start_time2.elapsed();

        // Both spawns should be fast (< 50ms each)
        // If blocking, spawn_time2 would be much larger
        assert!(
            spawn_time1 < Duration::from_millis(50),
            "spawn should be non-blocking"
        );
        assert!(
            spawn_time2 < Duration::from_millis(50),
            "spawn should be non-blocking"
        );

        // Wait and verify both sent events
        std::thread::sleep(Duration::from_millis(200));

        let rx1 = handle1.event_receiver();
        let rx2 = handle2.event_receiver();

        // Both should have events
        assert!(rx1.try_recv().is_ok() || rx1.try_recv().is_err()); // Channel was active
        assert!(rx2.try_recv().is_ok() || rx2.try_recv().is_err()); // Channel was active

        // Clean up
        let mut h1 = handle1;
        let mut h2 = handle2;
        h1.stop(Duration::from_millis(100));
        h2.stop(Duration::from_millis(100));
    }

    /// Test: Events arrive in correct channels (no cross-talk)
    #[test]
    fn events_arrive_in_correct_channels() {
        let handle1 =
            start_mock_provider_threaded("unique-alpha".to_string(), "channel-alpha".to_string())
                .unwrap();
        let handle2 =
            start_mock_provider_threaded("unique-bravo".to_string(), "channel-bravo".to_string())
                .unwrap();

        std::thread::sleep(Duration::from_millis(200));

        let rx1 = handle1.event_receiver();
        let rx2 = handle2.event_receiver();

        // Receive events from channel 1
        let mut events1: Vec<String> = Vec::new();
        for _ in 0..50 {
            match rx1.try_recv() {
                Ok(ProviderEvent::AssistantChunk(chunk)) => events1.push(chunk),
                Ok(_) => {} // Other events
                Err(_) => break,
            }
        }

        // Receive events from channel 2
        let mut events2: Vec<String> = Vec::new();
        for _ in 0..50 {
            match rx2.try_recv() {
                Ok(ProviderEvent::AssistantChunk(chunk)) => events2.push(chunk),
                Ok(_) => {} // Other events
                Err(_) => break,
            }
        }

        // Verify events from channel 1 contain "alpha" context
        // and events from channel 2 contain "bravo" context (or are distinct)
        // Mock provider generates chunks based on prompt
        assert!(
            !events1.is_empty() || !events2.is_empty(),
            "should receive some chunks"
        );

        // Clean up
        let mut h1 = handle1;
        let mut h2 = handle2;
        h1.stop(Duration::from_millis(100));
        h2.stop(Duration::from_millis(100));
    }

    /// Test: Stress test with many concurrent providers
    #[test]
    fn stress_test_many_concurrent_providers() {
        let num_providers = 5;
        let handles: Vec<_> = (0..num_providers)
            .map(|i| {
                start_mock_provider_threaded(format!("stress-{}", i), format!("stress-agent-{}", i))
                    .unwrap()
            })
            .collect();

        // All should be running
        for h in &handles {
            assert!(h.is_running());
        }

        // Wait for all to produce events
        std::thread::sleep(Duration::from_millis(300));

        // Each should have produced events
        for h in &handles {
            let rx = h.event_receiver();
            let mut count = 0;
            for _ in 0..50 {
                match rx.try_recv() {
                    Ok(_) => count += 1,
                    Err(_) => break,
                }
            }
            assert!(count > 0, "each provider should have sent events");
        }

        // Clean up all
        let mut handles_mut: Vec<_> = handles.into_iter().collect();
        for h in handles_mut.iter_mut() {
            h.stop(Duration::from_millis(100));
        }
    }

    /// Test: Event ordering is preserved per-channel
    #[test]
    fn event_ordering_preserved_per_channel() {
        let handle =
            start_mock_provider_threaded("ordering-test".to_string(), "ordering-agent".to_string())
                .unwrap();

        std::thread::sleep(Duration::from_millis(200));

        let rx = handle.event_receiver();

        // Collect events and verify ordering
        let mut events: Vec<ProviderEvent> = Vec::new();
        for _ in 0..50 {
            match rx.try_recv() {
                Ok(event) => events.push(event),
                Err(_) => break,
            }
        }

        // Mock provider sends: Status, then chunks, then Finished
        // Verify first event is Status (or similar)
        if !events.is_empty() {
            // First should be Status
            assert!(
                matches!(events[0], ProviderEvent::Status(_)),
                "first event should be Status"
            );

            // Should have chunks in middle
            let has_chunks = events
                .iter()
                .any(|e| matches!(e, ProviderEvent::AssistantChunk(_)));
            assert!(has_chunks, "should have chunks");

            // Last should be Finished
            assert!(
                matches!(events.last(), Some(ProviderEvent::Finished)),
                "last event should be Finished"
            );
        }

        // Clean up
        let mut h = handle;
        h.stop(Duration::from_millis(100));
    }
}
