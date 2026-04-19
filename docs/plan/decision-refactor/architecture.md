# 决策层架构重构方案

## 1. 当前问题分析

### 1.1 代码规模
- 总行数：约 26,000 行
- 文件数：47 个 Rust 模块
- 单一 crate，过于庞大

### 1.2 结构问题
- 模块按 Sprint 开发顺序命名，而非按职责分层
- 缺乏清晰的依赖层次
- 部分模块职责混杂

### 1.3 生命周期不清晰
- 决策流程分散在多个模块中
- 主执行流程未明确体现

---

## 2. 生命周期梳理

### 2.1 决策层主执行流程

```text
┌─────────────────────────────────────────────────────────────────────────────┐
│                         决策层生命周期 (Decision Lifecycle)                    │
└─────────────────────────────────────────────────────────────────────────────┘

┌──────────────────────────────────────────────────────────────────────────────┐
│ Phase 1: 初始化阶段 (Initialization)                                          │
├──────────────────────────────────────────────────────────────────────────────┤
│                                                                               │
│   ProviderEvent ──▶ Classifier ──▶ Situation                                 │
│        │              │               │                                       │
│        │              │               ▼                                       │
│        │              │      SituationRegistry.register()                    │
│        │              │                                                       │
│        ▼              ▼                                                       │
│   ProviderKind    ClassifierRegistry                                          │
│                                                                               │
└──────────────────────────────────────────────────────────────────────────────┘

┌──────────────────────────────────────────────────────────────────────────────┐
│ Phase 2: 决策输入构建 (Context Building)                                       │
├──────────────────────────────────────────────────────────────────────────────┤
│                                                                               │
│   Situation ──▶ DecisionContext                                              │
│                     │                                                         │
│                     ├── trigger_situation                                     │
│                     ├── main_agent_id                                         │
│                     ├── running_context (RunningContextCache)                 │
│                     ├── project_rules                                         │
│                     ├── decision_history                                      │
│                     └── metadata                                              │
│                                                                               │
└──────────────────────────────────────────────────────────────────────────────┘

┌──────────────────────────────────────────────────────────────────────────────┐
│ Phase 3: 策略选择 (Strategy Selection)                                         │
├──────────────────────────────────────────────────────────────────────────────┤
│                                                                               │
│   DecisionContext ──▶ DecisionStrategy ──▶ DecisionMakerType                 │
│                           │                                                   │
│                           ├── TieredStrategy                                  │
│                           ├── SituationMappingStrategy                        │
│                           ├── AdaptiveStrategy                                │
│                           └── CompositeStrategy                               │
│                                                                               │
│   策略输出: StrategySelection                                                 │
│     ├── maker_type                                                           │
│     ├── fallback_chain                                                       │
│     └── reason                                                               │
│                                                                               │
└──────────────────────────────────────────────────────────────────────────────┘

┌──────────────────────────────────────────────────────────────────────────────┐
│ Phase 4: 决策执行 (Decision Execution)                                         │
├──────────────────────────────────────────────────────────────────────────────┤
│                                                                               │
│   DecisionPipeline.execute(context)                                          │
│        │                                                                      │
│        ├── 1. Pre-Processing                                                 │
│        │      └── ReflectionRoundPreProcessor                                │
│        │                                                                      │
│        ├── 2. Maker Selection                                                 │
│        │      └── DecisionMakerRegistry.select_maker()                       │
│        │                                                                      │
│        ├── 3. Maker Execution                                                 │
│        │      └── DecisionMaker.make_decision()                              │
│        │             │                                                        │
│        │             ▼                                                        │
│        │      DecisionEngine.decide()                                        │
│        │             │                                                        │
│        │             ├── TieredEngine                                         │
│        │             │      ├── RuleEngine (Simple)                          │
│        │             │      ├── LLMEngine (Medium/Complex)                   │
│        │             │      └── CLIEngine (Critical)                         │
│        │             └── 直接引擎                                             │
│        │                                                                      │
│        ├── 4. Post-Processing                                                 │
│        │      └── ValidateActionsPostProcessor                                │
│        │                                                                      │
│        └── 5. Recording                                                       │
│               └── PipelineDecisionRecord                                     │
│                                                                               │
│   输出: DecisionOutput                                                        │
│     ├── actions[]                                                            │
│     ├── reasoning                                                            │
│     └── confidence                                                           │
│                                                                               │
└──────────────────────────────────────────────────────────────────────────────┘

┌──────────────────────────────────────────────────────────────────────────────┐
│ Phase 5: 输出处理 (Output Handling)                                            │
├──────────────────────────────────────────────────────────────────────────────┤
│                                                                               │
│   DecisionOutput ──▶ DecisionAction 执行                                     │
│        │                                                                      │
│        ├── ContinueAction                                                    │
│        ├── ReflectAction                                                     │
│        ├── RequestHumanAction ──▶ BlockingState                              │
│        ├── ConfirmCompletionAction                                           │
│        ├── SelectOptionAction                                                 │
│        └── CustomInstructionAction                                            │
│                                                                               │
│   BlockingState (人类决策场景):                                                │
│     ├── HumanDecisionBlocking                                                 │
│     ├── RateLimitBlockedReason                                                │
│     └── HumanDecisionQueue                                                    │
│                                                                               │
└──────────────────────────────────────────────────────────────────────────────┘

┌──────────────────────────────────────────────────────────────────────────────┐
│ Phase 6: 任务管理 (Task Management)                                            │
├──────────────────────────────────────────────────────────────────────────────┤
│                                                                               │
│   Task ──▶ DecisionProcess ──▶ TaskDecisionEngine                            │
│      │          │                  │                                          │
│      │          │                  ├── process_output(AgentOutput)           │
│      │          │                  ├── handle_human_response()                │
│      │          │                  └── generate_prompt()                      │
│      │          │                  │                                          │
│      │          │                  ▼                                          │
│      │          │            TaskDecisionAction                              │
│      │          │                  │                                          │
│      │          ▼                  ▼                                          │
│      │    DecisionStage        WorkflowAction                                │
│      │          │                  │                                          │
│      │          ├── transitions     ├── Continue                             │
│      │          ├── actions         ├── Reflect                               │
│      │          └                  ├── ConfirmCompletion                     │
│      │                             └── RequestHuman                           │
│      │                                                                        │
│      ▼                                                                        │
│   Automation Layer:                                                           │
│     ├── AutoChecker ──▶ AutoCheckResult                                      │
│     │      ├── SyntaxCheckRule                                               │
│     │      ├── TestCheckRule                                                 │
│     │      ├── CompileCheckRule                                              │
│     │      ├── BoundaryCheckRule                                             │
│     │      └── RiskCheckRule                                                 │
│     │                                                                          │
│     └── DecisionFilter ──▶ needs_human_decision()                            │
│            └── auto_decide()                                                 │
│                                                                               │
└──────────────────────────────────────────────────────────────────────────────┘

┌──────────────────────────────────────────────────────────────────────────────┐
│ Phase 7: 持久化 (Persistence)                                                  │
├──────────────────────────────────────────────────────────────────────────────┤
│                                                                               │
│   TaskRegistry ──▶ TaskStore ──▶ FileTaskStore                               │
│        │                                                                      │
│        ├── create()                                                          │
│        ├── get()                                                             │
│        ├── update() ──▶ TaskUpdate                                           │
│        ├── complete()                                                        │
│        ├── cancel()                                                          │
│        └── recover() ──▶ 从崩溃恢复                                           │
│                                                                               │
│   ExecutionRecord:                                                            │
│     ├── action                                                               │
│     ├── timestamp                                                            │
│     ├── stage                                                                │
│     ├── auto_check_result                                                    │
│     └── human_response                                                       │
│                                                                               │
└──────────────────────────────────────────────────────────────────────────────┘
```

### 2.2 状态流转图

```text
TaskStatus 状态流转:

┌─────────┐     ┌──────────────┐     ┌────────────┐     ┌──────────────────┐
│ Pending │────▶│  InProgress  │────▶│ Reflecting │────▶│ NeedsHumanDecision│
└─────────┘     └──────────────┘     └────────────┘     └──────────────────┘
     │               │                    │                       │
     │               │                    │                       │
     │               ▼                    ▼                       ▼
     │          ┌─────────────────┐  ┌─────────────┐        ┌─────────────┐
     │          │PendingConfirmation│ │   Paused   │        │  Cancelled  │
     │          └─────────────────┘  └─────────────┘        └─────────────┘
     │               │                    │
     │               ▼                    │
     │          ┌─────────────┐           │
     │          │  Completed  │◀──────────┘
     │          └─────────────┘
     │               │
     └───────────────┘
```

---

## 3. 子包拆分方案

### 3.1 拆分原则
1. **职责单一** - 每个子包只负责一个核心职责
2. **依赖分层** - 依赖关系单向，避免循环依赖
3. **接口清晰** - 子包间通过明确的 trait 和类型交互
4. **便于测试** - 每个子包可独立测试

### 3.2 子包结构

```text
decision/
├── decision-core/           # 核心抽象层（约 1,600 行）
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs
│       ├── types.rs         # 基础类型：ActionType, SituationType, UrgencyLevel
│       ├── error.rs         # 决策错误类型
│       ├── situation.rs     # DecisionSituation trait + ChoiceOption, ErrorInfo
│       ├── action.rs        # DecisionAction trait + ActionResult
│       ├── output.rs        # DecisionOutput + DecisionRecord
│       └── context.rs       # DecisionContext + RunningContextCache
│
├── decision-registry/       # 注册表层（约 600 行）
│   ├── Cargo.toml           # 依赖: decision-core
│   └── src/
│       ├── lib.rs
│       ├── action_registry.rs
│       ├── situation_registry.rs
│       ├── classifier_registry.rs
│       └── maker_registry.rs
│
├── decision-classifier/     # 分类器层（约 1,200 行）
│   ├── Cargo.toml           # 依赖: decision-core, decision-registry
│   └── src/
│       ├── lib.rs
│       ├── classifier.rs    # OutputClassifier trait
│       ├── acp_classifier.rs
│       ├── claude_classifier.rs
│       ├── codex_classifier.rs
│       ├── initializer.rs
│       ├── provider_event.rs
│       └── provider_kind.rs
│
├── decision-engine/         # 引擎层（约 2,800 行）
│   ├── Cargo.toml           # 依赖: decision-core, decision-registry
│   └── src/
│       ├── lib.rs
│       ├── engine.rs        # DecisionEngine trait
│       ├── rule_engine.rs
│       ├── llm_engine.rs
│       ├── tiered_engine.rs
│       ├── cli_engine.rs
│       ├── mock_engine.rs
│       ├── llm_caller.rs
│       └── condition.rs     # 条件系统
│
├── decision-pipeline/       # 管道层（约 1,700 行）
│   ├── Cargo.toml           # 依赖: decision-core, decision-engine, decision-registry
│   └── src/
│       ├── lib.rs
│       ├── maker.rs         # DecisionMaker trait + DecisionMakerType
│       ├── strategy.rs      # DecisionStrategy trait + 实现
│       ├── pipeline.rs      # DecisionPipeline
│       └── lifecycle.rs     # DecisionAgentState, TaskDecisionContext
│
├── decision-blocking/       # 阻塞层（约 2,900 行）
│   ├── Cargo.toml           # 依赖: decision-core
│   └── src/
│       ├── lib.rs
│       ├── blocking.rs      # BlockingReason trait + 实现
│       ├── concurrent.rs    # 异步决策处理
│       └── recovery.rs      # 错误恢复机制
│
├── decision-task/           # 任务管理层（约 3,500 行）
│   ├── Cargo.toml           # 依赖: decision-core, decision-pipeline
│   └── src/
│       ├── lib.rs
│       ├── task.rs          # Task entity + TaskStatus
│       ├── workflow.rs      # DecisionProcess, DecisionStage
│       ├── task_engine.rs   # TaskDecisionEngine
│       ├── automation.rs    # AutoChecker, DecisionFilter
│       ├── persistence.rs   # TaskRegistry, TaskStore
│       ├── task_metrics.rs
│       └── yaml_loader.rs
│
├── decision-gitflow/        # Git 流准备层（约 2,500 行）
│   ├── Cargo.toml           # 依赖: decision-task
│   └── src/
│       ├── lib.rs
│       ├── task_metadata.rs
│       ├── git_state.rs
│       ├── uncommitted_handler.rs
│       ├── task_preparation.rs
│       ├── commit_boundary.rs
│       └── task_completion.rs
│
├── decision-builtin/        # 内置实现层（约 2,400 行）
│   ├── Cargo.toml           # 依赖: 所有子包
│   └── src/
│       ├── lib.rs
│       ├── builtin_situations.rs
│       └── builtin_actions.rs
│
├── decision-metrics/        # 指标层（约 1,100 行）
│   ├── Cargo.toml           # 依赖: decision-core
│   └── src/
│       ├── lib.rs
│       └── metrics.rs
│
├── decision-prompts/        # 提示模板层（约 1,600 行）
│   ├── Cargo.toml           # 依赖: decision-core
│   └── src/
│       ├── lib.rs
│       └── mod.rs
│
└── decision/                # 统一导出包
    ├── Cargo.toml           # 依赖: 所有子包
    └── src/
        ├── lib.rs           # 重新导出所有公共类型
```

### 3.3 子包依赖关系

```text
                          ┌────────────────────┐
                          │   decision-core    │
                          │   (基础抽象层)      │
                          └────────────────────┘
                                    │
                    ┌───────────────┼───────────────┐
                    │               │               │
                    ▼               ▼               ▼
          ┌─────────────┐  ┌─────────────┐  ┌─────────────┐
          │ registry    │  │ metrics     │  │ prompts     │
          └─────────────┘  └─────────────┘  └─────────────┘
                    │
          ┌─────────┴─────────┐
          │                   │
          ▼                   ▼
    ┌───────────┐      ┌─────────────┐
    │ classifier│      │   engine    │
    └───────────┘      └─────────────┘
          │                   │
          └───────────┬───────┘
                      │
                      ▼
              ┌─────────────┐
              │   pipeline  │
              └─────────────┘
                      │
          ┌───────────┼───────────┐
          │           │           │
          ▼           ▼           ▼
    ┌──────────┐ ┌──────────┐ ┌──────────┐
    │ blocking │ │   task   │ │ builtin  │
    └──────────┘ └──────────┘ └──────────┘
                      │
                      ▼
              ┌─────────────┐
              │  gitflow    │
              └─────────────┘
                      │
                      ▼
              ┌─────────────┐
              │  decision   │
              │  (统一导出)  │
              └─────────────┘
```

---

## 4. 各子包职责说明

### 4.1 decision-core (核心抽象层)

**职责**: 提供决策层的基础抽象和类型定义，不依赖任何其他决策子包。

**核心内容**:
- `types.rs`: ActionType, SituationType, UrgencyLevel, DecisionEngineType
- `error.rs`: DecisionError
- `situation.rs`: DecisionSituation trait, ChoiceOption, ErrorInfo, CompletionProgress
- `action.rs`: DecisionAction trait, ActionResult
- `output.rs`: DecisionOutput, DecisionRecord
- `context.rs`: DecisionContext, RunningContextCache, ProjectRules

**设计原则**: 
- 纯抽象，无具体实现
- 所有 trait 定义在这里
- 可被外部 crate 直接依赖

### 4.2 decision-registry (注册表层)

**职责**: 提供情境、动作、分类器、决策者的注册和查找机制。

**核心内容**:
- `action_registry.rs`: ActionRegistry
- `situation_registry.rs`: SituationRegistry
- `classifier_registry.rs`: ClassifierRegistry
- `maker_registry.rs`: DecisionMakerRegistry

### 4.3 decision-classifier (分类器层)

**职责**: 将 Provider 输出分类为决策情境。

**核心内容**:
- `classifier.rs`: OutputClassifier trait
- `acp_classifier.rs`: ACP 输出分类器
- `claude_classifier.rs`: Claude 输出分类器
- `codex_classifier.rs`: Codex 输出分类器
- `provider_event.rs`: ProviderEvent
- `provider_kind.rs`: ProviderKind

### 4.4 decision-engine (引擎层)

**职责**: 执行具体决策逻辑的核心引擎。

**核心内容**:
- `engine.rs`: DecisionEngine trait
- `rule_engine.rs`: RuleBasedDecisionEngine (规则引擎)
- `llm_engine.rs`: LLMDecisionEngine (LLM 引擎)
- `tiered_engine.rs`: TieredDecisionEngine (分层引擎)
- `cli_engine.rs`: CLIDecisionEngine (CLI 引擎)
- `mock_engine.rs`: MockDecisionEngine (测试引擎)
- `llm_caller.rs`: LLMCaller trait
- `condition.rs`: Condition 系统

### 4.5 decision-pipeline (管道层)

**职责**: 协调决策流程，管理策略选择和 Maker 执行。

**核心内容**:
- `maker.rs`: DecisionMaker trait, DecisionMakerType, DecisionRegistries
- `strategy.rs`: DecisionStrategy trait, TieredStrategy, AdaptiveStrategy
- `pipeline.rs`: DecisionPipeline, PipelineConfig
- `lifecycle.rs`: DecisionAgentState, TaskDecisionContext

### 4.6 decision-blocking (阻塞层)

**职责**: 处理阻塞状态，管理人类决策队列和并发决策。

**核心内容**:
- `blocking.rs`: BlockingReason trait, HumanDecisionBlocking, RateLimitBlockedReason
- `concurrent.rs`: AsyncDecisionProcessor, NonBlockingDecisionProcessor
- `recovery.rs`: RecoveryStrategy

### 4.7 decision-task (任务管理层)

**职责**: 任务实体管理、工作流定义、自动化检查和持久化。

**核心内容**:
- `task.rs`: Task entity, TaskStatus, TaskId
- `workflow.rs`: DecisionProcess, DecisionStage, WorkflowAction
- `task_engine.rs`: TaskDecisionEngine
- `automation.rs`: AutoChecker, DecisionFilter
- `persistence.rs`: TaskRegistry, TaskStore, ExecutionRecord
- `task_metrics.rs`: TaskMetrics
- `yaml_loader.rs`: YAML 配置加载

### 4.8 decision-gitflow (Git 流准备层)

**职责**: Git 工作流准备和任务边界管理。

**核心内容**:
- `task_metadata.rs`: TaskMetadata, TaskType
- `git_state.rs`: GitState, GitStateAnalyzer
- `uncommitted_handler.rs`: UncommittedHandler
- `task_preparation.rs`: TaskPreparation
- `commit_boundary.rs`: CommitBoundary
- `task_completion.rs`: TaskCompletion

### 4.9 decision-builtin (内置实现层)

**职责**: 提供内置的情境和动作实现。

**核心内容**:
- `builtin_situations.rs`: WaitingForChoice, ErrorSituation, ClaimsCompletion, etc.
- `builtin_actions.rs`: ContinueAction, ReflectAction, RequestHumanAction, etc.

### 4.10 decision-metrics (指标层)

**职责**: 收集决策指标和统计信息。

**核心内容**:
- `metrics.rs`: DecisionMetrics, MetricRecord

### 4.11 decision-prompts (提示模板层)

**职责**: 提供决策提示模板。

**核心内容**:
- `mod.rs`: PromptTemplate, default_prompts

### 4.12 decision (统一导出包)

**职责**: 重新导出所有子包的公共类型，提供统一的 API。

---

## 5. 实施计划

### Phase 1: 创建核心层 (decision-core)
- 提取 types, error, situation, action, output, context
- 定义清晰的 trait 接口
- 确保无循环依赖

### Phase 2: 创建注册表层 (decision-registry)
- 提取各 Registry 实现
- 确保依赖仅指向 core

### Phase 3: 创建分类器层 (decision-classifier)
- 提取 Classifier 实现
- 提取 Provider 相关类型

### Phase 4: 创建引擎层 (decision-engine)
- 提取各 Engine 实现
- 提取 LLMCaller trait

### Phase 5: 创建管道层 (decision-pipeline)
- 提取 Maker, Strategy, Pipeline
- 提取 Lifecycle

### Phase 6: 创建阻塞层 (decision-blocking)
- 提取 Blocking 相关实现
- 提取 Concurrent 处理

### Phase 7: 创建任务管理层 (decision-task)
- 提取 Task, Workflow
- 提取 Automation, Persistence

### Phase 8: 创建 Git 流层 (decision-gitflow)
- 提取 Git 相关模块

### Phase 9: 创建内置实现层 (decision-builtin)
- 提取 builtin_situations, builtin_actions

### Phase 10: 创建辅助层 (metrics, prompts)
- 提取 metrics, prompts

### Phase 11: 创建统一导出包 (decision)
- 重新导出所有公共类型
- 更新 Cargo.toml 依赖

### Phase 12: 测试和验证
- 确保所有测试通过
- 更新外部依赖

---

## 6. 预期收益

1. **架构清晰**: 每个子包职责单一，层次分明
2. **依赖简化**: 依赖关系单向，易于理解
3. **易于维护**: 新功能可在对应子包添加
4. **便于测试**: 子包可独立测试
5. **编译优化**: 只需重新编译修改的子包
6. **复用性强**: core 层可被外部项目使用
