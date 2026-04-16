# Launch Config & Resume Design Sprint Specs

## Overview

This directory contains sprint specifications for implementing the Agent Launch Config and Resume feature, based on the design document at `docs/superpowers/specs/2026-04-16-agent-launch-config-and-resume-design.md`.

## Sprint Sequence

| Sprint | Title | Duration | Status |
|--------|-------|----------|--------|
| [Sprint 1](./sprint-1-core-data-model.md) | Core Data Model & Parser | 2 weeks | Backlog |
| [Sprint 2](./sprint-2-resolver-persistence.md) | Resolver & Persistence | 2 weeks | Backlog |
| [Sprint 3](./sprint-3-provider-ui.md) | Provider Integration & UI Overlay | 2 weeks | Backlog |
| [Sprint 4](./sprint-4-resume-integration.md) | Resume Integration & Error Handling | 1-2 weeks | Backlog |

## Architecture Summary

The feature introduces per-agent launch configuration as a first-class model:

```
┌─────────────────────────────────────────────────────────────────┐
│                     Agent Launch Bundle                          │
│                                                                  │
│  ┌─────────────────────┐      ┌─────────────────────┐          │
│  │ Work Agent Config   │      │ Decision Agent Config│          │
│  │                     │      │                      │          │
│  │ LaunchInputSpec     │      │ LaunchInputSpec      │          │
│  │ ResolvedLaunchSpec  │      │ ResolvedLaunchSpec   │          │
│  └─────────────────────┘      └─────────────────────┘          │
│                                                                  │
│  Persistence: launch-config.json + TuiResumeSnapshot            │
└─────────────────────────────────────────────────────────────────┘
```

## Key Design Decisions

1. **Provider selection is first** - User selects provider before entering config
2. **Work and decision configs are independent** - Empty decision config uses host default, not work config
3. **Two input syntaxes** - Pure env mode (KEY=VALUE) and command fragment mode
4. **Resume uses resolved config** - Not re-reading host environment at restore time
5. **Mock is excluded** - No launch overrides for Mock provider

## Related Documents

- Design Document: `docs/superpowers/specs/2026-04-16-agent-launch-config-and-resume-design.md`
- Related Sprint: `docs/plan/spec/multi-agent/sprint-06-shutdown-restore.md`
- Logging Design: `docs/superpowers/specs/2026-04-16-multi-agent-logging-design.md`
