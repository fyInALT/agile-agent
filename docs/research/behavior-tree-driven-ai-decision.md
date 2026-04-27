# Behavior Tree Driven AI Decision Flow - Design Document

## Executive Summary

This document defines how behavior trees in `decision-dsl` become the **process skeleton** that guides AI (Claude/Codex) through structured decision workflows, outputting commands to Work Agent. Behavior trees define *when* and *what* to ask; AI determines *how* to respond and *what command* to send.

---

## 1. Problem Statement

### 1.1 Current Workflow Gap

**Scenario**: Work Agent completes 2 of 4 sprints, stops early.

Current decision layer approach:
- Human reviews output, decides manually
- Or: Rule-based conditions miss nuance ("is it really complete?")
- No structured reflection flow
- No retry with alternative strategy

**What we need**:
1. AI judges completion quality (not human)
2. Structured reflection flow (multiple angles)
3. Retry with learning (adjust strategy on failure)
4. All 4 sprints verified before completion

### 1.2 Why Behavior Trees

Behavior trees provide:
- **Structured flow control**: Sequence, Selector, Repeater
- **Decision points**: PromptNode as AI invocation
- **State management**: Blackboard stores AI outputs
- **Routing logic**: Selector routes based on AI JSON output
- **Retry loops**: Repeater with max_attempts
- **Human escalation**: ForceHuman for unrecoverable cases

This is exactly what we need for decision workflows.

---

## 2. Architecture Overview

### 2.1 Layer Integration

```
┌───────────────────────────────────────────────────────────────────────┐
│                      Decision System Architecture                       │
├───────────────────────────────────────────────────────────────────────┤
│                                                                        │
│  Layer 1: Work Agent (Claude/Codex CLI)                               │
│  ├─ Executes tasks, produces output                                   │
│  ├─ provider_output → stored in Blackboard                            │
│  └──────────────────────────────────────────────────────────────────┤│
│                                                                        │
│  Layer 2: Decision Agent (decision-dsl)                               │
│  ├─ Behavior Tree defines decision flow                               │
│  ├─ PromptNode invokes AI for judgment                                │
│  ├─ Blackboard stores AI outputs                                      │
│  ├─ ActionNode outputs DecisionCommand                                │
│  └──────────────────────────────────────────────────────────────────┤│
│                                                                        │
│  Layer 3: AI Session (Claude API / Codex)                             │
│  ├─ Receives prompts from PromptNode                                  │
│  ├─ Returns structured JSON decisions                                 │
│  ├─ Provides reasoning for trace                                      │
│  └──────────────────────────────────────────────────────────────────┤│
│                                                                        │
│  Layer 4: Work Agent Controller                                       │
│  ├─ Receives DecisionCommand                                          │
│  ├─ Executes: SendInstruction, WakeUp, Reflect, etc.                  │
│  ├─ Returns to Layer 1                                                │
│                                                                        │
└───────────────────────────────────────────────────────────────────────┘
```

### 2.2 Data Flow

```
Work Agent Output
       │
       ▼
┌──────────────┐
│  Blackboard  │  provider_output, file_changes, task_description
└──────────────┘
       │
       ▼
┌──────────────┐
│ Behavior Tree│  Executor.tick(tree, ctx)
│   Executor   │
└──────────────┘
       │
       ├──── PromptNode ──► AI Session ──► JSON Response
       │                           │
       │                           ▼
       │                    ┌──────────────┐
       │                    │  Blackboard  │  store AI decision
       │                    └──────────────┘
       │                           │
       │                           ▼
       │                    Selector routes
       │                           │
       │                           ▼
       │                    ActionNode
       │                           │
       │                           ▼
       │                    ┌──────────────────┐
       │                    │ DecisionCommand  │  SendInstruction, etc.
       │                    └──────────────────┘
       │                           │
       └───────────────────────────┘
                                   │
                                   ▼
                           Work Agent Controller
                                   │
                                   ▼
                           Work Agent executes
```

---

## 3. Current Implementation Analysis

### 3.1 Existing Components

#### Blackboard (`decision-dsl/src/ext/blackboard.rs`)

```rust
pub struct Blackboard {
    // Work Agent context
    pub provider_output: String,
    pub file_changes: Vec<FileChangeRecord>,
    pub last_tool_call: Option<ToolCallRecord>,
    
    // Decision context
    pub task_description: String,
    pub reflection_round: u8,
    pub confidence_accumulator: f64,
    
    // AI outputs (stored by PromptNode.sets)
    scopes: Vec<HashMap<String, BlackboardValue>>,
    
    // Commands for Work Agent
    pub commands: Vec<DecisionCommand>,
    
    // AI conversation history
    pub llm_responses: HashMap<String, String>,
}
```

**What's available**:
- Work Agent output storage
- AI response history
- Command queue for Work Agent
- Scope-based variable storage

**What's needed**:
- Sprint tracking (current_sprint, total_sprints)
- Reflection chain storage
- Outcome history for learning

#### PromptNode (`decision-dsl/src/ast/node.rs`)

```rust
pub struct PromptNode {
    pub name: String,
    pub template: String,
    pub parser: OutputParser,  // Json parser for structured output
    pub sets: Vec<SetMapping>,  // Store AI output in blackboard
    pub timeout_ms: u64,
}
```

**Execution flow** (`runtime.rs:tick_prompt`):
1. First tick: render template, send to session, return Running
2. Second tick: check timeout, receive response, parse JSON
3. Store parsed values via `sets` mapping
4. If `__command` in output, push to command queue
5. Return Success/Failure

**This is the AI decision point**.

#### Session Trait (`decision-dsl/src/ext/traits.rs`)

```rust
pub trait Session {
    fn send(&mut self, message: &str) -> Result<(), SessionError>;
    fn send_with_hint(&mut self, message: &str, model: &str) -> Result<(), SessionError>;
    fn is_ready(&self) -> bool;
    fn receive(&mut self) -> Result<String, SessionError>;
}
```

**Implementation needed**: `AiDecisionSession` connecting to Claude/Codex.

#### DecisionCommand (`decision-dsl/src/ext/command.rs`)

```rust
pub enum DecisionCommand {
    Agent(AgentCommand),  // SendInstruction, WakeUp, Reflect, Terminate
    Git(GitCommand, Option<String>),  // Commit, Stash, etc.
    Task(TaskCommand),  // ConfirmCompletion, StopIfComplete
    Human(HumanCommand),  // Escalate, SelectOption
    Provider(ProviderCommand),  // RetryTool, SwitchProvider
}
```

**Key commands for Work Agent**:
- `SendInstruction { prompt, target_agent }` - Tell Work Agent what to do
- `WakeUp` - Restart stalled agent
- `Reflect { prompt }` - Ask agent to self-review
- `Terminate { reason }` - Stop agent

---

## 4. Design: Behavior Tree as Decision Flow Skeleton

### 4.1 Core Concept

**Behavior Tree = Structured Decision Workflow**

Each traversal through the tree represents one decision cycle:
1. Sequence → ordered steps (plan → execute → reflect → verify)
2. Selector → branching based on AI output
3. Repeater → retry loops with max attempts
4. PromptNode → AI judgment at each decision point

### 4.2 Sprint Completion Flow Example

**Requirement**: Complete 4 sprints, verify each, retry if incomplete.

```yaml
apiVersion: decision.agile-agent.io/v1
kind: BehaviorTree
metadata:
  name: sprint-completion-flow
spec:
  root:
    kind: Sequence
    payload:
      name: complete_all_sprints
      children:
        # Step 1: Initialize sprint counter
        - kind: SetVar
          payload:
            name: init_counter
            key: current_sprint
            value: 1
            
        # Step 2: Main sprint loop
        - kind: Repeater
          payload:
            name: sprint_loop
            maxAttempts: 4  # 4 sprints
            child:
              kind: Sequence
              payload:
                name: sprint_cycle
                children:
                  # 2.1: Execute sprint
                  - kind: Prompt
                    payload:
                      name: execute_sprint
                      template: |
                        Complete sprint {{ current_sprint }} of 4.
                        Current task: {{ task_description }}
                        Previous work: {{ file_changes }}
                        
                        When done, report status.
                      parser:
                        kind: Json
                        payload:
                          schema:
                            type: object
                            properties:
                              status: { type: string }
                              completed_items: { type: array }
                              output: { type: string }
                            required: [status]
                      sets:
                        - key: sprint_status
                          field: status
                        - key: sprint_output
                          field: output
                      timeoutMs: 60000
                      
                  # 2.2: Reflect on completion
                  - kind: Prompt
                    payload:
                      name: reflect_completion
                      template: |
                        Sprint {{ current_sprint }} reported: {{ sprint_status }}
                        Output: {{ sprint_output }}
                        
                        Analyze:
                        1. Was this sprint genuinely completed?
                        2. Are there incomplete items?
                        3. Should we proceed or retry?
                        
                        Output format:
                        {
                          "completed": boolean,
                          "incomplete": ["item1", ...],
                          "action": "proceed|retry|escalate",
                          "reasoning": "brief explanation"
                        }
                      parser:
                        kind: Json
                      sets:
                        - key: reflection_result
                          field: action
                        - key: reflection_reasoning
                          field: reasoning
                          
                  # 2.3: Route based on reflection
                  - kind: Selector
                    payload:
                      name: route_after_reflection
                      children:
                        # Proceed if completed
                        - kind: Condition
                          payload:
                            name: check_completed
                            evaluator:
                              kind: VariableIs
                              payload:
                                key: reflection_result
                                expected: "proceed"
                                
                        # Retry if incomplete
                        - kind: Sequence
                          payload:
                            name: retry_cycle
                            children:
                              - kind: Action
                                payload:
                                  name: send_retry_instruction
                                  command:
                                    kind: Agent
                                    payload:
                                      kind: SendInstruction
                                      payload:
                                        prompt: |
                                          Previous sprint incomplete.
                                          Missing: {{ sprint_status.incomplete }}
                                          Reason: {{ reflection_reasoning }}
                                          
                                          Retry with adjusted approach.
                                        target_agent: "{{ agent_id }}"
                                        
                        # Escalate if stuck
                        - kind: ForceHuman
                          payload:
                            name: escalate_stuck
                            reason: "Sprint {{ current_sprint }} stuck after reflection"
                            
                  # 2.4: Increment sprint counter if proceeding
                  - kind: SetVar
                    payload:
                      name: increment_sprint
                      key: current_sprint
                      value: "{{ current_sprint + 1 }}"
                      
        # Step 3: Alternative reflection angle
        - kind: Prompt
          payload:
            name: alternative_reflection
            template: |
              All 4 sprints reported complete.
              
              Verify from different perspective:
              - Are all requirements addressed?
              - Is there any technical debt left?
              - Any hidden blockers?
              
              Output:
              {
                "verified": boolean,
                "issues": ["issue1", ...],
                "confidence": 0.0-1.0
              }
            parser:
              kind: Json
            sets:
              - key: verification_result
                field: verified
                
        # Step 4: Final action
        - kind: Selector
          payload:
            name: finalize
            children:
              - kind: Sequence
                payload:
                  name: complete_flow
                  children:
                    - kind: Condition
                      payload:
                        evaluator:
                          kind: VariableIs
                          payload:
                            key: verification_result
                            expected: true
                    - kind: Action
                      payload:
                        name: confirm_completion
                        command:
                          kind: Task
                          payload:
                            kind: ConfirmCompletion
                            
              - kind: Action
                payload:
                  name: request_review
                  command:
                    kind: Human
                    payload:
                      kind: EscalateToHuman
                      payload:
                        reason: "Verification found issues: {{ alternative_reflection.issues }}"
```

### 4.3 Key Patterns

#### Pattern 1: PromptNode as Decision Point

Each PromptNode is where AI makes a judgment:

```yaml
- kind: Prompt
  payload:
    template: |
      Context: {{ provider_output }}
      
      Decision required: [describe what to decide]
      
      Output format:
      {
        "decision": "option1|option2|option3",
        "reasoning": "...",
        "confidence": 0.0-1.0
      }
    parser:
      kind: Json
    sets:
      - key: decision_output
        field: decision
```

The structured JSON enables programmatic routing.

#### Pattern 2: Selector Routes Based on AI Output

```yaml
- kind: Selector
  children:
    - Condition: decision_output == "proceed"
    - Condition: decision_output == "retry"
      then: Action(RetryInstruction)
    - ForceHuman: (fallback for unexpected output)
```

#### Pattern 3: Repeater for Retry Loop

```yaml
- kind: Repeater
  payload:
    maxAttempts: 3
    child:
      kind: Sequence
      children:
        - Prompt: "Try again with feedback"
        - Condition: "success == true"
```

#### Pattern 4: Multi-Angle Reflection

```yaml
children:
  - Prompt:  # Primary reflection
      template: "Verify from implementation perspective"
  - Prompt:  # Test perspective
      template: "Verify from testing perspective"  
  - Prompt:  # Edge case perspective
      template: "Check for edge cases"
```

#### Pattern 5: ActionNode Outputs to Work Agent

```yaml
- kind: Action
  payload:
    command:
      kind: Agent
      payload:
        kind: SendInstruction
        payload:
          prompt: "{{ ai_decision.next_instruction }}"
          target_agent: "{{ work_agent_id }}"
```

---

## 5. Integration Design

### 5.1 Decision Session Implementation

Implement `Session` trait for AI connection:

```rust
// In agent-decision crate or agent-daemon

use decision_dsl::ext::traits::{Session, SessionError, SessionErrorKind};

pub struct ClaudeDecisionSession {
    /// Claude API client
    client: ClaudeClient,
    
    /// Conversation history for context
    history: Vec<Message>,
    
    /// Pending response
    pending_response: Option<String>,
    
    /// Max history tokens
    max_history_tokens: usize,
}

impl Session for ClaudeDecisionSession {
    fn send(&mut self, message: &str) -> Result<(), SessionError> {
        // Add to history
        self.history.push(Message::user(message));
        
        // Send to Claude
        let response = self.client.send_with_history(&self.history)?;
        
        // Store pending response
        self.pending_response = Some(response);
        
        Ok(())
    }
    
    fn send_with_hint(&mut self, message: &str, model: &str) -> Result<(), SessionError> {
        // Use specific model (sonnet for speed, opus for complexity)
        self.client.set_model(model);
        self.send(message)
    }
    
    fn is_ready(&self) -> bool {
        self.pending_response.is_some()
    }
    
    fn receive(&mut self) -> Result<String, SessionError> {
        self.pending_response.take()
            .ok_or(SessionError {
                kind: SessionErrorKind::UnexpectedFormat,
                message: "no response available".into(),
            })
    }
}
```

### 5.2 Decision Agent Slot Integration

In `agent-daemon`, the Decision Agent slot runs behavior tree:

```rust
// In agent-daemon/src/decision_agent_slot.rs

pub struct DecisionAgentSlot {
    /// Behavior tree to execute
    tree: Tree,
    
    /// Tree executor
    executor: Executor,
    
    /// Decision session (Claude connection)
    session: ClaudeDecisionSession,
    
    /// Blackboard (shared with Work Agent output)
    blackboard: Blackboard,
    
    /// Clock for timeouts
    clock: SystemClock,
}

impl DecisionAgentSlot {
    /// Execute one decision cycle
    pub fn tick(&mut self) -> Result<TickResult, DecisionError> {
        let logger = NullLogger;
        
        // Sync Work Agent output to blackboard
        self.sync_work_agent_output();
        
        // Execute behavior tree
        let mut ctx = TickContext {
            blackboard: &mut self.blackboard,
            session: &mut self.session,
            clock: &self.clock,
            logger: &logger,
        };
        
        let result = self.executor.tick(&mut self.tree, &mut ctx)?;
        
        // Drain commands and send to Work Agent
        let commands = self.blackboard.drain_commands();
        for cmd in commands {
            self.send_to_work_agent(cmd)?;
        }
        
        Ok(result)
    }
    
    /// Sync Work Agent's latest output
    fn sync_work_agent_output(&mut self) {
        if let Some(output) = self.get_work_agent_output() {
            self.blackboard.provider_output = output;
        }
        if let Some(changes) = self.get_work_agent_file_changes() {
            self.blackboard.file_changes = changes;
        }
    }
    
    /// Send command to Work Agent controller
    fn send_to_work_agent(&self, cmd: DecisionCommand) -> Result<(), DecisionError> {
        match cmd {
            DecisionCommand::Agent(AgentCommand::SendInstruction { prompt, target_agent }) => {
                self.work_agent_controller.send_instruction(prompt, target_agent);
            }
            DecisionCommand::Agent(AgentCommand::WakeUp) => {
                self.work_agent_controller.wake_up();
            }
            DecisionCommand::Agent(AgentCommand::Reflect { prompt }) => {
                self.work_agent_controller.request_reflection(prompt);
            }
            DecisionCommand::Task(TaskCommand::ConfirmCompletion) => {
                self.work_agent_controller.confirm_completion();
            }
            DecisionCommand::Human(HumanCommand::Escalate { reason, context }) => {
                self.human_interface.escalate(reason, context);
            }
            _ => {}
        }
        Ok(())
    }
}
```

### 5.3 Event Loop Integration

The daemon's EventLoop triggers decision ticks:

```rust
// In agent-daemon/src/event_loop.rs

fn handle_provider_event(&mut self, event: ProviderEvent) {
    // Work Agent produced output
    if let ProviderEvent::Output { content, .. } = event {
        // Store in shared state
        self.work_agent_output = content;
        
        // Trigger decision agent
        if let Some(decision_slot) = &mut self.decision_agent_slot {
            decision_slot.blackboard.provider_output = content.clone();
            
            // Tick behavior tree
            let result = decision_slot.tick();
            
            // Process result
            match result.status {
                NodeStatus::Success => {
                    // Decision cycle complete
                    self.record_decision(result);
                }
                NodeStatus::Running => {
                    // Waiting for AI response, poll later
                    self.schedule_decision_poll();
                }
                NodeStatus::Failure => {
                    // Decision failed, handle escalation
                    self.handle_decision_failure(result);
                }
            }
        }
    }
}
```

---

## 6. Prompt Template Design

### 6.1 Decision Prompt Template

```yaml
template: |
  ## Context
  
  Task: {{ task_description }}
  Sprint: {{ current_sprint }} / {{ total_sprints }}
  
  ## Work Agent Output
  
  {{ provider_output }}
  
  ## Recent Changes
  
  {% for change in file_changes %}
  - {{ change.path }}: {{ change.change_type }}
  {% endfor %}
  
  ## Previous Reflections
  
  {% if reflection_history %}
  Earlier analysis: {{ reflection_history | last }}
  {% endif %}
  
  ## Decision Required
  
  [Specific question to answer]
  
  ## Output Format
  
  {
    "decision": "<option>",
    "reasoning": "<brief explanation>",
    "confidence": <0.0-1.0>,
    "next_instruction": "<optional instruction for work agent>"
  }
```

### 6.2 Context Injection Strategy

| Context | When to Include |
|---------|----------------|
| `task_description` | Always |
| `provider_output` | Always |
| `file_changes` | When checking completion |
| `reflection_history` | When retrying |
| `decision_history` | When analyzing patterns |
| `sprint_context` | In multi-sprint flows |

### 6.3 JSON Schema for Structured Output

```yaml
parser:
  kind: Json
  payload:
    schema:
      type: object
      properties:
        decision:
          type: string
          enum: [proceed, retry, escalate, reflect]
        reasoning:
          type: string
        confidence:
          type: number
          minimum: 0.0
          maximum: 1.0
        next_instruction:
          type: string
        missing_items:
          type: array
          items:
            type: string
      required: [decision, reasoning, confidence]
```

---

## 7. Blackboard Extensions

### 7.1 New Fields for Decision Flow

```rust
pub struct Blackboard {
    // Existing fields...
    
    // Sprint tracking
    pub current_sprint: u8,
    pub total_sprints: u8,
    pub sprint_goals: Vec<String>,
    
    // Reflection chain
    pub reflection_chain: Vec<ReflectionEntry>,
    
    // Decision history
    pub decision_chain: Vec<DecisionEntry>,
    
    // Outcome tracking
    pub recent_outcomes: Vec<NodeOutcome>,
    
    // Work Agent ID (for SendInstruction)
    pub work_agent_id: String,
}

pub struct ReflectionEntry {
    pub sprint: u8,
    pub result: String,  // proceed/retry/escalate
    pub reasoning: String,
    pub timestamp: Instant,
}

pub struct DecisionEntry {
    pub node_name: String,
    pub decision: String,
    pub outcome: NodeStatus,
}
```

### 7.2 Template Access

Update `BlackboardExt::to_template_context`:

```rust
impl BlackboardExt for Blackboard {
    fn to_template_context(&self) -> Value {
        let mut ctx = HashMap::new();
        
        // Sprint context
        ctx.insert("current_sprint", Value::from(self.current_sprint as i64));
        ctx.insert("total_sprints", Value::from(self.total_sprints as i64));
        ctx.insert("sprint_goal", Value::from(
            self.sprint_goals.get(self.current_sprint as usize)
                .map(|s| s.as_str())
                .unwrap_or("")
        ));
        
        // Reflection chain
        let reflections: Vec<Value> = self.reflection_chain.iter()
            .map(|r| {
                let mut map = HashMap::new();
                map.insert("sprint", Value::from(r.sprint as i64));
                map.insert("result", Value::from(r.result.as_str()));
                map.insert("reasoning", Value::from(r.reasoning.as_str()));
                Value::from(map)
            })
            .collect();
        ctx.insert("reflection_chain", Value::from(reflections));
        
        // Work Agent ID
        ctx.insert("work_agent_id", Value::from(self.work_agent_id.as_str()));
        
        // ... existing fields
    }
}
```

---

## 8. Error Handling

### 8.1 Prompt Timeout

Behavior tree already handles timeout:

```yaml
- kind: Prompt
  payload:
    timeoutMs: 30000  # 30 second timeout
```

On timeout, PromptNode returns Failure, Selector routes to fallback.

### 8.2 Parse Failure

```yaml
- kind: Sequence
  children:
    - kind: Prompt
      payload:
        parser:
          kind: Json
    - kind: Selector
      children:
        - Condition: "parsed_output.valid == true"
        - ForceHuman:
            reason: "AI response couldn't be parsed"
```

### 8.3 AI Uncertainty

Prompt AI to output confidence:

```yaml
template: |
  Output confidence level:
  - 0.9+: Certain decision
  - 0.7-0.9: Likely correct
  - 0.5-0.7: Uncertain, use fallback
  
  Output: {"decision": "...", "confidence": <number>}
```

Selector routes based on confidence:

```yaml
- kind: Selector
  children:
    - Condition: "confidence >= 0.7"
      then: proceed
    - Action: use fallback rules
```

### 8.4 Retry with Strategy Adjustment

```yaml
- kind: Repeater
  payload:
    maxAttempts: 3
    child:
      kind: Sequence
      children:
        - kind: Prompt
          payload:
            name: analyze_failure
            template: |
              Previous attempt failed: {{ previous_output }}
              
              Why did it fail? What different approach should we try?
              
              Output:
              {
                "failure_reason": "...",
                "alternative_strategy": "...",
                "adjusted_instruction": "..."
              }
        - kind: Prompt
          payload:
            name: retry_with_strategy
            template: |
              Retry with adjusted approach.
              
              Strategy: {{ analyze_failure.alternative_strategy }}
              Instruction: {{ analyze_failure.adjusted_instruction }}
```

---

## 9. Conversation History Management

### 9.1 Session-Level History

`ClaudeDecisionSession` maintains conversation history:

```rust
pub struct ClaudeDecisionSession {
    history: Vec<Message>,
    max_history_tokens: usize,
}

impl ClaudeDecisionSession {
    fn prune_history(&mut self) {
        // When approaching token limit
        while self.estimate_tokens() > self.max_history_tokens {
            // Summarize oldest messages
            let summary = self.summarize_old_messages();
            self.history = vec![
                Message::system(format!("Previous context: {}", summary)),
                // Keep recent messages
            ];
            self.history.extend(self.recent_messages.drain(..));
        }
    }
}
```

### 9.2 Decision-Level History

Blackboard stores key decisions:

```rust
pub struct Blackboard {
    pub decision_chain: Vec<DecisionEntry>,
}
```

Template includes previous decisions:

```yaml
template: |
  Previous decisions in this cycle:
  {% for entry in decision_chain %}
  - {{ entry.node_name }}: {{ entry.decision }} → {{ entry.outcome }}
  {% endfor %}
```

---

## 10. Implementation Roadmap

### 10.1 Phase 1: Session Implementation (Week 1)

1. Implement `ClaudeDecisionSession` in `agent-daemon`
2. Connect to Claude API with conversation history
3. Add timeout handling and retry logic
4. Test with simple PromptNode scenarios

### 10.2 Phase 2: Blackboard Extensions (Week 1)

1. Add sprint tracking fields
2. Add reflection/decision chain storage
3. Update `BlackboardExt` for template access
4. Test template rendering with new fields

### 10.3 Phase 3: Decision Agent Slot (Week 2)

1. Implement `DecisionAgentSlot` in `agent-daemon`
2. Integrate behavior tree executor
3. Connect Session to executor
4. Implement command dispatch to Work Agent

### 10.4 Phase 4: Event Loop Integration (Week 2)

1. Trigger decision tick on Work Agent output
2. Handle Running status (poll for AI response)
3. Handle Success/Failure routing
4. Add escalation path to human interface

### 10.5 Phase 5: Flow Templates (Week 3)

1. Create sprint-completion-flow tree template
2. Create task-verification-flow template
3. Create error-recovery-flow template
4. Test full decision cycles

### 10.6 Phase 6: Production Integration (Week 3)

1. Deploy to main agent workflow
2. Monitor decision quality metrics
3. Adjust prompt templates based on results
4. Add human override capabilities

---

## 11. Testing Strategy

### 11.1 Unit Tests

Test each component independently:

```rust
#[test]
fn prompt_node_routes_based_on_ai_output() {
    let mut bb = Blackboard::default();
    let mut session = MockSession::with_reply(
        r#"{"decision": "proceed", "confidence": 0.9}"#
    );
    
    let mut tree = simple_tree(Node::Prompt(...));
    let mut executor = Executor::new();
    
    let result = executor.tick(&mut tree, &mut ctx).unwrap();
    
    assert_eq!(result.status, NodeStatus::Success);
    assert_eq!(bb.get("decision_output"), Some(&"proceed"));
}

#[test]
fn selector_routes_to_retry_on_low_confidence() {
    let mut bb = Blackboard::default();
    bb.set("confidence", BlackboardValue::Float(0.5));
    
    let mut tree = selector_with_confidence_routing();
    let result = executor.tick(&mut tree, &mut ctx).unwrap();
    
    // Should route to retry branch
    assert!(result.commands.contains(&DecisionCommand::Agent(AgentCommand::SendInstruction { ... })));
}
```

### 11.2 Integration Tests

Test full decision flow with mock AI:

```rust
#[test]
fn sprint_completion_flow_verifies_all_sprints() {
    let mut session = MockDecisionSession::new();
    session.set_responses([
        "Sprint 1 complete",
        r#"{"decision": "proceed", "confidence": 0.8}"#,
        "Sprint 2 complete",
        r#"{"decision": "proceed", "confidence": 0.85}"#,
        // ... 4 sprints
    ]);
    
    let mut slot = DecisionAgentSlot::new(sprint_completion_tree(), session);
    
    for _ in 0..8 {  // 4 sprints × 2 prompts each
        slot.tick();
    }
    
    // Verify all sprints processed
    assert_eq!(slot.blackboard.current_sprint, 5);  // incremented past 4
    assert!(slot.blackboard.commands.contains(&TaskCommand::ConfirmCompletion));
}
```

---

## 12. Summary

This design enables:

| Capability | Implementation |
|------------|----------------|
| AI judges completion | PromptNode asks AI, returns structured decision |
| Structured reflection flow | Sequence of PromptNodes for multi-angle reflection |
| Retry with learning | Repeater + failure analysis PromptNode |
| Sprint verification | Behavior tree defines 4-sprint flow with checks |
| Command output | ActionNode sends AI-decided instruction to Work Agent |
| Human escalation | ForceHuman for unrecoverable cases |

**Key architectural decisions**:

1. Behavior tree is process skeleton, not execution engine
2. PromptNode is the AI decision point
3. Selector routes based on AI's structured JSON output
4. Blackboard stores AI outputs for chain reasoning
5. ActionNode outputs DecisionCommand to Work Agent
6. Session maintains conversation history for context

This transforms behavior trees from static rule systems into AI-driven decision flows, enabling autonomous verification, reflection, and retry without human intervention.