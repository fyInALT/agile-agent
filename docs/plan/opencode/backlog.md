# OpenCode Provider Backlog

## Metadata

- Created: 2026-04-13
- Project: agile-agent
- Target: OpenCode Provider Support
- Language: English

## Overview

This backlog breaks down OpenCode provider integration into Scrum-style sprints.

## Sprint Summary

| Sprint | Title | Focus | Stories | Est. Days |
|--------|-------|-------|---------|-----------|
| Sprint O1 | Foundation | OpenCode detection & enum | 3 | 1 |
| Sprint O2 | ACP Protocol | Session negotiation | 4 | 2 |
| Sprint O3 | Event Mapping | ProviderEvent translation | 3 | 1 |
| Sprint O4 | Integration | Full provider integration | 3 | 1 |

**Total Estimated**: ~5 days

## Sprint Documents

- [Sprint O1: Foundation](./sprint-o1-foundation.md) (inline)
- [Sprint O2: ACP Protocol](./sprint-o2-acp-protocol.md) (inline)
- [Sprint O3: Event Mapping](./sprint-o3-event-mapping.md) (inline)
- [Sprint O4: Integration](./sprint-o4-integration.md) (inline)

## Priority Levels

| Priority | Label | Meaning |
|----------|-------|---------|
| P0 | Critical | Must complete for basic OpenCode support |
| P1 | High | Should complete for full integration |
| P2 | Medium | Nice to have |

## Definition of Done

1. **Code Complete**: All tasks implemented
2. **Tests Pass**: Unit tests and integration tests
3. **Documentation**: README updated
4. **Doctor Output**: OpenCode appears in provider list

## Key Design Decisions

### Decision 1: Minimal Integration Approach

**Chosen**: Add OpenCode as new provider module without major refactoring.

**Alternatives Considered**:
- Trait-based abstraction (too complex for 3 providers)
- Protocol adapter pattern (adds unnecessary layer)

**Rationale**: Current architecture handles 2 providers well. Adding 3rd follows established pattern. Can refactor if more providers needed later.

### Decision 2: ACP v1 Protocol

**Chosen**: Implement ACP v1 as documented.

**Notes**:
- ACP streaming (`session/update`) is marked as future feature
- Use polling/waiting pattern for prompt completion
- Permission handling via `permission.asked` notification (auto-approve)

### Decision 3: Session Continuity

**Chosen**: Use `session/load` for multi-turn conversations.

**Session Flow**:
1. `initialize` → get capabilities
2. `session/new` → create session, get `sessionId`
3. `session/prompt` → send user message
4. Store `sessionId` in `SessionHandle::OpenCodeSession`
5. Next turn: `session/load` with stored `sessionId`
6. Repeat `session/prompt`

## Risk Register

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| ACP protocol changes | Low | Medium | Follow stable spec |
| OpenCode CLI unavailable | Medium | Low | Graceful fallback |
| Streaming incomplete | Medium | Medium | Use polling |
| Permission handling | Low | Low | Auto-approve |

## References

- [Provider Analysis](./provider-analysis.md)
- [Integration Plan](./integration-plan.md)
- OpenCode GitHub: https://github.com/anomalyco/opencode
- ACP Specification: https://agentclientprotocol.com/