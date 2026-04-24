# Decision DSL Implementation Plan

Implementation plan for the `decision-dsl` crate, split into 5 Scrum sprints.

## Sprint Overview

| Sprint | Title | Duration | Focus | Key Deliverable |
|--------|-------|----------|-------|-----------------|
| [Sprint 1](sprint-01-core-types-and-foundation.md) | Core Types & Parser Foundation | 2 weeks | Error types, traits, Blackboard, Command enum | `cargo test --lib` passes on core modules |
| [Sprint 2](sprint-02-ast-and-desugaring.md) | AST & Desugaring | 2 weeks | AST model, YAML parser, desugaring, validation | `DecisionRules` YAML compiles to `BehaviorTree` AST |
| [Sprint 3](sprint-03-evaluators-parsers-and-templates.md) | Evaluators, Parsers & Templates | 2 weeks | 9 evaluators, 4 parsers, minijinja templates | All evaluators and parsers tested in isolation |
| [Sprint 4](sprint-04-runtime-executor.md) | Runtime Executor | 2 weeks | Executor tick loop, all 14 node behaviors | Full decision cycle produces `Vec<DecisionCommand>` |
| [Sprint 5](sprint-05-observability-hot-reload-and-integration.md) | Observability, Hot Reload & Integration | 2 weeks | Tracing, hot reload, mocks, host bridges | Production-ready for `agent-decision` integration |

## Dependency Graph

```
Sprint 1 (Foundation)
  │
  ├──► Sprint 2 (AST & Parser)
  │      │
  │      ├──► Sprint 3 (Evaluators & Templates)
  │      │      │
  │      │      └──► Sprint 4 (Runtime)
  │      │             │
  │      │             └──► Sprint 5 (Ops & Integration)
  │      │
  │      └──► Sprint 4 (Runtime) ──► Sprint 5
  │
  └──► Sprint 3 ──► Sprint 4 ──► Sprint 5
```

## Design Documents

These sprints implement the specification documents in `docs/decision-dsl/`:

- `decision-dsl.md` — DSL language specification (YAML syntax)
- `implementation/README.md` — Package structure, public API, integration
- `implementation/decision-dsl-ast.md` — AST design, desugaring, Blackboard, Command enum
- `implementation/decision-dsl-runtime.md` — Executor, node behaviors, Prompt lifecycle
- `implementation/decision-dsl-evaluators.md` — Evaluator and OutputParser enums
- `implementation/decision-dsl-template.md` — minijinja template engine
- `implementation/decision-dsl-ext.md` — Traits, errors, testing, hot reload, observability
- `implementation/decision-dsl-reflection.md` — Historical design rationale

## Language Policy

All spec documents under `docs/plan/dsl/` are written in English, per `docs/plan/spec/README.md`.
