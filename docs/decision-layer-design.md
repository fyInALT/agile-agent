# 决策层思考流程设计

> 本文档梳理当前决策层的架构设计和思考流程，为后续DSL(领域特定语言)设计提供基础参考。

## 1. 整体架构

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                           Decision Layer Architecture                          │
│                                                                                │
│  Core → Model → Pipeline → Engine → Classifier → Provider → State → Runtime  │
│                                                                                │
│  执行流程:                                                                      │
│  Provider Output → Classifier → Situation → Context → Engine → Output         │
└─────────────────────────────────────────────────────────────────────────────┘
```

### 核心层次

| 层次 | 模块 | 职责 |
|------|------|------|
| **Core** | `context.rs`, `output.rs`, `error.rs`, `types.rs` | 基础类型定义 |
| **Model** | `situation/`, `action/`, `task/` | 业务模型定义 |
| **Pipeline** | `pipeline.rs`, `maker.rs`, `strategy.rs` | 执行流程编排 |
| **Engine** | `rule_engine.rs`, `llm_engine.rs`, `tiered_engine.rs` | 决策执行引擎 |
| **Classifier** | 输出分类器 | Provider输出 → Situation识别 |
| **Condition** | `condition.rs` | 规则条件表达式系统 |

---

## 2. 核心数据结构

### 2.1 DecisionContext (决策输入)

```rust
pub struct DecisionContext {
    /// 触发情境 (trait object, 支持动态扩展)
    pub trigger_situation: Box<dyn DecisionSituation>,
    
    /// 工作Agent ID
    pub main_agent_id: String,
    
    /// 当前任务/故事ID
    pub current_task_id: Option<String>,
    pub current_story_id: Option<String>,
    
    /// 运行上下文缓存 (执行历史, 有大小限制)
    pub running_context: RunningContextCache,
    
    /// 项目规则 (从CLAUDE.md等提取)
    pub project_rules: ProjectRules,
    
    /// 决策历史记录
    pub decision_history: Vec<DecisionRecord>,
    
    /// 元数据 (用于engine状态同步)
    pub metadata: HashMap<String, String>,  // 如: reflection_round
}
```

#### RunningContextCache 结构

```rust
pub struct RunningContextCache {
    /// 工具调用记录 (max N entries)
    pub tool_calls: VecDeque<ToolCallRecord>,
    
    /// 文件变更记录 (max N entries)
    pub file_changes: VecDeque<FileChangeRecord>,
    
    /// 思维摘要 (滚动更新)
    pub thinking_summary: Option<String>,
    
    /// 关键输出 (max N entries)
    pub key_outputs: VecDeque<String>,
}
```

### 2.2 DecisionOutput (决策输出)

```rust
pub struct DecisionOutput {
    /// 动作序列
    pub actions: Vec<Box<dyn DecisionAction>>,
    
    /// 推理过程
    pub reasoning: String,
    
    /// 置信度 (0.0-1.0)
    pub confidence: f64,
    
    /// 是否请求人工介入
    pub human_requested: bool,
    
    /// 反思轮次更新 (用于claims_completion状态同步)
    pub updated_reflection_round: Option<u8>,
}
```

---

## 3. Decision执行流程

```
┌─────────────────────────────────────────────────────────────────────┐
│                     Decision Pipeline Flow                           │
│                                                                       │
│  INPUT: Provider Output → Classifier → Situation                     │
│                                                                       │
│  ┌─────────────────────────────────────────────────────────────┐    │
│  │  1. PRE-PROCESSORS                                           │    │
│  │     - enrich context                                         │    │
│  │     - validate metadata                                      │    │
│  │     - sync reflection_round                                  │    │
│  └─────────────────────────────────────────────────────────────┘    │
│                          ↓                                           │
│  ┌─────────────────────────────────────────────────────────────┐    │
│  │  2. TIER SELECTION (TieredDecisionEngine)                   │    │
│  │     - Simple → RuleBased                                     │    │
│  │     - Medium/Complex → LLM                                   │    │
│  │     - Critical → CLI (Human)                                 │    │
│  └─────────────────────────────────────────────────────────────┘    │
│                          ↓                                           │
│  ┌─────────────────────────────────────────────────────────────┐    │
│  │  3. ENGINE EXECUTION                                         │    │
│  │     - build_prompt()                                         │    │
│  │     - call_llm() / rule_match()                              │    │
│  │     - parse_response()                                       │    │
│  │     - extract reasoning & confidence                         │    │
│  └─────────────────────────────────────────────────────────────┘    │
│                          ↓                                           │
│  ┌─────────────────────────────────────────────────────────────┐    │
│  │  4. POST-PROCESSORS                                          │    │
│  │     - validate actions                                       │    │
│  │     - confidence threshold check                             │    │
│  │     - human escalation override                              │    │
│  └─────────────────────────────────────────────────────────────┘    │
│                          ↓                                           │
│  OUTPUT: DecisionOutput (actions, reasoning, confidence)             │
│                                                                       │
└─────────────────────────────────────────────────────────────────────┘
```

---

## 4. Situation类型系统

### 4.1 DecisionSituation Trait

```rust
pub trait DecisionSituation: Send + Sync + 'static {
    /// 情境类型标识
    fn situation_type(&self) -> SituationType;
    
    /// 实现类型名 (用于调试)
    fn implementation_type(&self) -> &'static str;
    
    /// 是否需要人工介入
    fn requires_human(&self) -> bool;
    
    /// 人工介入紧迫度
    fn human_urgency(&self) -> UrgencyLevel;
    
    /// 转换为Prompt文本
    fn to_prompt_text(&self) -> String;
    
    /// 可用动作列表
    fn available_actions(&self) -> Vec<ActionType>;
    
    /// 错误信息 (如果是错误情境)
    fn error_info(&self) -> Option<&ErrorInfo>;
}
```

### 4.2 内置Situation类型

| Situation Type | Tier | 描述 | 可用Actions |
|----------------|------|------|-------------|
| `waiting_for_choice` | Simple | Agent等待选择确认 | `select_option`, `select_first` |
| `claims_completion` | Medium | Agent声称任务完成 | `reflect`, `confirm_completion` |
| `error` | Complex/Simple | 错误发生 (429用Simple避免死锁) | `retry`, `request_human` |
| `partial_completion` | Complex | 部分完成 | `continue`, `reflect` |
| `agent_idle` | Simple | Agent空闲 | `continue_all_tasks`, `stop_if_complete` |
| `task_starting` | High | 新任务开始 | `prepare_task_start` |
| `rate_limit_recovery` | Simple | Rate限制恢复 | `retry` |

---

## 5. 分层决策引擎 (TieredDecisionEngine)

### 5.1 Tier选择逻辑

```rust
pub fn from_situation(situation: &dyn DecisionSituation) -> DecisionTier {
    // Critical: 人工介入标记
    if situation.requires_human() {
        return DecisionTier::Critical;
    }
    
    let type_name = situation.situation_type().name;
    
    // Complex: 错误恢复(非429)、部分完成
    if type_name == "error" {
        // 429/rate_limit → Simple (避免LLM调用死锁)
        if is_rate_limit_error() {
            return DecisionTier::Simple;
        }
        return DecisionTier::Complex;
    }
    
    if type_name == "partial_completion" {
        return DecisionTier::Complex;
    }
    
    // Medium: claims_completion需要验证
    if type_name == "claims_completion" {
        return DecisionTier::Medium;
    }
    
    // Simple: well-known patterns
    if type_name == "waiting_for_choice" || type_name == "agent_idle" {
        return DecisionTier::Simple;
    }
    
    // Default: Medium
    DecisionTier::Medium
}
```

### 5.2 Tier层级说明

| Tier | 引擎 | 适用场景 | 特点 |
|------|------|----------|------|
| **Simple (1)** | RuleBased | 确定性情境 | 无LLM调用, 规则匹配 |
| **Medium (2)** | LLM | 需要判断的情境 | LLM决策, 适中复杂度 |
| **Complex (3)** | LLM | 错误恢复/复杂判断 | LLM决策, 更多上下文 |
| **Critical (4)** | CLI/Human | 必须人工介入 | 用户交互 |

---

## 6. RuleBased引擎 (规则引擎)

### 6.1 规则结构定义

```rust
pub struct DecisionRule {
    /// 规则名称
    pub name: String,
    
    /// 条件表达式
    pub condition: ConditionExpr,
    
    /// 动作规格
    pub actions: Vec<ActionSpec>,
    
    /// 优先级 (Critical > High > Medium > Low)
    pub priority: RulePriority,
}

pub struct ActionSpec {
    /// 动作类型名
    pub type_name: String,
    
    /// 动作参数
    pub params: HashMap<String, String>,
}
```

### 6.2 内置规则表

| Rule Name | Condition | Actions | Priority |
|-----------|-----------|---------|----------|
| `approve-first` | `situation_type("waiting_for_choice")` | `select_first` | Medium |
| `reflect-first` | `situation_type("claims_completion") AND reflection_rounds(0,1)` | `reflect` | High |
| `confirm-when-max-reflections` | `situation_type("claims_completion") AND reflection_rounds(2,10)` | `confirm_completion` | High |
| `retry-error` | `situation_type("error")` | `retry` | Medium |
| `retry-rate-limit` | `situation_type("rate_limit_recovery")` | `retry` | Medium |
| `continue-on-idle` | `situation_type("agent_idle")` | `continue_all_tasks` | Medium |
| `prepare-task-start` | `situation_type("task_starting")` | `prepare_task_start` | High |

### 6.3 规则匹配流程

```
1. 合并自定义规则 + 内置规则
2. 按优先级排序 (Critical > High > Medium > Low)
3. 遍历规则, 评估ConditionExpr
4. 返回第一个匹配的规则
5. 无匹配 → 默认动作 (custom_instruction)
```

---

## 7. Condition表达式系统 (DSL雏形)

### 7.1 表达式类型

```rust
pub enum ConditionExpr {
    /// 单一条件
    Single(Condition),
    
    /// AND组合 - 全部匹配
    And(Vec<ConditionExpr>),
    
    /// OR组合 - 任一匹配
    Or(Vec<ConditionExpr>),
    
    /// NOT - 取反
    Not(Box<ConditionExpr>),
}
```

### 7.2 Condition类型

```rust
pub enum Condition {
    /// 情境类型匹配
    SituationType { type_name: String },
    
    /// 项目规则关键词存在
    ProjectKeyword { keyword: String },
    
    /// 反思轮次范围
    ReflectionRounds { min: u8, max: u8 },
    
    /// 置信度低于阈值
    ConfidenceBelow { threshold: f64 },
    
    /// 距上次动作的时间范围
    TimeSinceLastAction { min_seconds: u64, max_seconds: Option<u64> },
    
    /// 自定义条件 (可扩展)
    Custom { name: String, params: HashMap<String, String> },
}
```

### 7.3 使用示例

```rust
// claims_completion第一轮 → reflect
ConditionExpr::and(vec![
    ConditionExpr::single(Condition::situation_type("claims_completion")),
    ConditionExpr::single(Condition::reflection_rounds(0, 1)),
])

// 达到最大反思轮次 → confirm_completion
ConditionExpr::and(vec![
    ConditionExpr::single(Condition::situation_type("claims_completion")),
    ConditionExpr::single(Condition::reflection_rounds(2, 10)),
])

// 非错误情境
ConditionExpr::not(ConditionExpr::single(Condition::situation_type("error")))

// 项目关键词匹配
Condition::project_keyword("TDD")
```

### 7.4 自定义ConditionEvaluator

```rust
pub trait ConditionEvaluator: Send + Sync {
    fn evaluate(&self, context: &DecisionContext, params: &HashMap<String, String>) -> bool;
}

// 注册自定义评估器
registry.register("custom_check", Box::new(MyCustomEvaluator));
```

---

## 8. LLM引擎 (LLMDecisionEngine)

### 8.1 执行流程

```
1. sync_reflection_round_from_context()  // 从metadata同步
2. merge_decision_history_from_context()  // 合并历史记录
3. build_prompt_internal():
   - 有PromptBuilder → build_prompt_with_builder()
   - 无PromptBuilder → build_prompt_legacy()
4. call_llm_with_retry()                  // 调用LLM (带重试)
5. parse_response_internal()              // 解析响应
6. extract_reasoning() & extract_confidence()
7. 状态管理:
   - reflect → increment_reflection_round()
   - confirm_completion → reset_reflection_round()
   - 达到max → 强制request_human
```

### 8.2 Prompt模板结构

```text
You are a decision helper for a development agent.

## Current Situation
{situation_text}

## Available Actions
{action_formats}

## Project Rules
{project_rules_summary}

## Current Task
{task_info}

## Running Context Summary
{running_context_summary}

## Decision History (Recent)
{recent_history}

## Instructions
Select exactly one action from the Available Actions above.

## Output Format
ACTION: <action_type>
PARAMETERS: <json parameters if applicable>
REASONING: <brief explanation>
CONFIDENCE: <number between 0.0 and 1.0>
```

### 8.3 反思机制状态管理

```
claims_completion流程:
  Round 0 → reflect (increment to 1)
  Round 1 → reflect (increment to 2)
  Round 2 → confirm_completion OR request_human (达到max_reflection_rounds)

状态变化规则:
  ON reflect:            reflection_round += 1
  ON confirm_completion: reflection_round = 0 (reset)
  ON continue:           reflection_round = 0 (reset)
  ON 达到max:            强制 request_human
```

---

## 9. Action类型系统

### 9.1 DecisionAction Trait

```rust
pub trait DecisionAction: Send + Sync + 'static {
    /// 动作类型
    fn action_type(&self) -> ActionType;
    
    /// 实现类型名 (调试)
    fn implementation_type(&self) -> &'static str;
    
    /// Prompt格式文本
    fn to_prompt_format(&self) -> String;
    
    /// 序列化参数
    fn serialize_params(&self) -> String;
    
    /// 克隆为boxed
    fn clone_boxed(&self) -> Box<dyn DecisionAction>;
}
```

### 9.2 内置Action类型

| Action Type | 用途 | 关键参数 |
|-------------|------|----------|
| `select_option` | 选择选项 | `option_id`, `reason` |
| `select_first` | 选择第一个选项 | 无 |
| `reflect` | 反思验证 | `prompt` |
| `confirm_completion` | 确认完成 | `submit_pr`, `next_task_id` |
| `continue` | 继续执行 | `prompt`, `focus_items` |
| `retry` | 重试 | `prompt`, `cooldown_ms`, `adjusted` |
| `request_human` | 请求人工介入 | `message` |
| `custom_instruction` | 自定义指令 | `instruction` |
| `continue_all_tasks` | 继续所有任务 | `instruction` |
| `stop_if_complete` | 停止(完成) | `reason` |
| `prepare_task_start` | 准备任务开始 | `task_meta`, `pre_actions` |
| `commit_changes` | 提交变更 | `commit_message`, `is_wip` |
| `stash_changes` | 暂存变更 | `description` |
| `discard_changes` | 丢弃变更 | 无 |
| `create_task_branch` | 创建任务分支 | `branch_name`, `base_branch` |
| `rebase_to_main` | Rebase到主分支 | `base_branch` |

---

## 10. Pre/Post Processor系统

### 10.1 Pre-Processor

```rust
pub trait DecisionPreProcessor {
    fn process(&self, context: &mut DecisionContext) -> Result<()>;
    fn processor_name(&self) -> &'static str;
    fn clone_boxed(&self) -> Box<dyn DecisionPreProcessor>;
}
```

**内置Pre-Processor:**
- `ReflectionRoundPreProcessor` - 同步reflection_round到metadata

### 10.2 Post-Processor

```rust
pub trait DecisionPostProcessor {
    fn process(&self, output: &mut DecisionOutput) -> Result<()>;
    fn processor_name(&self) -> &'static str;
    fn clone_boxed(&self) -> Box<dyn DecisionPostProcessor>;
}
```

**内置Post-Processor:**
- `ValidateActionsPostProcessor` - 确保output有至少一个action
- `ConfidenceThresholdPostProcessor` - 置信度阈值检查

---

## 11. DSL设计方向

基于当前架构，未来DSL可以覆盖以下能力：

### 11.1 Situation定义

```dsl
SITUATION claims_completion {
  requires_human: false
  tier: Medium
  available_actions: [reflect, confirm_completion, continue]
  
  // 可扩展自定义condition
  custom_conditions: {
    has_uncommitted_changes: check_git_status,
    tests_failing: check_test_result
  }
}
```

### 11.2 Rule定义

```dsl
RULE reflect_first {
  WHEN situation == "claims_completion"
       AND reflection_round IN [0, 1]
  THEN reflect WITH {
    prompt: "Verify task completion: check tests, review changes"
  }
  PRIORITY High
  TIER Simple  // 可覆盖默认tier
}

RULE confirm_after_verification {
  WHEN situation == "claims_completion"
       AND reflection_round >= 2
       AND confidence >= 0.8
       AND project_keyword("TDD")  // 检查测试
  THEN confirm_completion WITH {
    submit_pr: true,
    next_task_id: auto_select
  }
  PRIORITY High
}
```

### 11.3 状态管理

```dsl
STATE reflection_round {
  INITIAL: 0
  ON reflect: increment
  ON confirm_completion: reset
  ON continue: reset
  MAX: 2
  ON_MAX: request_human  // 超过max时强制人工
}

STATE confidence_accumulator {
  // 累积置信度, 用于复杂决策
  INITIAL: 0.0
  ON each_decision: accumulate(weight: 0.3)
  THRESHOLD: 0.7
}
```

### 11.4 流程编排

```dsl
FLOW task_completion {
  TRIGGER: claims_completion
  
  STEP 1: reflect
    REPEAT max: 2 rounds
    ON success: continue to STEP 2
    ON error: retry max: 3
    
  STEP 2: verify_completion
    ACTIONS: [check_tests, check_git_status]
    ON pass: confirm_completion
    ON fail: custom_instruction("Fix remaining issues")
    
  FALLBACK: request_human WITH {
    message: "Unable to verify completion after {reflection_round} rounds"
  }
}
```

### 11.5 Pipeline配置

```dsl
PIPELINE default {
  PRE_PROCESSORS: [
    sync_reflection_round,
    load_project_rules,
    compress_running_context(max_bytes: 10240)
  ]
  
  POST_PROCESSORS: [
    validate_actions,
    confidence_threshold(min: 0.5),
    human_override_for: ["submit_pr", "delete_files"]
  ]
  
  TIMEOUT: 30000ms
  MAX_HISTORY: 100
  FALLBACK_TIER: Medium
}
```

---

## 12. 关键设计要点总结

### 12.1 架构优势

| 设计要点 | 说明 |
|----------|------|
| **分层决策** | TieredEngine根据复杂度自动选择RuleBased/LLM/CLI |
| **规则优先** | Simple情境用规则匹配, 避免LLM开销和调用延迟 |
| **死锁避免** | 429错误用Simple tier, 避免LLM调用导致的死锁 |
| **反思机制** | claims_completion有reflection_round状态验证流程 |
| **人工兜底** | Critical tier + max_reflection_rounds强制人工介入 |

### 12.2 可扩展性

| 扩展点 | 方式 |
|--------|------|
| **自定义Condition** | 实现`ConditionEvaluator` trait |
| **自定义Action** | 实现`DecisionAction` trait |
| **自定义Situation** | 实现`DecisionSituation` trait |
| **自定义Prompt** | 使用`PromptBuilder`配置 |
| **自定义Processor** | 实现`DecisionPreProcessor`/`DecisionPostProcessor` |

### 12.3 状态同步机制

```
DecisionAgentState (Core)          TieredDecisionEngine (Decision)
        │                                    │
        │  reflection_round                  │  reflection_round
        │  decision_history                  │  history
        │                                    │
        └──────────── metadata ──────────────┘
                     
同步时机:
- 请求发送前: 从State提取到Context.metadata
- 响应返回后: 从Output.updated_reflection_round同步回State
```

---

## 13. 文件结构参考

```
decision/src/
├── core/                     # 基础类型层
│   ├── context.rs            # DecisionContext, RunningContextCache
│   ├── output.rs             # DecisionOutput, DecisionRecord
│   ├── error.rs              # DecisionError
│   └── types.rs              # ActionType, SituationType, DecisionEngineType
│
├── model/                    # 业务模型层
│   ├── situation/            # Situation子系统
│   │   ├── situation.rs      # DecisionSituation trait
│   │   ├── builtin_situations.rs  # 内置Situation实现
│   │   └── situation_registry.rs  # Situation注册器
│   ├── action/               # Action子系统
│   │   ├── action.rs         # DecisionAction trait
│   │   ├── action_registry.rs     # Action注册器
│   │   └── builtin_actions.rs     # 内置Action实现
│   └── task/                 # Task子系统
│
├── engine/                   # 引擎层
│   ├── engine.rs             # DecisionEngine trait
│   ├── rule_engine.rs        # RuleBased引擎
│   ├── llm_engine.rs         # LLM引擎
│   ├── tiered_engine.rs      # 分层决策引擎
│   ├── cli_engine.rs         # CLI引擎 (人工决策)
│   ├── mock_engine.rs        # Mock引擎 (测试)
│   └── llm_caller.rs         # LLMCaller trait
│
├── pipeline/                 # 流程编排层
│   ├── pipeline.rs           # DecisionPipeline
│   ├── maker.rs              # DecisionMaker trait
│   ├── strategy.rs           # 策略选择
│   └── maker_registry.rs     # Maker注册器
│
├── condition.rs              # 条件表达式系统
├── config/                   # 配置层
│   └── prompts.rs            # PromptBuilder配置
│
└── lib.rs                    # 模块入口
```

---

## 14. 后续工作方向

1. **DSL语法设计** - 定义完整的DSL语法规范
2. **DSL解析器** - 实现DSL文本到内部结构的解析
3. **可视化工具** - 决策流程可视化编辑器
4. **热加载** - 支持DSL规则动态更新
5. **测试框架** - DSL规则的单元测试支持

---

> 文档版本: v1.0
> 最后更新: 2026-04-23
> 相关模块: `agent-decision`, `agent-core/decision_agent_slot`
