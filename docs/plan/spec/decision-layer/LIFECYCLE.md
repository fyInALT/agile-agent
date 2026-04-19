# Decision Agent Lifecycle

## Overview

This document describes the complete lifecycle of a Decision Agent, from creation to destruction.

## Agent Lifecycle States

```
┌───────────────────────────────────────────────────────────────────┐
│                    Decision Agent Lifecycle                        │
└───────────────────────────────────────────────────────────────────┘

     ┌─────────────┐
     │   Created   │  (Lazy/Eager policy)
     └──────┬──────┘
            │
            ▼
     ┌─────────────┐
     │    Idle     │  (waiting for task)
     └──────┬──────┘
            │ switch_task()
            ▼
     ┌─────────────┐
     │   Running   │  (active decision making)
     └──────┬──────┘
            │
            ├──────────────────────┐
            │                      │
            ▼                      ▼
     ┌─────────────┐        ┌─────────────┐
     │   Blocked   │        │  Reflecting │  (reflection cycle)
     └──────┬──────┘        └──────┬──────┘
            │                      │
            │ human_response       │ increment_reflection()
            │                      │
            ▼                      ▼
     ┌─────────────┐        ┌─────────────┐
     │   Running   │◄───────│   Running   │
     └─────────────┘        └─────────────┘
            │
            │ story_complete / idle_timeout / manual_stop / fatal_error
            ▼
     ┌─────────────┐
     │  Destroyed  │  (cleanup and archive)
     └─────────────┘
```

## Creation Policy

### Lazy Creation (Default)
- Agent created when first blocked event occurs
- Saves resources when no decisions needed
- Recommended for most scenarios

```rust
DecisionAgentCreationPolicy::Lazy  // default
```

### Eager Creation
- Agent created immediately when Main Agent spawns
- Faster first decision response
- Useful for time-sensitive scenarios

```rust
DecisionAgentCreationPolicy::Eager
```

## Destruction Triggers

| Trigger | Description | Action |
|---------|-------------|--------|
| `StoryComplete` | Story successfully completed | Archive transcripts, clear state |
| `MainAgentStopped` | Parent Main Agent stopped | Cleanup, no archive |
| `IdleTimeout` | No activity for 30 minutes | Archive, mark idle |
| `ManualStop` | Human manual stop command | Cleanup immediately |
| `FatalError` | Error requiring full reset | Reset state, log error |

## State Management

### DecisionAgentState

```rust
pub struct DecisionAgentState {
    /// Decision agent ID
    pub agent_id: AgentId,
    
    /// Parent main agent ID
    pub parent_agent_id: AgentId,
    
    /// Current task ID
    pub current_task_id: Option<TaskId>,
    
    /// Current story ID  
    pub current_story_id: Option<StoryId>,
    
    /// Task contexts (active)
    pub task_contexts: HashMap<TaskId, TaskDecisionContext>,
    
    /// Configuration
    pub config: DecisionAgentConfig,
    
    /// Creation timestamp
    pub created_at: DateTime<Utc>,
    
    /// Last activity timestamp
    pub last_activity: DateTime<Utc>,
    
    /// Reflection rounds total
    pub reflection_rounds: u8,
    
    /// Retry count total
    pub retry_count: u8,
}
```

### Task Decision Context

```rust
pub struct TaskDecisionContext {
    /// Task ID
    pub task_id: TaskId,
    
    /// Decisions made for this task
    pub decisions: Vec<DecisionRecord>,
    
    /// Reflection rounds for this task
    pub reflection_rounds: u8,
    
    /// Retry count for this task
    pub retry_count: u8,
    
    /// Timeout count for this task
    pub timeout_count: u8,
    
    /// Task start time
    pub started_at: DateTime<Utc>,
    
    /// Task completion time (if completed)
    pub completed_at: Option<DateTime<Utc>,
}
```

## Key Operations

### Switch Task

```rust
pub fn switch_task(&mut self, new_task: TaskId) {
    // Archive current task
    if let Some(current_id) = &self.current_task_id {
        if let Some(ctx) = self.task_contexts.get_mut(current_id) {
            ctx.mark_complete();
        }
    }
    
    // Create or restore new task context
    if !self.task_contexts.contains_key(&new_task) {
        self.task_contexts.insert(new_task.clone(), TaskDecisionContext::new(new_task.clone()));
    }
    
    self.current_task_id = Some(new_task);
    self.last_activity = Utc::now();
}
```

### Switch Story

```rust
pub fn switch_story(&mut self, new_story: StoryId) {
    self.current_story_id = Some(new_story);
    self.last_activity = Utc::now();
    
    // Reset reflection rounds for new story
    self.reflection_rounds = 0;
}
```

### Record Decision

```rust
pub fn record_decision(&mut self, record: DecisionRecord) {
    if let Some(task_id) = &self.current_task_id {
        if let Some(ctx) = self.task_contexts.get_mut(task_id) {
            ctx.add_decision(record);
        }
    }
    self.last_activity = Utc::now();
}
```

### Increment Reflection

```rust
pub fn increment_reflection(&mut self) {
    self.reflection_rounds += 1;
    if let Some(task_id) = &self.current_task_id {
        if let Some(ctx) = self.task_contexts.get_mut(task_id) {
            ctx.increment_reflection();
        }
    }
    self.last_activity = Utc::now();
}
```

### Can Reflect

```rust
pub fn can_reflect(&self) -> bool {
    self.reflection_rounds < self.config.max_reflection_rounds
}
```

### Idle Expiration Check

```rust
pub fn is_idle_expired(&self) -> bool {
    let elapsed = (Utc::now() - self.last_activity).num_milliseconds();
    elapsed > self.config.idle_timeout_ms as i64
}
```

## Persistence

### Save State

```rust
pub fn persist(&self, path: &Path) -> crate::error::Result<()> {
    let json = serde_json::to_string(self)?;
    std::fs::write(path, json)?;
    Ok(())
}
```

### Restore State

```rust
pub fn restore(path: &Path) -> crate::error::Result<Self> {
    let json = std::fs::read_to_string(path)?;
    let state: Self = serde_json::from_str(&json)?;
    Ok(state)
}
```

## Task Status Flow

```
┌───────────────────────────────────────────────────────────────────┐
│                       Task Status Flow                             │
└───────────────────────────────────────────────────────────────────┘

     ┌─────────────┐
     │   Pending   │  (task created, waiting to start)
     └──────┬──────┘
            │ transition_to(InProgress)
            ▼
     ┌─────────────┐
     │  InProgress │  (task being executed)
     └──────┬──────┘
            │
            ├──────────────────────┐
            │                      │
            ▼                      ▼
     ┌─────────────┐        ┌─────────────────┐
     │  Reflecting │        │ PendingConfirmation │
     └──────┬──────┘        └──────────┬──────────┘
            │                          │
            │ reflection complete      │ confirmed
            ▼                          ▼
     ┌─────────────┐              ┌─────────────┐
     │  InProgress │◄─────────────│   Completed │
     └─────────────┘              └─────────────┘
            │                          
            │ needs human              
            ▼                          
     ┌───────────────────┐          
     │ NeedsHumanDecision │          
     └──────────┬────────┘          
            │                      
            │ human approves       
            ▼                      
     ┌─────────────┐              
     │  InProgress │◄──────────────
     └─────────────┘              
            │
            │ timeout / error
            ▼
     ┌─────────────┐
     │    Paused   │  (recovery state)
     └──────┬──────┘
            │
            │ resume / cancel
            ├──────────────────────┐
            │                      │
            ▼                      ▼
     ┌─────────────┐        ┌─────────────┐
     │  InProgress │        │  Cancelled  │
     └─────────────┘        └─────────────┘
```

## Valid Status Transitions

| From | To | Condition |
|------|-----|-----------|
| Pending | InProgress | Task starts |
| InProgress | Reflecting | Issue found |
| InProgress | PendingConfirmation | Goal achieved |
| InProgress | Paused | Timeout/error |
| InProgress | Cancelled | User cancels |
| Reflecting | InProgress | Issue fixed |
| Reflecting | NeedsHumanDecision | Max reflections |
| Reflecting | Paused | Recovery needed |
| PendingConfirmation | Completed | Human confirms |
| PendingConfirmation | Reflecting | Human rejects |
| PendingConfirmation | Paused | Timeout |
| NeedsHumanDecision | InProgress | Human approves |
| NeedsHumanDecision | Cancelled | Human cancels |
| Paused | InProgress | Recovery complete |
| Paused | Cancelled | Manual cancel |
| Any* | Cancelled | Except Completed |

## Configuration

```rust
pub struct DecisionAgentConfig {
    /// Creation policy
    pub creation_policy: DecisionAgentCreationPolicy,
    
    /// Idle timeout in milliseconds (default: 30 minutes)
    pub idle_timeout_ms: u64,
    
    /// Keep transcript after destruction
    pub keep_transcript: bool,
    
    /// Maximum reflection rounds
    pub max_reflection_rounds: u8,
    
    /// Maximum retry count
    pub max_retry_count: u8,
    
    /// Context cache max bytes
    pub context_cache_max_bytes: usize,
}

impl Default for DecisionAgentConfig {
    fn default() -> Self {
        Self {
            creation_policy: DecisionAgentCreationPolicy::Lazy,
            idle_timeout_ms: 1800000, // 30 minutes
            keep_transcript: true,
            max_reflection_rounds: 3,
            max_retry_count: 3,
            context_cache_max_bytes: 10240, // 10KB
        }
    }
}
```

## Integration with Pipeline

The lifecycle integrates with the decision pipeline:

```
DecisionPipeline.execute()
    │
    ├─► Pre-processor: sync reflection_round from state
    │
    ├─► Strategy: select maker based on tier
    │
    ├─► Maker: execute decision
    │   │
    │   ├─► RuleBased (Simple tier)
    │   ├─► LLM (Medium/Complex tier)
    │   ├─► CLI (Critical tier - human)
    │
    ├─► Post-processor: validate output
    │
    ├─► Record decision in state
    │
    └─► Return output
        │
        ├─► Continue → keep Running
        ├─► Reflect → transition to Reflecting
        ├─► RequestHuman → transition to Blocked
        ├─► ConfirmCompletion → transition to PendingConfirmation
        └─► Cancel → transition to Cancelled
```
