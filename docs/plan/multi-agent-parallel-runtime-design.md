# Multi-Agent Parallel Runtime Design

## Metadata

- Status: `Draft`
- Created: 2026-04-13
- Target Phase: `Phase 2 - Parallel Agent Runtime`
- Language policy: English

## 1. Executive Summary

This document outlines the architectural changes required to support multiple concurrent agent runtimes based on Codex, Claude, and Opencode providers, executing in parallel via multi-threading to prevent single-runtime blocking of the workspace.

### Key Goals

1. **Parallel Execution**: Multiple agents can execute simultaneously without blocking each other
2. **Provider Diversity**: Support mixing Claude, Codex, and Opencode agents in the same workplace
3. **State Isolation**: Each agent maintains independent transcript, session, and task state
4. **Observable Coordination**: TUI can display status of all active agents
5. **Safe Persistence**: Concurrent persistence must not corrupt state

## 2. Current Architecture Analysis

### 2.1 Single-Agent Model

Current implementation follows a strict single-agent model:

```
┌─────────────────────────────────────────────────────┐
│                    TuiState                          │
│  ┌─────────────────┬───────────────────────────┐    │
│  │   RuntimeSession│                           │    │
│  │  ┌───────────┬────────────────┐            │    │
│  │  │ AppState  │ AgentRuntime   │            │    │
│  │  │           │                │            │    │
│  │  │ - trans-  │ - meta         │            │    │
│  │  │   cript   │ - workplace    │            │    │
│  │  │ - status  │ - provider_    │            │    │
│  │  │ - input   │   binding      │            │    │
│  │  └───────────┴────────────────┘            │    │
│  └─────────────────────────────────────────────┘    │
└─────────────────────────────────────────────────────┘
         │
         ▼
    Provider Thread (single)
    ┌────────────────────────────────┐
    │  mpsc::channel<ProviderEvent>  │
    │  - AssistantChunk              │
    │  - ThinkingChunk               │
    │  - ToolCallStarted/Finished    │
    │  - Finished                    │
    └────────────────────────────────┘
```

### 2.2 Key Components

| Component | Current Role | Multi-Agent Impact |
|-----------|--------------|---------------------|
| `AgentRuntime` | Single agent identity | Need AgentRuntimePool |
| `RuntimeSession` | AppState + AgentRuntime | Need per-agent sessions |
| `AppState` | Global state container | Split into per-agent + shared |
| `AgentStore` | Persistence for one agent | Thread-safe multi-agent store |
| `Provider::start()` | Single thread spawn | Thread pool management |
| `TuiState` | Single session view | Multi-agent dashboard |

### 2.3 Blocking Points in Current Design

1. **Provider Event Loop**: `app_loop.rs` waits on single `provider_rx`
2. **State Mutation**: `AppState` is mutated directly during event processing
3. **Persistence Timing**: `persist_if_changed()` is synchronous
4. **Task Assignment**: `active_task_id` is a single optional value
5. **Transcript Rendering**: Single transcript view

## 3. Proposed Architecture

### 3.1 High-Level Model

```
┌────────────────────────────────────────────────────────────────┐
│                        WorkplaceContext                          │
│  ┌─────────────────────────────────────────────────────────┐    │
│  │                    SharedWorkplaceState                  │    │
│  │  - backlog (shared todos/tasks)                         │    │
│  │  - workplace_id                                         │    │
│  │  - skills_registry                                      │    │
│  └─────────────────────────────────────────────────────────┘    │
│                                                                  │
│  ┌────────────────────────────────────────────────────────────┐ │
│  │                    AgentPool                               │ │
│  │  ┌──────────────┐ ┌──────────────┐ ┌──────────────┐        │ │
│  │  │ AgentSlot 0  │ │ AgentSlot 1  │ │ AgentSlot 2  │ ...    │ │
│  │  │ ┌──────────┐ │ │ ┌──────────┐ │ │ ┌──────────┐ │        │ │
│  │  │ │Claude    │ │ │ │Codex     │ │ │ │Opencode  │ │        │ │
│  │  │ │Session   │ │ │ │Thread    │ │ │ │Session   │ │        │ │
│  │  │ └──────────┘ │ │ └──────────┘ │ │ └──────────┘ │        │ │
│  │  │              │ │              │ │              │        │ │
│  │  │ - status    │ │ - status     │ │ - status     │        │ │
│  │  │ - transcript│ │ - transcript │ │ - transcript │        │ │
│  │  │ - task_id   │ │ - task_id    │ │ - task_id    │        │ │
│  │  │ - rx/Tx     │ │ - rx/Tx      │ │ - rx/Tx      │        │ │
│  │  └──────────────┘ └──────────────┘ └──────────────┘        │ │
│  └────────────────────────────────────────────────────────────┘ │
│                                                                  │
│  ┌────────────────────────────────────────────────────────────┐ │
│  │                    EventAggregator                          │ │
│  │  - polls all agent event channels                          │ │
│  │  - dispatches events to TUI/state                          │ │
│  │  - handles cross-agent coordination                        │ │
│  └────────────────────────────────────────────────────────────┘ │
└────────────────────────────────────────────────────────────────┘
```

### 3.2 Core Data Structures

#### 3.2.1 AgentSlot

Each agent occupies a slot with independent state:

```rust
pub struct AgentSlot {
    /// Unique agent identifier
    agent_id: AgentId,
    
    /// Agent display codename (alpha, bravo, etc.)
    codename: AgentCodename,
    
    /// Provider type binding
    provider_type: ProviderType,
    
    /// Current runtime status
    status: AgentSlotStatus,
    
    /// Provider session handle for multi-turn continuity
    session_handle: Option<SessionHandle>,
    
    /// Per-agent transcript (copy for TUI rendering)
    transcript: Vec<TranscriptEntry>,
    
    /// Currently assigned task (if any)
    assigned_task_id: Option<TaskId>,
    
    /// Event channel receiver from provider thread
    event_rx: Option<mpsc::Receiver<ProviderEvent>>,
    
    /// Thread handle for join/cancel operations
    thread_handle: Option<std::thread::JoinHandle<()>>,
    
    /// Last activity timestamp
    last_activity: Instant,
}

pub enum AgentSlotStatus {
    Idle,
    Starting,
    Responding { started_at: Instant },
    ToolExecuting { tool_name: String },
    Finishing,
    Stopped { reason: String },
    Error { message: String },
}
```

#### 3.2.2 AgentPool

Central coordination structure:

```rust
pub struct AgentPool {
    /// All active agent slots
    slots: Vec<AgentSlot>,
    
    /// Shared workplace reference
    workplace: WorkplaceStore,
    
    /// Max concurrent agents (configurable)
    max_slots: usize,
    
    /// Agent index counter for generating new IDs
    next_agent_index: usize,
    
    /// Index of the currently focused agent (for TUI)
    focused_slot: usize,
}

impl AgentPool {
    /// Spawn a new agent with specified provider
    pub fn spawn_agent(&mut self, provider: ProviderKind, cwd: PathBuf) -> Result<AgentId>;
    
    /// Stop a specific agent
    pub fn stop_agent(&mut self, agent_id: &AgentId) -> Result<AgentStopResult>;
    
    /// Assign a task to an idle agent
    pub fn assign_task(&mut self, agent_id: &AgentId, task_id: TaskId) -> Result<()>;
    
    /// Get all agents with their current status
    pub fn agent_statuses(&self) -> Vec<AgentStatusSnapshot>;
    
    /// Poll events from all active agents
    pub fn poll_events(&mut self, timeout: Duration) -> Vec<AgentEvent>;
    
    /// Switch focus to different agent (TUI)
    pub fn focus_agent(&mut self, index: usize) -> Result<()>;
}
```

#### 3.2.3 SharedWorkplaceState

State shared across all agents:

```rust
pub struct SharedWorkplaceState {
    /// Workplace identity
    workplace_id: WorkplaceId,
    
    /// Shared backlog (todos and tasks)
    backlog: BacklogState,
    
    /// Skills registry (shared configuration)
    skills: SkillRegistry,
    
    /// Current working directory
    cwd: PathBuf,
    
    /// Composer input (before submission)
    input: String,
    
    /// Global loop control
    loop_run_active: bool,
    remaining_loop_iterations: usize,
    
    /// Global app flags
    should_quit: bool,
}
```

#### 3.2.4 AgentEvent

Unified event type for cross-agent communication:

```rust
pub enum AgentEvent {
    /// Event from a specific agent's provider
    FromAgent {
        agent_id: AgentId,
        event: ProviderEvent,
    },
    
    /// Agent status changed
    StatusChanged {
        agent_id: AgentId,
        old_status: AgentSlotStatus,
        new_status: AgentSlotStatus,
    },
    
    /// Agent completed its assigned task
    TaskCompleted {
        agent_id: AgentId,
        task_id: TaskId,
        result: TaskCompletionResult,
    },
    
    /// Agent encountered an error
    AgentError {
        agent_id: AgentId,
        error: String,
    },
    
    /// Agent thread finished/crashed
    ThreadFinished {
        agent_id: AgentId,
        outcome: ThreadOutcome,
    },
}

pub enum ThreadOutcome {
    Normal,
    Error(String),
    Cancelled,
}
```

### 3.3 Event Aggregation Strategy

The TUI event loop needs to poll multiple agent channels without blocking:

```rust
pub struct EventAggregator {
    /// All active receiver channels
    receivers: Vec<(AgentId, mpsc::Receiver<ProviderEvent>)>,
    
    /// Poll timeout per cycle
    poll_timeout: Duration,
}

impl EventAggregator {
    /// Poll all channels and collect available events
    pub fn poll_all(&self) -> Vec<AgentEvent> {
        let mut events = Vec::new();
        
        // Use try_recv on each channel (non-blocking)
        for (agent_id, rx) in &self.receivers {
            while let Ok(event) = rx.try_recv() {
                events.push(AgentEvent::FromAgent {
                    agent_id: agent_id.clone(),
                    event,
                });
            }
        }
        
        events
    }
    
    /// Poll with timeout for at least one event
    pub fn poll_with_timeout(&self, timeout: Duration) -> Vec<AgentEvent> {
        // Strategy: poll all channels, if none have events, sleep briefly
        // This prevents tight spin loops while allowing responsive updates
        let deadline = Instant::now() + timeout;
        
        loop {
            let events = self.poll_all();
            if !events.is_empty() {
                return events;
            }
            
            if Instant::now() >= deadline {
                return Vec::new();
            }
            
            // Brief sleep to reduce CPU usage
            std::thread::sleep(Duration::from_millis(20));
        }
    }
}
```

### 3.4 Thread Pool Management

#### 3.4.1 Provider Thread Lifecycle

Each provider runs in its own thread with clear lifecycle:

```rust
pub struct ProviderThreadHandle {
    /// Thread join handle
    handle: std::thread::JoinHandle<()>,
    
    /// Event sender (owned by provider thread)
    event_tx: mpsc::Sender<ProviderEvent>,
    
    /// Thread name for debugging
    thread_name: String,
    
    /// Start timestamp
    started_at: Instant,
}

/// Start provider in a managed thread
pub fn start_provider_thread(
    provider: ProviderKind,
    agent_id: AgentId,
    prompt: String,
    cwd: PathBuf,
    session_handle: Option<SessionHandle>,
) -> Result<ProviderThreadHandle> {
    let (event_tx, event_rx) = mpsc::channel();
    let thread_name = format!("agent-{}-{}", agent_id.as_str(), provider.label());
    
    let handle = thread::Builder::new()
        .name(thread_name.clone())
        .spawn(move || {
            // Provider execution loop
            if let Err(err) = provider::start_provider(
                provider,
                prompt,
                cwd,
                session_handle,
                event_tx.clone(),
            ) {
                let _ = event_tx.send(ProviderEvent::Error(err.to_string()));
            }
            let _ = event_tx.send(ProviderEvent::Finished);
        })?;
    
    Ok(ProviderThreadHandle {
        handle,
        event_tx, // Note: tx is cloned into thread, we keep original for potential cancellation
        thread_name,
        started_at: Instant::now(),
    })
}
```

#### 3.4.2 Thread Safety Considerations

| Resource | Access Pattern | Synchronization |
|----------|---------------|-----------------|
| AgentSlot fields | Read by TUI, Write by event loop | Single-threaded mutation, thread-safe reads via snapshots |
| SharedWorkplaceState | Read by all threads, Write by main loop | Mutex or single-threaded with message passing |
| Backlog state | Read/write by task assignment | Mutex with interior mutability |
| AgentStore persistence | Write from multiple threads | File-based locking or serialized writes |
| Transcript append | Write from provider thread | Send via channel, mutate in main loop |

**Key Principle**: Provider threads NEVER directly mutate shared state. All mutations happen in the main TUI event loop based on received events.

### 3.5 Persistence Strategy

#### 3.5.1 Concurrent Persistence Requirements

Multiple agents may need to persist simultaneously:

```rust
pub struct AgentPersistenceCoordinator {
    /// Workplace store reference
    workplace: WorkplaceStore,
    
    /// Pending persistence operations queue
    pending_ops: VecDeque<PersistenceOp>,
    
    /// Last flush timestamp
    last_flush: Instant,
    
    /// Minimum interval between flushes
    flush_interval: Duration,
}

pub enum PersistenceOp {
    SaveMeta { agent_id: AgentId, meta: AgentMeta },
    SaveTranscript { agent_id: AgentId, transcript: AgentTranscript },
    SaveState { agent_id: AgentId, state: AgentState },
    SaveMessages { agent_id: AgentId, messages: AgentMessages },
}

impl AgentPersistenceCoordinator {
    /// Queue a persistence operation
    pub fn queue(&mut self, op: PersistenceOp);
    
    /// Flush all pending operations (called periodically)
    pub fn flush(&mut self) -> Result<Vec<PathBuf>>;
    
    /// Force immediate save for critical state
    pub fn force_save(&mut self, agent_id: &AgentId) -> Result<()>;
}
```

#### 3.5.2 File-Based Isolation

Each agent has its own directory under `agents/`:

```
workplace/
├── meta.json
├── backlog.json
├── agents/
│   ├── agent_001/
│   │   ├── meta.json
│   │   ├── state.json
│   │   ├── transcript.json
│   │   ├── messages.json
│   │   └── memory.json
│   ├── agent_002/
│   │   ├── meta.json
│   │   └── ...
│   └── agent_003/
│       └── ...
```

This isolation allows concurrent file writes without conflict.

## 4. Implementation Phases

### Phase 4.1: Foundation (Estimated: 2-3 days)

**Goal**: Establish core data structures without changing TUI behavior.

#### Tasks

1. **T4.1.1**: Create `AgentSlot` and `AgentSlotStatus` structs
2. **T4.1.2**: Create `AgentPool` with basic lifecycle methods
3. **T4.1.3**: Create `SharedWorkplaceState` extracting shared fields from `AppState`
4. **T4.1.4**: Create `AgentEvent` enum and `EventAggregator`
5. **T4.1.5**: Write unit tests for AgentPool without provider threads

#### Acceptance

- AgentPool can spawn/stop mock agents
- AgentSlot status transitions work correctly
- EventAggregator polls multiple mock channels

### Phase 4.2: Provider Integration (Estimated: 3-4 days)

**Goal**: Run real providers through AgentPool.

#### Tasks

1. **T4.2.1**: Modify `provider::start_provider` to return thread handle + receiver
2. **T4.2.2**: Implement `ProviderThreadHandle` management in AgentSlot
3. **T4.2.3**: Wire Claude provider through AgentPool
4. **T4.2.4**: Wire Codex provider through AgentPool
5. **T4.2.5**: Implement graceful thread cancellation (drop tx, wait for join)
6. **T4.2.6**: Add provider thread lifecycle logging

#### Acceptance

- Multiple Claude/Codex agents can run simultaneously
- Provider threads emit events to correct agent channels
- Thread cancellation works cleanly

### Phase 4.3: Task Distribution (Estimated: 2-3 days)

**Goal**: Assign tasks to specific agents.

#### Tasks

1. **T4.3.1**: Add task assignment logic to AgentPool
2. **T4.3.2**: Implement task completion tracking per agent
3. **T4.3.3**: Add backlog mutation with mutex for concurrent access
4. **T4.3.4**: Implement task stealing/rebalancing (optional)
5. **T4.3.5**: Add task queue visualization helpers

#### Acceptance

- Tasks can be assigned to specific agents
- Completed tasks update backlog correctly
- Concurrent task completion works safely

### Phase 4.4: TUI Multi-Agent View (Estimated: 4-5 days)

**Goal**: Display all agent states in TUI.

#### Tasks

1. **T4.4.1**: Create agent status bar component (show all agent statuses)
2. **T4.4.2**: Implement agent focus switching (Tab/Ctrl+number)
3. **T4.4.3**: Create per-agent transcript view switching
4. **T4.4.4**: Add agent creation UI (spawn new agent)
5. **T4.4.5**: Add agent stop UI (cancel specific agent)
6. **T4.4.6**: Update footer to show multi-agent context

#### Acceptance

- TUI shows status of all running agents
- User can switch focus between agents
- User can spawn/stop agents from TUI

### Phase 4.5: Persistence & Recovery (Estimated: 2-3 days)

**Goal**: Persist multi-agent state and restore on restart.

#### Tasks

1. **T4.5.1**: Implement `AgentPersistenceCoordinator`
2. **T4.5.2**: Add periodic flush in event loop
3. **T4.5.3**: Restore all agents from workplace on bootstrap
4. **T4.5.4**: Handle recovery of interrupted agents
5. **T4.5.5**: Add workplace-level agent list persistence

#### Acceptance

- All agent states persist on quit
- All agents restore on restart
- Interrupted agents recover safely

## 5. Detailed Component Design

### 5.1 Modified RuntimeSession

```rust
/// New multi-agent runtime session
pub struct MultiAgentSession {
    /// Shared workplace state
    workplace: SharedWorkplaceState,
    
    /// Agent pool
    agents: AgentPool,
    
    /// Persistence coordinator
    persistence: AgentPersistenceCoordinator,
    
    /// Focused agent index (for TUI)
    focused_index: usize,
    
    /// Should quit flag
    should_quit: bool,
}

impl MultiAgentSession {
    pub fn bootstrap(cwd: PathBuf, default_provider: ProviderKind) -> Result<Self>;
    
    pub fn spawn_agent(&mut self, provider: ProviderKind) -> Result<AgentId>;
    
    pub fn stop_agent(&mut self, agent_id: &AgentId) -> Result<()>;
    
    pub fn submit_to_focused(&mut self, prompt: String) -> Result<()>;
    
    pub fn poll_events(&mut self, timeout: Duration) -> Vec<AgentEvent>;
    
    pub fn process_event(&mut self, event: AgentEvent) -> Result<Option<LoopAction>>;
    
    pub fn switch_focus(&mut self, index: usize) -> Result<()>;
    
    pub fn focused_agent(&self) -> Option<&AgentSlot>;
    
    pub fn persist_all(&mut self) -> Result<()>;
    
    pub fn shutdown(&mut self) -> Result<()>;
}
```

### 5.2 Modified TuiState

```rust
pub struct MultiAgentTuiState {
    /// Multi-agent session
    session: MultiAgentSession,
    
    /// Composer state
    composer: TextArea,
    composer_state: TextAreaState,
    
    /// Viewport state for focused agent transcript
    transcript_viewport_height: u16,
    transcript_scroll_offset: usize,
    transcript_max_scroll: usize,
    transcript_follow_tail: bool,
    
    /// Agent selection overlay state
    agent_browser_open: bool,
    agent_browser_selected: usize,
    
    /// Transcript overlay (unchanged)
    transcript_overlay: Option<TranscriptOverlayState>,
    
    /// Busy tracking for focused agent
    focused_busy_started_at: Option<Instant>,
}

impl MultiAgentTuiState {
    pub fn from_session(session: MultiAgentSession) -> Self;
    
    pub fn focused_agent(&self) -> Option<&AgentSlot>;
    
    pub fn focused_transcript(&self) -> &[TranscriptEntry];
    
    pub fn all_agent_statuses(&self) -> Vec<AgentStatusSnapshot>;
    
    pub fn spawn_agent(&mut self, provider: ProviderKind) -> Result<()>;
    
    pub fn stop_focused_agent(&mut self) -> Result<()>;
    
    pub fn switch_focus(&mut self, index: usize) -> Result<()>;
    
    pub fn open_agent_browser(&mut self);
    
    pub fn close_agent_browser(&mut self);
    
    pub fn submit_prompt(&mut self) -> Result<()>;
}
```

### 5.3 Modified App Loop

```rust
pub fn run_multi_agent(terminal: &mut AppTerminal, resume_last: bool) -> Result<()> {
    let launch_cwd = env::current_dir()?;
    let session = MultiAgentSession::bootstrap(launch_cwd, provider::default_provider())?;
    let mut state = MultiAgentTuiState::from_session(session);
    
    loop {
        // Render current state
        terminal.draw(|frame| render_multi_agent(frame, &mut state))?;
        
        if state.should_quit() {
            break;
        }
        
        // Poll agent events (non-blocking)
        let agent_events = state.session.poll_events(Duration::from_millis(80));
        
        // Process each agent event
        for event in agent_events {
            match state.session.process_event(event)? {
                Some(LoopAction::Continue { agent_id, prompt }) => {
                    // Agent requested continuation
                    state.session.continue_agent(&agent_id, prompt)?;
                }
                Some(LoopAction::TaskCompleted { .. }) => {
                    // Task finished, update UI
                }
                None => {}
            }
        }
        
        // Poll terminal input events
        if event::poll(Duration::from_millis(20))? {
            match event::read()? {
                Event::Key(key_event) => {
                    handle_multi_agent_key_event(&mut state, key_event)?;
                }
                Event::Paste(text) => {
                    handle_paste_event(&mut state, &text);
                }
                Event::Resize(_, _) => {}
                _ => {}
            }
        }
        
        // Periodic persistence
        state.session.persist_if_changed()?;
    }
    
    state.session.shutdown()?;
    Ok(())
}
```

### 5.4 Multi-Agent Rendering

```rust
fn render_multi_agent(frame: &mut Frame, state: &mut MultiAgentTuiState) {
    let areas = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // Agent status bar
            Constraint::Min(1),    // Transcript
            Constraint::Length(if state.focused_agent().map(|a| a.is_busy()).unwrap_or(false) { 1 } else { 0 }),
            Constraint::Length(state.composer.desired_height(frame.area().width, 8)),
            Constraint::Length(1), // Footer
        ])
        .split(frame.area());
    
    render_agent_status_bar(frame, state, areas[0]);
    render_focused_transcript(frame, state, areas[1]);
    if state.focused_agent().map(|a| a.is_busy()).unwrap_or(false) {
        render_working_line(frame, state, areas[2]);
    }
    render_composer(frame, state, areas[3]);
    render_footer(frame, state, areas[4]);
    
    if state.agent_browser_open {
        render_agent_browser_overlay(frame, state);
    }
    
    if state.transcript_overlay.is_some() {
        render_transcript_overlay(frame, state);
    }
}

fn render_agent_status_bar(frame: &mut Frame, state: &MultiAgentTuiState, area: Rect) {
    let statuses = state.all_agent_statuses();
    let mut spans = Vec::new();
    
    for (index, snap) in statuses.iter().enumerate() {
        let is_focused = index == state.focused_index;
        let style = if is_focused {
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
        } else if snap.status.is_busy() {
            Style::default().fg(Color::Green)
        } else if snap.status.is_error() {
            Style::default().fg(Color::Red)
        } else {
            Style::default().fg(Color::DarkGray)
        };
        
        let marker = if snap.status.is_busy() { "●" } else { "○" };
        let label = format!("{}{}[{}]", 
            if is_focused { ">" } else { " " },
            marker,
            snap.codename
        );
        
        spans.push(Span::styled(label, style));
        spans.push(Span::raw(" "));
    }
    
    let bar = Paragraph::new(Line::from(spans));
    frame.render_widget(bar, area);
}
```

## 6. Input Handling Changes

### 6.1 Key Bindings for Multi-Agent

| Key | Action |
|-----|--------|
| `Tab` | Cycle focus to next agent |
| `Shift+Tab` | Cycle focus to previous agent |
| `Ctrl+1` through `Ctrl+9` | Focus specific agent slot |
| `Ctrl+N` | Spawn new agent (prompts for provider) |
| `Ctrl+X` | Stop focused agent |
| `Ctrl+A` | Open agent browser overlay |
| `Enter` | Submit prompt to focused agent |
| `Esc` | Close overlays / request quit |
| `Ctrl+C` | Force quit (stops all agents) |

### 6.2 Agent Browser Overlay

```
┌─────────────────────────────────────────────────────┐
│ Agents (3)          ↑↓ select  n new  x stop  esc   │
├─────────────────────────────────────────────────────┤
│ > ● alpha [claude]  task-1: write tests             │
│   ○ bravo [codex]   idle                             │
│   ○ charlie [mock]  stopped: user cancelled         │
├─────────────────────────────────────────────────────┤
│ q or esc to close                                   │
└─────────────────────────────────────────────────────┘
```

## 7. Thread Safety Guarantees

### 7.1 Memory Safety Model

```
┌─────────────────────────────────────────────────────────┐
│                    Main Thread (TUI)                     │
│  - Owns MultiAgentTuiState                              │
│  - Owns AgentPool                                       │
│  - Owns all AgentSlots                                  │
│  - Mutates state on received events                     │
│  - Renders frame                                        │
└─────────────────────────────────────────────────────────┘
                          │
          ┌───────────────┼───────────────┐
          │               │               │
          ▼               ▼               ▼
┌─────────────┐  ┌─────────────┐  ┌─────────────┐
│ Provider    │  │ Provider    │  │ Provider    │
│ Thread 1    │  │ Thread 2    │  │ Thread 3    │
│             │  │             │  │             │
│ - Owns      │  │ - Owns      │  │ - Owns      │
│   event_tx  │  │   event_tx  │  │   event_tx  │
│ - Reads     │  │ - Reads     │  │ - Reads     │
│   cwd only  │  │   cwd only  │  │   cwd only  │
│             │  │             │  │             │
│ - Sends     │  │ - Sends     │  │ - Sends     │
│   events    │  │   events    │  │   events    │
│   ONLY      │  │   ONLY      │  │   ONLY      │
└─────────────┘  └─────────────┘  └─────────────┘
```

### 7.2 Rules

1. **Provider threads NEVER directly mutate shared state**
2. **All state mutations happen in main thread after receiving events**
3. **Channel communication is the ONLY cross-thread data transfer**
4. **File persistence uses per-agent directories (no file conflicts)**
5. **Backlog uses Mutex for interior mutability if needed**

## 8. Error Handling

### 8.1 Agent Thread Errors

```rust
pub enum AgentErrorKind {
    /// Provider process crashed
    ProviderCrash { exit_code: i32, stderr: String },
    
    /// Provider start failed
    ProviderStartFailed { reason: String },
    
    /// Thread panicked
    ThreadPanic { backtrace: String },
    
    /// Timeout waiting for response
    Timeout { duration: Duration },
    
    /// User cancelled
    UserCancelled,
    
    /// Persistence failed
    PersistenceFailed { path: PathBuf, reason: String },
}

/// Error handling flow
fn handle_agent_error(pool: &mut AgentPool, agent_id: &AgentId, error: AgentErrorKind) {
    // 1. Mark slot as error status
    pool.mark_error(agent_id, error);
    
    // 2. Clean up thread handle
    pool.cleanup_thread(agent_id);
    
    // 3. Save error state for recovery
    pool.save_error_snapshot(agent_id);
    
    // 4. If task was assigned, mark as blocked
    if let Some(task_id) = pool.get_assigned_task(agent_id) {
        pool.block_task(&task_id, format!("agent error: {:?}", error));
    }
    
    // 5. Optionally auto-restart agent
    if pool.auto_restart_enabled() {
        pool.restart_agent(agent_id);
    }
}
```

### 8.2 Graceful Shutdown

```rust
impl MultiAgentSession {
    pub fn shutdown(&mut self) -> Result<()> {
        // 1. Mark all agents as stopping
        for slot in &mut self.agents.slots {
            slot.status = AgentSlotStatus::Stopping;
        }
        
        // 2. Drop all event_tx to signal threads
        for slot in &mut self.agents.slots {
            slot.event_tx = None;
        }
        
        // 3. Wait for threads with timeout
        let deadline = Instant::now() + Duration::from_secs(5);
        for slot in &mut self.agents.slots {
            if let Some(handle) = slot.thread_handle.take() {
                let remaining = deadline.saturating_duration_since(Instant::now());
                match handle.join_timeout(remaining) {
                    Ok(()) => {}
                    Err(_) => {
                        // Thread didn't finish in time, log warning
                        logging::warn_event(...);
                    }
                }
            }
        }
        
        // 4. Persist final state
        self.persist_all()?;
        
        // 5. Mark all as stopped
        for slot in &mut self.agents.slots {
            slot.status = AgentSlotStatus::Stopped { reason: "shutdown".to_string() };
        }
        
        Ok(())
    }
}
```

## 9. Testing Strategy

### 9.1 Unit Tests

| Test Area | Focus |
|-----------|-------|
| AgentPool lifecycle | Spawn, stop, status transitions |
| EventAggregator | Multi-channel polling, ordering |
| PersistenceCoordinator | Queue, flush, file isolation |
| Task assignment | Assignment, completion, blocking |
| Error handling | Crash recovery, graceful shutdown |

### 9.2 Integration Tests

| Test Area | Focus |
|-----------|-------|
| Multi-provider execution | Claude + Codex simultaneously |
| Concurrent persistence | Multiple agents persisting |
| Task parallelism | Multiple tasks completing |
| TUI event flow | Events reach correct agent |

### 9.3 Stress Tests

| Test Area | Focus |
|-----------|-------|
| Max agents | Spawn 10+ agents |
| Long-running | Agents running for hours |
| Error recovery | Recover from various failures |
| Shutdown | Clean shutdown under load |

## 10. Migration Path

### 10.1 Backward Compatibility

Multi-agent should be opt-in initially:

1. **Default mode**: Single agent (current behavior)
2. **Flag**: `--multi-agent` or config option to enable
3. **UI**: Single-agent TUI unchanged until user spawns second agent

### 10.2 Data Migration

Existing single-agent workplaces work unchanged:

- `agent_001` becomes the first slot
- Single `meta.json` at root converted to agent-specific
- Backlog unchanged (shared)

## 11. Open Questions

1. **Opencode provider**: Should we add now or defer to separate sprint?
2. **Task stealing**: Should idle agents steal tasks from busy ones?
3. **Agent limits**: What's the reasonable max concurrent agents?
4. **Cross-agent communication**: Should agents be able to call each other?
5. **Resource pooling**: Should we pool MCP connections across agents?

## 12. References

- `agent_runtime.rs`: Current single-agent runtime
- `runtime_session.rs`: Current session model
- `provider.rs`: Provider abstraction
- `app_loop.rs`: Current TUI event loop
- `loop_runner.rs`: Autonomous loop logic
- `workplace_store.rs`: Workplace persistence

## 13. Appendix: Code Migration Checklist

### Files to Modify

| File | Changes |
|------|---------|
| `core/src/agent_runtime.rs` | Add AgentSlot, AgentPool |
| `core/src/runtime_session.rs` | Replace with MultiAgentSession |
| `core/src/app.rs` | Split into SharedWorkplaceState + per-agent |
| `core/src/provider.rs` | Return thread handle |
| `core/src/loop_runner.rs` | Adapt for multi-agent |
| `tui/src/app_loop.rs` | Multi-agent event loop |
| `tui/src/ui_state.rs` | MultiAgentTuiState |
| `tui/src/render.rs` | Multi-agent rendering |
| `tui/src/input.rs` | Multi-agent key bindings |

### New Files to Create

| File | Purpose |
|------|---------|
| `core/src/agent_pool.rs` | AgentPool, AgentSlot |
| `core/src/agent_event.rs` | AgentEvent, EventAggregator |
| `core/src/persistence_coordinator.rs` | Concurrent persistence |
| `core/src/shared_state.rs` | SharedWorkplaceState |
| `tui/src/agent_browser.rs` | Agent selection overlay |
| `tui/src/status_bar.rs` | Agent status bar rendering |