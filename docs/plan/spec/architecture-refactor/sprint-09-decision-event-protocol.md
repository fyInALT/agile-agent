# Sprint 9: DecisionEvent Protocol Enhancement

## Metadata

- Sprint ID: `sref-009`
- Title: `DecisionEvent Protocol Enhancement`
- Duration: 1.5 weeks
- Priority: P1 (High)
- Status: `Backlog`
- Created: 2026-04-20
- Depends on: `sref-001` (Shared Kernel), `sref-004` (Decision Layer Decoupling)

---

## Background

### The Information Loss Problem

The `DomainEvent → DecisionEvent` conversion in `agent/events/src/decision_event.rs` collapses many rich event variants into generic `StatusUpdate { status: "running" }` messages. This was originally a design choice: the decision layer only "cares about" a subset of events, so irrelevant details were stripped away.

However, this assumption has proven incorrect. The decision layer's **classifier** uses `DecisionEvent` (via `ProviderEvent`) to detect situations that need intervention. When information is stripped too aggressively, the classifier cannot distinguish important scenarios.

### Current Conversion Loss Audit

| DomainEvent Variant | DecisionEvent Mapping | Information Lost |
|--------------------|----------------------|-----------------|
| `ExecCommandOutputDelta { call_id, delta }` | `StatusUpdate { status: "running" }` | Which command? What output? Progress indication |
| `WebSearchStarted { call_id, query }` | `StatusUpdate { status: "websearch started" }` | The actual search query |
| `WebSearchFinished { call_id, query, action }` | `StatusUpdate { status: "websearch completed" }` | Query, action taken, result |
| `ViewImage { call_id, path }` | `StatusUpdate { status: "running" }` | Image path, context |
| `ImageGenerationFinished { call_id, revised_prompt, result, saved_path }` | `StatusUpdate { status: "running" }` | Generation result, path |
| `PatchApplyOutputDelta { call_id, delta }` | `StatusUpdate { status: "running" }` | Patch progress, conflicts |
| `McpToolCallFinished { result_blocks, error, status, is_error }` | `ClaudeToolCallFinished { name: "mcp", .. }` | Full result blocks, structured error |

### Why This Matters for Classification

The classifier in `decision/src/classifier/classifier_registry.rs` makes situation-type judgments based on `ProviderEvent` content:

```rust
// classifier_registry.rs
match event {
    ProviderEvent::Finished { .. } => Some(SituationType::new("claims_completion")),
    ProviderEvent::Error { .. } => Some(SituationType::new("error")),
    ProviderEvent::ClaudeToolCallFinished { success, .. } => {
        if !success { Some(SituationType::new("error")) } else { None }
    }
    ProviderEvent::StatusUpdate { .. } => None, // ← ALL collapsed events end up here
    // ...
}
```

When `WebSearchFinished` collapses to `StatusUpdate`, the classifier cannot detect:
- A web search that returned no results (might need human help)
- A web search that resulted in a 404/error (might need retry)
- A patch apply that produced conflicts (needs human resolution)

### Root Cause

`DecisionEvent` was designed as a **minimal subset** for the decision layer. The design philosophy was "decisions only need to know: finished, error, or needs approval." But the **classifier** needs richer context to make nuanced situation classifications.

The `StatusUpdate` catch-all was meant for "progress events that don't need decisions," but it has become a dumping ground for events that *do* contain decision-relevant information.

---

## Sprint Goal

Extend `DecisionEvent` with specific variants for previously collapsed event types, update the conversion logic to preserve critical fields, and enhance the classifier to leverage the new information. No new dependencies; all changes within `agent-events` and `agent-decision`.

---

## Stories

### Story 9.1: Audit Information Loss and Define Retention Policy

**Priority**: P0
**Effort**: 2 points
**Status**: Backlog

Catalog every `DomainEvent` variant and decide which fields must be preserved in `DecisionEvent` for accurate classification.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T9.1.1 | List all 24 `DomainEvent` variants | Todo | - |
| T9.1.2 | For each variant, identify which classifier decisions depend on its fields | Todo | - |
| T9.1.3 | Define retention policy: `Full` (all fields), `Partial` (key fields), `Summary` (status string only) | Todo | - |
| T9.1.4 | Produce retention matrix: `DomainEvent` variant × `DecisionEvent` target × `retained fields` | Todo | - |
| T9.1.5 | Review with classifier owners to validate retention choices | Todo | - |

#### Acceptance Criteria

- Every `DomainEvent` variant has a defined retention policy
- Retention choices are justified by classifier requirements
- Matrix is approved by decision layer maintainers

#### Technical Notes

Example retention matrix (partial):

```
DomainEvent                    │ Policy    │ DecisionEvent Target              │ Retained Fields
───────────────────────────────┼───────────┼───────────────────────────────────┼────────────────────────────────
ExecCommandOutputDelta         │ Summary   │ StatusUpdate                      │ "running" (no change)
WebSearchStarted               │ Partial   │ NEW: WebSearchStarted             │ call_id, query
WebSearchFinished              │ Partial   │ NEW: WebSearchFinished            │ call_id, query, action
ViewImage                      │ Summary   │ StatusUpdate                      │ "running" (no change)
ImageGenerationFinished        │ Partial   │ NEW: ImageGenerationFinished      │ call_id, result, saved_path
PatchApplyOutputDelta          │ Summary   │ StatusUpdate                      │ "running" (no change)
McpToolCallFinished            │ Partial   │ ClaudeToolCallFinished (enhanced) │ result_blocks, error, is_error
```

Rationale: `WebSearchFinished` is critical because a failed search might indicate the agent is stuck and needs a different approach — a decision-relevant situation.

---

### Story 9.2: Extend DecisionEvent with Specific Variants

**Priority**: P0
**Effort**: 3 points
**Status**: Backlog

Add new `DecisionEvent` variants and enhance existing ones to carry retained fields.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T9.2.1 | Add `WebSearchStarted { call_id: Option<String>, query: String }` | Todo | - |
| T9.2.2 | Add `WebSearchFinished { call_id: Option<String>, query: String, action: Option<WebSearchAction> }` | Todo | - |
| T9.2.3 | Add `ImageGenerationFinished { call_id: Option<String>, result: Option<String>, saved_path: Option<String> }` | Todo | - |
| T9.2.4 | Enhance `ClaudeToolCallFinished` with `result_blocks: Option<Vec<String>>` (for MCP results) | Todo | - |
| T9.2.5 | Enhance `Error` with `error_type: Option<String>` populated from domain context | Todo | - |
| T9.2.6 | Update `DecisionEvent::needs_decision()` to include new variants where appropriate | Todo | - |
| T9.2.7 | Update `DecisionEvent::is_running()` to include new variants where appropriate | Todo | - |
| T9.2.8 | Write unit tests for all new variant construction | Todo | - |

#### Acceptance Criteria

- `DecisionEvent` enum covers all retention-policy variants from Story 9.1
- New variants carry all retained fields
- `needs_decision()` and `is_running()` are accurate for new variants
- Serde compatibility is maintained (existing serialized events still parse)

#### Technical Notes

```rust
pub enum DecisionEvent {
    // ... existing variants ...

    // NEW: Web search events (previously collapsed to StatusUpdate)
    WebSearchStarted {
        call_id: Option<String>,
        query: String,
    },

    WebSearchFinished {
        call_id: Option<String>,
        query: String,
        action: Option<WebSearchAction>,
    },

    // NEW: Image generation (previously collapsed to StatusUpdate)
    ImageGenerationFinished {
        call_id: Option<String>,
        result: Option<String>,
        saved_path: Option<String>,
    },

    // ENHANCED: Tool call finished (adds MCP result blocks)
    ClaudeToolCallFinished {
        name: String,
        output: Option<String>,
        success: bool,
        // NEW:
        result_blocks: Option<Vec<String>>,
    },

    // ... existing variants ...
}
```

**Backward compatibility note**: Adding variants to an enum is backward-compatible for deserialization if the enum uses `#[serde(tag = "type")]` or similar. However, `DecisionEvent` currently uses default serde enum representation. Adding variants does NOT break existing JSON parsing — unknown variants will simply fail to deserialize, which is acceptable since `DecisionEvent` is not persisted long-term.

---

### Story 9.3: Update DomainEvent → DecisionEvent Conversion

**Priority**: P0
**Effort**: 3 points
**Status**: Backlog

Rewrite the `From<&DomainEvent> for Option<DecisionEvent>` implementation to use the new variants.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T9.3.1 | Update `WebSearchStarted` conversion to produce `DecisionEvent::WebSearchStarted` | Todo | - |
| T9.3.2 | Update `WebSearchFinished` conversion to produce `DecisionEvent::WebSearchFinished` | Todo | - |
| T9.3.3 | Update `ViewImage` / `ImageGenerationFinished` conversion | Todo | - |
| T9.3.4 | Update `McpToolCallFinished` conversion to populate `result_blocks` | Todo | - |
| T9.3.5 | Keep `ExecCommandOutputDelta` / `PatchApplyOutputDelta` as `StatusUpdate { "running" }` (confirmed low-value) | Todo | - |
| T9.3.6 | Write unit tests for every conversion arm | Todo | - |
| T9.3.7 | Ensure `#[deny(warnings)]` on `From` impl to catch new DomainEvent variants | Todo | - |

#### Acceptance Criteria

- Every `DomainEvent` variant maps to the most specific `DecisionEvent` possible
- No information is lost for variants with `Full` or `Partial` retention policy
- Conversion tests verify field-by-field equality
- Adding a new `DomainEvent` variant without updating the `From` impl causes a compile error

#### Technical Notes

The `#[deny(warnings)]` trick for exhaustiveness:

```rust
impl From<&DomainEvent> for Option<DecisionEvent> {
    fn from(event: &DomainEvent) -> Self {
        #[allow(unreachable_patterns)]
        match event {
            // ... all variants explicitly matched ...
        }
    }
}
```

Actually, Rust's `match` on enums is already exhaustive. If a new `DomainEvent` variant is added, the `match` will fail to compile unless `_ =>` is used. Currently the `From` impl has no `_ =>` catch-all, so it IS exhaustive. This is good — we just need to maintain it.

---

### Story 9.4: Enhance Classifier to Leverage New Variants

**Priority**: P0
**Effort**: 4 points
**Status**: Backlog

Update `agent-decision` classifiers to use the new `DecisionEvent` variants for more accurate situation classification.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T9.4.1 | Update `classifier_registry.rs` to match `WebSearchStarted` / `WebSearchFinished` | Todo | - |
| T9.4.2 | Classify `WebSearchFinished` with failed/null action as `SituationType::new("web_search_failed")` | Todo | - |
| T9.4.3 | Update `ClaudeClassifier` to inspect `result_blocks` in `ClaudeToolCallFinished` | Todo | - |
| T9.4.4 | Classify MCP tool calls with `is_error = true` as `SituationType::with_subtype("error", "mcp_tool_failed")` | Todo | - |
| T9.4.5 | Update `ErrorSituation` builder to use `error_type` from enhanced `DecisionEvent::Error` | Todo | - |
| T9.4.6 | Write classifier tests for new event variants | Todo | - |
| T9.4.7 | Benchmark classification accuracy before/after (manual spot-check) | Todo | - |

#### Acceptance Criteria

- `WebSearchFinished` with no results triggers a decision (previously: ignored as `StatusUpdate`)
- MCP tool call failures trigger `error` situation with `mcp_tool_failed` subtype
- Classification accuracy is improved for previously "invisible" events
- No regressions: existing classification tests still pass

#### Technical Notes

The classifier architecture uses trait objects:

```rust
// classifier_registry.rs
impl ClassifierRegistry {
    pub fn classify(&self, event: &ProviderEvent, provider: ProviderKind) -> ClassifyResult {
        // First: try provider-specific classifiers
        if let Some(classifier) = self.classifiers.get(&provider) {
            if let Some(situation) = classifier.classify_type(event) {
                return ClassifyResult::NeedsDecision { situation_type: situation, ... };
            }
        }

        // Fallback: registry-wide rules
        self.classify_type(event)
    }
}
```

`ProviderEvent` in `agent-decision` is equivalent to `DecisionEvent`. We need to update the `classify_type` method to match the new variants.

---

### Story 9.5: End-to-End Classification Validation

**Priority**: P1
**Effort**: 3 points
**Status**: Backlog

Run end-to-end tests that feed realistic `DomainEvent` sequences through the full pipeline (`DomainEvent → DecisionEvent → ProviderEvent → ClassifyResult → DecisionCommand`) and verify correct situation detection.

#### Tasks

| ID | Task | Status | Assignee |
|----|------|--------|----------|
| T9.5.1 | Create `ClassificationE2ETest` harness | Todo | - |
| T9.5.2 | Test: Web search sequence → `NeedsDecision { "web_search_failed" }` | Todo | - |
| T9.5.3 | Test: MCP error sequence → `NeedsDecision { "error" / "mcp_tool_failed" }` | Todo | - |
| T9.5.4 | Test: Image generation success → `Running` (no decision) | Todo | - |
| T9.5.5 | Test: Patch apply with delta → `Running` (no decision, confirmed correct) | Todo | - |
| T9.5.6 | Test: Mixed event sequence (streaming + tool + error) → correct final situation | Todo | - |
| T9.5.7 | Document classification accuracy improvements | Todo | - |

#### Acceptance Criteria

- All new event variants have at least one end-to-end classification test
- Previously "invisible" events now produce correct `ClassifyResult`
- No false positives: events that should produce `Running` still do
- Test suite runs in < 1 second per scenario (no real LLM calls)

#### Technical Notes

End-to-end test example:

```rust
#[test]
fn web_search_no_results_triggers_decision() {
    let events = vec![
        DomainEvent::WebSearchStarted {
            call_id: Some("ws-1".to_string()),
            query: "nonexistent_library_rust".to_string(),
        },
        DomainEvent::WebSearchFinished {
            call_id: Some("ws-1".to_string()),
            query: "nonexistent_library_rust".to_string(),
            action: None, // No action taken = no useful results
        },
    ];

    let classifier = ClassifierRegistry::new();
    for event in &events {
        let decision_event: Option<DecisionEvent> = event.into();
        let provider_event = ProviderEvent::from(decision_event.unwrap());
        let result = classifier.classify(&provider_event, ProviderKind::Claude);

        if let ClassifyResult::NeedsDecision { situation_type, .. } = result {
            assert_eq!(situation_type.name, "web_search_failed");
            return;
        }
    }
    panic!("Expected NeedsDecision for web search with no results");
}
```

---

## Dependency Graph

```
Story 9.1 (Audit)
    │
    └──► Story 9.2 (Extend DecisionEvent)
              │
              ├──► Story 9.3 (Update Conversion)
              │         │
              │         └──► Story 9.4 (Enhance Classifier)
              │                   │
              │                   └──► Story 9.5 (E2E Validation)
              │
              └──► Story 9.5 (Test design can start in parallel)
```

## Risks

| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|------------|
| New `DecisionEvent` variants bloat the enum (too many variants) | Medium | Medium | Group related events under a single variant with enum payload (e.g., `ToolEvent { kind, ... }`) |
| Classifier changes cause false positives for previously ignored events | Medium | High | Extensive E2E testing; start with conservative classification rules |
| `ProviderEvent` in `agent-decision` diverges from `DecisionEvent` | Low | High | Add compile-time assertion or shared type alias |
| Serde backward compatibility broken for in-flight events | Low | Medium | `DecisionEvent` is not persisted; only affects runtime channels |

## Definition of Done

- [ ] `DecisionEvent` has specific variants for all high-value `DomainEvent` types
- [ ] `From<&DomainEvent>` conversion is exhaustive (compile-time verified)
- [ ] Classifier uses new variants for improved situation detection
- [ ] End-to-end tests verify correct classification for all new paths
- [ ] No regressions in existing classification test suite
- [ ] `cargo clippy --workspace --tests -- -D warnings` passes
- [ ] `cargo test --workspace --lib` passes with zero failures
