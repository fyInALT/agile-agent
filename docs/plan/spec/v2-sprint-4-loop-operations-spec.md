# V2 Sprint 4 Loop Operations Spec

## Metadata

- Sprint: `V2 / Sprint 4`
- Stories covered:
  - `V2-S09`
  - `V2-S10`
  - `V2-S11`
  - `V2-S13`
  - `V2-S14`

## 1. Purpose

This sprint turns the V2 loop from a controlled single-path prototype into an operational system that can:

- run multiple iterations
- be observed in the TUI
- be launched headlessly
- enforce execution guardrails
- resume from the latest loop state

## 2. Scope

### In scope

- multi-iteration loop execution
- TUI loop-state visibility
- headless run-loop mode
- retry and execution guardrails
- recent-state loop continuity

### Out of scope

- multi-agent orchestration
- Scrum event automation
- workflow self-improvement

## 3. Sprint Goal

The V2 autonomous loop can run repeatedly, visibly, and safely in both TUI and headless modes.

## 4. Product Decisions

- Operations features must not hide what the loop is doing.
- Guardrails are mandatory, not optional.
- Recent-state continuity is enough; full historical replay is not required.

## 5. Detailed Execution Checklist

## S4-T01 Support multiple loop iterations

- Continue selecting work until no ready work or a guardrail stops the loop.

## S4-T02 Expose loop state in the TUI

- Show planning/executing/verifying/escalating/idle states.

## S4-T03 Add headless run-loop command

- Add a CLI command that runs the loop without the TUI.

## S4-T04 Add retry/iteration/failure guardrails

- Max iterations
- Max retries
- Max verification failures

## S4-T05 Add recent loop-state continuity

- Save enough state to continue after restart.

## 6. Acceptance

Sprint 4 is done when:

1. The loop can run more than one iteration.
2. Loop state is visible in the TUI.
3. The loop can run headlessly.
4. Guardrails stop runaway behavior.
5. Recent-state restore can continue the loop.

## 7. Test Plan

- multi-iteration loop tests
- headless command tests
- guardrail tests
- restore-continuity tests

## 8. Review Demo

1. Run multiple autonomous iterations.
2. Show loop state in the TUI.
3. Run the loop headlessly.
4. Trigger a guardrail and show safe stop behavior.
