# Behavior Tree Driven AI Decision Flow

## Overview

The goal is to use behavior trees as **process skeletons** that guide AI (Codex/Claude) through structured decision workflows. Each node in the tree represents a decision point where AI analyzes Work Agent output and determines the next action.

**Key insight**: Behavior tree defines *when* and *what* to ask AI; AI determines *how* to respond and *what command* to send to Work Agent.

---

## 1. Core Concept

### 1.1 Behavior Tree as Decision Flow Skeleton

```
┌─────────────────────────────────────────────────────────────┐
│              Behavior Tree = Process Skeleton                │
│                                                              │
│  Defines:                                                    │
│  - When to invoke AI (at each PromptNode)                   │
│  - What to ask AI (prompt template)                         │
│  - How to route based on AI response (Selector/Sequence)    │
│  - What to retry (Repeater)                                 │
│  - When to escalate (ForceHuman)                            │
│                                                              │
│  AI Determines:                                              │
│  - Whether task is complete                                 │
│  - What command to send to Work Agent                       │
│  - Whether to retry or proceed                              │
│  - Whether situation needs human intervention               │
└─────────────────────────────────────────────────────────────┘
```

### 1.2 Example: Sprint Completion Flow

```yaml
apiVersion: decision.agile-agent.io/v1
kind: BehaviorTree
metadata:
  name: sprint-completion-flow
spec:
  root:
    kind: Repeater
    payload:
      name: complete_all_sprints
      maxAttempts: 4  # Up to 4 sprint iterations
      child:
        kind: Sequence
        payload:
          name: sprint_cycle
          children:
            # Step 1: Ask Work Agent to complete current sprint
            - kind: Prompt
              payload:
                name: do_sprint
                template: |
                  Complete sprint {{ current_sprint }} of {{ total_sprints }}.
                  Focus on: {{ sprint_goal }}
                  
                  Report your progress when done.
                parser:
                  kind: Json
                  payload:
                    schema: null
                sets:
                  - key: sprint_result
                    field: status
                timeoutMs: 60000
                
            # Step 2: AI reflects on completion
            - kind: Prompt
              payload:
                name: reflect_completion
                template: |
                  Work Agent reported: {{ sprint_result }}
                  
                  Analyze:
                  1. Was this sprint actually completed?
                  2. Are there incomplete tasks?
                  3. Should we retry or proceed to next sprint?
                  
                  Output:
                  {"completed": bool, "incomplete_tasks": [], "action": "proceed|retry"}
                parser:
                  kind: Json
                  payload:
                    schema: null
                sets:
                  - key: reflection_result
                    field: action
                    
            # Step 3: Route based on AI reflection
            - kind: Selector
              payload:
                name: route_after_reflection
                children:
                  # If completed, proceed
                  - kind: Condition
                    payload:
                      name: is_completed
                      evaluator:
                        kind: VariableIs
                        payload:
                          key: reflection_result.action
                          expected: "proceed"
                  # If not completed, increment sprint counter and retry
                  - kind: Sequence
                    payload:
                      name: retry_cycle
                      children:
                        - kind: SetVar
                          payload:
                            name: note_incomplete
                            key: incomplete_sprints
                            value: "{{ incomplete_sprints }} + 1"
                        - kind: Action
                          payload:
                            name: retry_sprint
                            command:
                              kind: Agent
                              payload:
                                kind: SendInstruction
                                payload:
                                  prompt: "Previous sprint incomplete. Re-attempt."
                                  target_agent: "{{ work_agent_id }}"
                                
            # Step 4: Alternative reflection angle
            - kind: Prompt
              payload:
                name: alternative_reflection
                template: |
                  Before moving on, verify from different perspective:
                  
                  Sprint {{ current_sprint }} goal: {{ sprint_goal }}
                  Work done: {{ file_changes }}
                  
                  Are we missing anything critical?
                  
                  Output:
                  {"missing": bool, "critical_items": [], "should_pause": bool}
                parser:
                  kind: Json
                  payload:
                    schema: null
                    
            # Step 5: Finalize or escalate
            - kind: Selector
              payload:
                name: finalize_or_escalate
                children:
                  - kind: Condition
                    payload:
                      name: no_missing_items
                      evaluator:
                        kind: VariableIs
                        payload:
                          key: alternative_reflection.missing
                          expected: false
                  - kind: ForceHuman
                    payload:
                      name: escalate_missing
                      reason: "Critical items detected: {{ alternative_reflection.critical_items }}"
                      child:
                        kind: Action
                        payload:
                          name: pause_for_review
                          command:
                            kind: Human
                            payload:
                              kind: EscalateToHuman
                              payload:
                                reason: "Critical items missing after sprint"
```

This tree defines:
1. **Flow structure**: Sequence of steps → reflection → routing → alternative check → finalize
2. **AI decision points**: Each PromptNode asks AI to analyze
3. **Routing logic**: Selector routes based on AI's JSON output
4. **Retry behavior**: Repeater allows up to 4 sprint cycles

---

## 2. Key Patterns

### 2.1 Task Completion Verification

**Problem**: Work Agent says "done" but might be incomplete.

**Solution**: Behavior tree forces AI to verify:

```yaml
# Step 1: Work Agent completes task
- kind: Prompt
  payload:
    name: execute_task
    template: "Complete {{ task_description }}"
    parser: { kind: Json }
    sets: [{ key: task_output, field: result }]

# Step 2: AI verifies completion
- kind: Prompt
  payload:
    name: verify_completion
    template: |
      Task: {{ task_description }}
      Agent output: {{ task_output }}
      
      Has this task been genuinely completed?
      Check for:
      - All requirements addressed
      - Tests passing (if applicable)
      - No TODOs or partial implementations
      
      Output: {"completed": bool, "missing": [], "confidence": float}
```

AI does the verification, not human.

### 2.2 Multi-Iteration Flow Control

**Problem**: Work Agent needs to complete 4 sprints, but might stop at 2.

**Solution**: Repeater + reflection loop:

```yaml
- kind: Repeater
  payload:
    name: complete_all
    maxAttempts: {{ total_sprints }}
    child:
      kind: Sequence
      payload:
        children:
          - Prompt: "Complete sprint {{ current }}"
          - Prompt: "Did you complete sprint {{ current }}?"
          - Selector:
              - Condition: reflection.complete → proceed
              - Action: retry this sprint
```

The tree ensures all sprints are addressed, with AI deciding each iteration's success.

### 2.3 Multi-Angle Reflection

**Problem**: Single reflection might miss issues.

**Solution**: Sequential reflection prompts:

```yaml
children:
  - Prompt: 
      name: primary_reflection
      template: "Verify completion from implementation perspective"
  - Prompt:
      name: test_reflection  
      template: "Verify from testing perspective"
  - Prompt:
      name: edge_case_reflection
      template: "Check for edge cases and error handling"
```

Each prompt asks AI to examine from different angle.

### 2.4 Adaptive Retry Strategy

**Problem**: Simple retry might repeat the same mistake.

**Solution**: Reflection-driven retry:

```yaml
- kind: Sequence
  payload:
    children:
      - Prompt:
          name: analyze_failure
          template: |
            Previous attempt: {{ last_output }}
            Why did it fail? What approach should we try differently?
            
            Output: {"failure_reason": str, "alternative_approach": str}
      - Prompt:
          name: retry_with_strategy
          template: |
            Retry with this approach: {{ analyze_failure.alternative_approach }}
            
            Original goal: {{ task_description }}
```

AI analyzes failure and suggests alternative approach for retry.

---

## 3. PromptNode as Decision Engine

### 3.1 Current PromptNode Behavior

```rust
pub struct PromptNode {
    pub name: String,
    pub template: String,
    pub parser: OutputParser,
    pub sets: Vec<SetMapping>,
    pub timeout_ms: u64,
}
```

Execution:
1. Render template with Blackboard context
2. Send to Session (LLM)
3. Parse response with OutputParser
4. Store result via SetMapping
5. Return Success/Running/Failure

### 3.2 Decision Flow Integration

The `PromptNode` is the **decision point** in the flow:

```
PromptNode execution:
          │
          ▼
    ┌───────────┐
    │ Render    │  template + blackboard → prompt
    │ Template  │
    └───────────┘
          │
          ▼
    ┌───────────┐
    │ Send to   │  prompt → Codex/Claude
    │ AI        │
    └───────────┘
          │
          ▼
    ┌───────────┐
    │ Parse     │  raw response → JSON
    │ Response  │  {"completed": true, "action": "proceed"}
    └───────────┘
          │
          ▼
    ┌───────────┐
    │ Store in  │  JSON → blackboard variables
    │ Blackboard│  reflection_result = "proceed"
    └───────────┘
          │
          ▼
    ┌───────────┐
    │ Next Node │  Selector reads blackboard, routes
    │ Execution │
    └───────────┘
```

### 3.3 OutputParser for Structured Decisions

Use JSON parser for AI decisions:

```yaml
parser:
  kind: Json
  payload:
    schema:
      type: object
      properties:
        completed:
          type: boolean
        action:
          type: string
          enum: [proceed, retry, escalate]
        reasoning:
          type: string
      required: [completed, action]
```

Structured output enables programmatic routing.

---

## 4. Blackboard as Decision State

### 4.1 Key Fields for Decision Flow

```rust
pub struct Blackboard {
    // Task context (from Work Agent)
    pub provider_output: String,
    pub file_changes: Vec<FileChangeRecord>,
    pub last_tool_call: Option<ToolCallRecord>,
    
    // Decision flow state
    pub current_sprint: u8,
    pub total_sprints: u8,
    pub sprint_goal: String,
    pub incomplete_sprints: u8,
    
    // AI reflections (stored by PromptNode.sets)
    // sprint_result, reflection_result, alternative_reflection, etc.
}
```

### 4.2 Template Context Injection

Prompt templates access decision state:

```yaml
template: |
  Sprint {{ current_sprint }} of {{ total_sprints }}
  
  Goal: {{ sprint_goal }}
  Work done: {{ file_changes }}
  Previous reflection: {{ reflection_result }}
```

AI receives full context to make informed decision.

### 4.3 Decision Chain State

Each PromptNode stores its result, enabling chain reasoning:

```
Prompt 1: do_sprint        → sprint_result = {"status": "partial"}
Prompt 2: reflect          → reflection_result = {"action": "retry"}
Prompt 3: alternative_check → alternative_reflection = {"missing": true}
Prompt 4: escalate         → (uses alternative_reflection.missing)
```

Later prompts can reference earlier AI decisions.

---

## 5. Work Agent Command Flow

### 5.1 ActionNode for Command Output

After AI decides, `ActionNode` sends command to Work Agent:

```yaml
- kind: Action
  payload:
    name: send_instruction
    command:
      kind: Agent
      payload:
        kind: SendInstruction
        payload:
          prompt: "{{ reflection_result.next_instruction }}"
          target_agent: "{{ work_agent_id }}"
```

The command content comes from AI's reflection output.

### 5.2 Command Types

| Command | Purpose | When Used |
|---------|---------|-----------|
| `SendInstruction` | Tell Work Agent what to do | After AI decides next step |
| `WakeUp` | Restart paused agent | Agent stalled |
| `Reflect` | Ask agent to self-review | After sprint completion |
| `Terminate` | Stop agent | Task complete or unrecoverable |

### 5.3 Dynamic Command Construction

AI output can construct commands dynamically:

```yaml
# AI outputs: {"command": "commit", "files": ["a.rs", "b.rs"]}
- kind: Action
  payload:
    name: execute_ai_command
    command:
      kind: Git
      payload:
        kind: CommitChanges
        payload:
          message: "{{ ai_output.commit_message }}"
          is_wip: "{{ ai_output.is_wip }}"
```

---

## 6. Complete Example: Feature Implementation Flow

```yaml
apiVersion: decision.agile-agent.io/v1
kind: BehaviorTree
metadata:
  name: feature-implementation
spec:
  root:
    kind: Sequence
    payload:
      name: feature_flow
      children:
        # Phase 1: Planning
        - kind: Prompt
          payload:
            name: plan_feature
            template: |
              Feature: {{ feature_description }}
              Existing code: {{ codebase_summary }}
              
              Create implementation plan with:
              - Files to modify
              - Test files needed
              - Expected steps
              
              Output: {"plan": {"steps": [], "files": [], "tests": []}}
            parser: { kind: Json }
            sets: [{ key: impl_plan, field: plan }]
            
        # Phase 2: Implementation Loop
        - kind: Repeater
          payload:
            name: implement_all_steps
            maxAttempts: "{{ impl_plan.steps | length }}"
            child:
              kind: Sequence
              payload:
                children:
                  # Step execution
                  - kind: Prompt
                    payload:
                      name: do_step
                      template: |
                        Step {{ current_step }}: {{ impl_plan.steps[current_step] }}
                        
                        Implement this step. Report progress.
                      parser: { kind: Json }
                      sets: [{ key: step_result, field: status }]
                      
                  # Step verification
                  - kind: Prompt
                    payload:
                      name: verify_step
                      template: |
                        Step: {{ impl_plan.steps[current_step] }}
                        Result: {{ step_result }}
                        Files changed: {{ file_changes }}
                        
                        Was this step correctly implemented?
                        
                        Output: {"correct": bool, "issues": [], "retry": bool}
                      parser: { kind: Json }
                      
                  # Retry or proceed
                  - kind: Selector
                    payload:
                      children:
                        - Condition: verify_step.correct == true
                        - Action:
                            command:
                              kind: Agent
                              payload:
                                kind: SendInstruction
                                payload:
                                  prompt: "Retry step {{ current_step }}: {{ verify_step.issues }}"
                                  
        # Phase 3: Testing
        - kind: Prompt
          payload:
            name: run_tests
            template: "Run all tests for this feature. Report results."
            parser: { kind: Json }
            sets: [{ key: test_result, field: status }]
            
        # Phase 4: Test Reflection
        - kind: Selector
          payload:
            children:
              - Condition: test_result.passed == true
              - Sequence:
                  children:
                    - Prompt:
                        name: analyze_test_failure
                        template: |
                          Test failures: {{ test_result.failures }}
                          
                          What's wrong? How to fix?
                          
                          Output: {"root_cause": str, "fixes": []}
                    - Action:
                        command:
                          kind: Agent
                          payload:
                            kind: SendInstruction
                            payload:
                              prompt: "Fix test failures: {{ analyze_test_failure.fixes }}"
                              
        # Phase 5: Final Review
        - kind: Prompt
          payload:
            name: final_review
            template: |
              Feature: {{ feature_description }}
              All steps done, tests passing.
              
              Final verification:
              - Are all requirements met?
              - Is code quality acceptable?
              - Any remaining TODOs?
              
              Output: {"complete": bool, "remaining": [], "quality": str}
            parser: { kind: Json }
            
        # Phase 6: Commit or Continue
        - kind: Selector
          payload:
            children:
              - Sequence:
                  condition: final_review.complete == true
                  children:
                    - Action:
                        command:
                          kind: Git
                          payload:
                            kind: CommitChanges
                            payload:
                              message: "feat: {{ feature_description }}"
                              is_wip: false
                    - Action:
                        command:
                          kind: Task
                          payload:
                            kind: ConfirmCompletion
              - ForceHuman:
                  reason: "{{ final_review.remaining }}"
```

This tree:
1. Plans with AI
2. Implements step-by-step with AI verification
3. Tests with AI failure analysis
4. Final review with AI
5. Routes to commit or escalate

---

## 7. Integration Architecture

### 7.1 Decision Layer + Behavior Tree

```
┌───────────────────────────────────────────────────────────────┐
│                    Integration Architecture                     │
│                                                                │
│  ┌─────────────┐    ┌─────────────┐    ┌─────────────┐       │
│  │ Work Agent  │───▶│  Blackboard │───▶│ Behavior    │       │
│  │ (Codex)     │    │  (State)    │    │ Tree        │       │
│  └─────────────┘    └─────────────┘    │ Executor    │       │
│         │                              └─────────────┘       │
│         │                                    │               │
│         ▼                                    ▼               │
│  ┌─────────────┐                       ┌─────────────┐       │
│  │ Provider    │◀──────────────────────│ PromptNode  │       │
│  │ Output      │    Commands            │ (AI Call)   │       │
│  └─────────────┘                       └─────────────┘       │
│                                               │               │
│                                               ▼               │
│                                        ┌─────────────┐       │
│                                        │ AI Session  │       │
│                                        │ (Claude)    │       │
│                                        └─────────────┘       │
│                                               │               │
│                                               ▼               │
│  ┌─────────────┐                       ┌─────────────┐       │
│  │ Decision    │◀──────────────────────│ AI Response │       │
│  │ Command     │    Structured output   │ (JSON)      │       │
│  └─────────────┘                       └─────────────┘       │
│         │                                    │               │
│         ▼                                    ▼               │
│  ┌─────────────┐                       ┌─────────────┐       │
│  │ Back to     │◀──────────────────────│ Store in    │       │
│  │ Work Agent  │    Next instruction    │ Blackboard  │       │
│  └─────────────┘                       └─────────────┘       │
└───────────────────────────────────────────────────────────────┘
```

### 7.2 Session Implementation

```rust
pub struct AiDecisionSession {
    /// Underlying Claude/Codex connection
    llm_client: Box<dyn LlmClient>,
}

impl Session for AiDecisionSession {
    fn send(&mut self, message: &str) -> Result<(), SessionError> {
        // Send prompt to Claude/Codex
        self.llm_client.send(message)
    }
    
    fn is_ready(&self) -> bool {
        // Check if AI response is available
        self.llm_client.has_response()
    }
    
    fn receive(&mut self) -> Result<String, SessionError> {
        // Get AI response
        self.llm_client.receive()
    }
}
```

### 7.3 TickContext Configuration

```rust
let mut ctx = TickContext {
    blackboard: &mut bb,
    session: &mut ai_decision_session,  // AI connection
    clock: &clock,
    logger: &logger,
};
```

`session` is the AI connection (Claude/Codex) for decision prompts.

---

## 8. Prompt Design Guidelines

### 8.1 Structured Output Request

Always request structured output for routing:

```yaml
template: |
  Analyze: {{ situation }}
  
  Output format:
  {
    "decision": "proceed|retry|escalate",
    "reasoning": "brief explanation",
    "confidence": 0.0-1.0,
    "next_instruction": "optional instruction for work agent"
  }
```

Structured output enables:
- Selector routing based on `decision`
- ActionNode using `next_instruction`
- Trace logging with `reasoning`

### 8.2 Context Injection Best Practices

Include relevant context, not everything:

```yaml
# Good: Relevant context for decision
template: |
  Task: {{ task_description }}
  Current step: {{ current_step }}
  Recent work: {{ file_changes | truncate(5) }}
  
  Is this step complete?

# Bad: Too much context
template: |
  Full history: {{ conversation_history }}
  All decisions: {{ decision_history }}
  Every file: {{ file_changes }}
```

### 8.3 Decision Prompts vs Action Prompts

| Type | Purpose | Output |
|------|---------|--------|
| Decision Prompt | AI judges situation | `{"decision": "...", "reasoning": "..."}` |
| Action Prompt | AI instructs Work Agent | `{"instruction": "...", "context": "..."}` |

```yaml
# Decision prompt
- Prompt:
    name: judge_completion
    template: "Is task done? Output: {completed: bool, reasoning: str}"

# Action prompt  
- Prompt:
    name: plan_next
    template: "What should Work Agent do next? Output: {instruction: str}"
```

---

## 9. Error Handling

### 9.1 AI Timeout → Retry

```yaml
- kind: Selector
  payload:
    children:
      - Prompt:  # Primary AI call
          timeoutMs: 30000
      - Prompt:  # Fallback with shorter timeout
          timeoutMs: 10000
          template: "Quick check: {{ provider_output }}. Is this OK?"
```

### 9.2 Parse Failure → Escalate

```yaml
- kind: Sequence
  payload:
    children:
      - Prompt:
          parser: { kind: Json }
      - Selector:
          children:
            - Condition: prompt_output.valid == true
            - ForceHuman:
                reason: "AI response couldn't be parsed: {{ raw_response }}"
```

### 9.3 Repeater for Retry

```yaml
- kind: Repeater
  payload:
    maxAttempts: 3
    child:
      kind: Sequence
      children:
        - Prompt: "Try again with clearer instructions"
        - Condition: success == true
```

---

## 10. Summary

The behavior tree drives AI decision flow:

| Component | Role |
|-----------|------|
| **Behavior Tree** | Defines decision flow skeleton (when to ask, how to route) |
| **PromptNode** | Invokes AI at each decision point |
| **Blackboard** | Stores decision state and AI outputs |
| **Selector/Sequence** | Routes based on AI's structured output |
| **ActionNode** | Sends AI-decided commands to Work Agent |
| **Repeater** | Enables retry loops with AI guidance |
| **ForceHuman** | Escalates when AI cannot decide |

**Key workflow**:
1. Work Agent produces output → stored in Blackboard
2. Behavior Tree Executor reaches PromptNode
3. PromptNode sends context to AI
4. AI analyzes and returns structured decision
5. Parser extracts decision, stores in Blackboard
6. Selector routes to next node based on decision
7. ActionNode sends AI-decided command to Work Agent
8. Loop continues until task completion

This architecture replaces human verification with AI judgment, enabling autonomous decision cycles while maintaining structured process control through behavior trees.