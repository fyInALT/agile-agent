# V2 Sprint 2 Continuation and Completion Spec

## Metadata

- Sprint: `V2 / Sprint 2`
- Stories covered:
  - `V2-S04`
  - `V2-S05`

## 1. Purpose

This sprint upgrades the one-shot autonomous iteration into a real execution loop by teaching the system:

- when to continue
- when to consider a task complete

## 2. Scope

### In scope

- continuation policy
- completion judge
- loop state transitions around continued execution

### Out of scope

- verification execution
- escalation
- multi-iteration backlog draining

## 3. Sprint Goal

The system can automatically keep pushing an unfinished task until it either reaches a credible completion state or clearly requires further processing.

## 4. Product Decisions

- “Done” is not provider self-report only.
- Continuation policy should stay heuristic and observable.
- All continue/stop decisions must be transcript-visible or artifact-visible.

## 5. Detailed Execution Checklist

## S2-T01 Add continuation policy

- Detect unfinished execution states such as:
  - analysis only
  - code changed but not tested
  - explicit next-step suggestion

## S2-T02 Add completion judge

- Evaluate task state from:
  - provider result
  - task objective
  - explicit result signals

## S2-T03 Add loop continuation state

- Support repeated turns on the same task.

## S2-T04 Record continue/complete decisions

- Make the system’s decisions visible.

## 6. Acceptance

Sprint 2 is done when:

1. The system can automatically continue at least one unfinished task.
2. The system can stop on a credible completion state.
3. Continue/stop decisions are visible.

## 7. Test Plan

- continuation heuristics tests
- completion judge tests
- autonomous continuation smoke test

## 8. Review Demo

1. Run a task that produces an obviously unfinished intermediate result.
2. Show the system continuing automatically.
3. Show the system stopping when completion is judged.
