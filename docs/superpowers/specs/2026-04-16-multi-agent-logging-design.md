# Multi-Agent System Logging Design

## Metadata

- Date: `2026-04-16`
- Project: `agile-agent`
- Status: `draft`
- Language: `English/Chinese`
- Extends: `2026-04-13-debug-logging-and-observability-design.md`

## 概述

本文档扩展现有的日志设计，增加 Multi-Agent 系统相关的详细日志记录。包括 Agent 池管理、Agent Slot 状态转换、Backlog/Kanban 任务状态变化、Agent 间通信等所有关键流程的日志。

**重要：本系统日志主要用于 debug 辅助用途，需要尽可能详细地记录所有状态变化和关键流程。**

## 设计原则

- **详细优先**：日志详细程度优先于日志最小化，便于问题诊断
- **Debug 级别为主**：大部分日志使用 debug 级别，只有在出现错误或警告时才使用 warn/error
- **状态变更必须带原因**：所有状态转换必须记录转换原因
- **完整的上下文链**：每次日志包含足够的上下文信息（agent_id, task_id, workspace_id, from/to status 等）
- **非阻塞**：日志记录失败不影响主流程
- **可追溯**：任何操作都可以通过日志追踪其完整生命周期

## 1. Agent Pool 日志

### 1.1 Agent 创建与销毁

| 事件名 | 时机 | 记录内容 |
|--------|------|----------|
| `pool.agent.spawn` | spawn_agent() 成功 | agent_id, codename, provider_type, role, pool_size, max_slots |
| `pool.agent.spawn.failed` | spawn_agent() 失败 | reason, pool_size, max_slots |
| `pool.agent.spawn_overview` | spawn_overview_agent() 成功 | agent_id, codename, provider_type |
| `pool.agent.stop` | stop_agent() 成功 | agent_id, reason, slot_index |
| `pool.agent.remove` | remove_agent() 成功 | agent_id, pool_size_after |
| `pool.agent.remove.failed` | remove_agent() 失败 | agent_id, reason |

### 1.2 Focus 管理

| 事件名 | 时机 | 记录内容 |
|--------|------|----------|
| `pool.focus.change` | focus_agent_by_index() 成功 | old_index, new_index, old_agent_id, new_agent_id |
| `pool.focus.change.by_id` | focus_agent() 成功 | old_agent_id, new_agent_id, new_index |
| `pool.focus.invalid_index` | focus_agent_by_index() 失败 | attempted_index, pool_size |
| `pool.focus.invalid_id` | focus_agent() 失败 | attempted_id, available_ids |

### 1.3 Task 分配

| 事件名 | 时机 | 记录内容 |
|--------|------|----------|
| `pool.task.assign` | assign_task_with_backlog() 成功 | agent_id, task_id, backlog_status_before, backlog_status_after |
| `pool.task.assign.failed` | assign_task_with_backlog() 失败 | agent_id, task_id, reason |
| `pool.task.auto_assign` | auto_assign_task() 成功 | agent_id, task_id |
| `pool.task.auto_assign.no_idle_agent` | auto_assign_task() 无空闲 agent | available_agents_count, ready_tasks_count |
| `pool.task.auto_assign.no_ready_task` | auto_assign_task() 无就绪任务 | available_agents_count, ready_tasks_count |
| `pool.task.complete` | complete_task_with_backlog() 成功 | agent_id, task_id, result (Success/Failure), backlog_status |

## 2. Agent Slot 状态转换日志

### 2.1 状态转换

每次状态转换记录以下信息：

```
transition: {
    agent_id: String,
    codename: String,
    from_status: String,
    to_status: String,
    reason: String,          // 转换原因
    trigger: String,         // trigger: "user" / "agent" / "system" / "provider"
    timestamp_ms: u64,
    task_id: Option<String>, // 如果与任务相关
}
```

| 事件名 | 时机 | 记录内容 |
|--------|------|----------|
| `slot.status.transition` | transition_to() 成功 | agent_id, codename, from_status, to_status, reason, trigger, task_id |
| `slot.status.transition.invalid` | transition_to() 失败 | agent_id, from_status, attempted_status, reason |

### 2.2 详细状态标签

每个状态及其可能的原因：

| 状态 | 可能的原因示例 |
|------|--------------|
| `Idle` | "initial", "task_completed", "user_resumed", "recovery" |
| `Starting` | "spawning", "restarting" |
| `Responding` | "processing_prompt", "thinking" |
| `ToolExecuting` | "tool_name: bash", "tool_name: read_file", etc. |
| `Finishing` | "response_complete", "interrupted" |
| `Stopping` | "user_requested", "provider_shutdown" |
| `Stopped` | "user_requested", "task_complete", "idle_timeout" |
| `Error` | "panic", "channel_closed", "provider_crash", "timeout" |
| `Blocked` | "awaiting_user_input", "api_design_unconfirmed", "dependency_not_ready", "resource_unavailable" |

### 2.3 Provider Thread 管理

| 事件名 | 时机 | 记录内容 |
|--------|------|----------|
| `slot.thread.set` | set_provider_thread() | agent_id, has_event_rx, has_thread_handle |
| `slot.thread.clear` | clear_provider_thread() | agent_id |
| `slot.thread.take` | take_thread_handle() | agent_id |
| `slot.session.handle.set` | set_session_handle() | agent_id, handle_id |
| `slot.session.handle.clear` | clear_session_handle() | agent_id |

## 3. Backlog / Kanban 日志

### 3.1 任务状态变化

| 事件名 | 时机 | 记录内容 |
|--------|------|----------|
| `backlog.task.create` | push_task() | task_id, todo_id, objective, initial_status |
| `backlog.task.start` | start_task() | task_id, old_status, new_status |
| `backlog.task.complete` | complete_task() | task_id, old_status, new_status, summary, completed_by (agent_id/user/system) |
| `backlog.task.fail` | fail_task() | task_id, old_status, new_status, error, failed_by |
| `backlog.task.block` | block_task() | task_id, old_status, new_status, reason, blocked_by |
| `backlog.task.status_change` | 任何状态变化 | task_id, old_status, new_status, changed_by, reason |

### 3.2 Todo 状态变化

| 事件名 | 时机 | 记录内容 |
|--------|------|----------|
| `backlog.todo.create` | push_todo() | todo_id, title, priority, initial_status |
| `backlog.todo.status_change` | 任何状态变化 | todo_id, old_status, new_status, changed_by |
| `backlog.todo.priority_change` | priority 修改 | todo_id, old_priority, new_priority |

### 3.3 Backlog 统计

| 事件名 | 时机 | 记录内容 |
|--------|------|----------|
| `backlog.snapshot` | task_queue_snapshot() | total_tasks, ready_tasks, running_tasks, completed_tasks, failed_tasks, blocked_tasks, available_agents, active_agents |

## 4. Multi-Agent Session 日志

### 4.1 Agent 间通信

| 事件名 | 时机 | 记录内容 |
|--------|------|----------|
| `session.agent.message` | Agent 发送消息 | from_agent_id, to_agent_id, message_type, message_preview (前100字符) |
| `session.agent.broadcast` | 广播消息 | from_agent_id, recipient_count, message_type |
| `session.agent.reply` | 回复消息 | from_agent_id, to_agent_id, in_response_to_message_id |

### 4.2 @ 指令路由

| 事件名 | 时机 | 记录内容 |
|--------|------|----------|
| `session.route.parse` | 解析 @ 指令 | raw_input, parsed_targets (Vec<agent_id>), routing_decision, final_recipient |
| `session.route.direct` | 直接发送给焦点 Agent | agent_id, prompt_preview |
| `session.route.broadcast` | 广播给多个 Agent | agent_ids, prompt_preview |
| `session.route.no_target` | 无有效目标 | raw_input, error |
| `session.route.target_not_found` | 目标不存在 | requested_target, available_agents |

### 4.3 Event Aggregator

| 事件名 | 时机 | 记录内容 |
|--------|------|----------|
| `aggregator.channel.register` | 注册事件通道 | agent_id, total_channels |
| `aggregator.channel.unregister` | 注销事件通道 | agent_id, remaining_channels |
| `aggregator.channel.empty` | 通道为空（无事件） | agent_id |
| `aggregator.event.dispatch` | 分发事件 | agent_id, event_type, queue_depth |
| `aggregator.disconnected` | 通道断开连接 | agent_id, disconnect_reason |

## 5. Task Engine 扩展日志

### 5.1 任务生命周期

| 事件名 | 时机 | 记录内容 |
|--------|------|----------|
| `task.engine.turn.resolve` | TurnResolution 决策 | task_id, decision (Continue/Complete/Failed/Escalated/Idle), reason, next_action_preview |
| `task.engine.verification.start` | 开始验证 | task_id, verification_plan_preview |
| `task.engine.verification.complete` | 验证完成 | task_id, passed, findings |
| `task.engine.escalation.trigger` | 触发升级 | task_id, reason, escalated_to |
| `task.engine.guardrail.check` | 检查 guardrail | guardrail_type, passed, remaining_iterations |

## 6. 日志级别

**重要：绝大多数日志使用 `debug` 级别。**

| 级别 | 使用场景 |
|------|----------|
| `debug` | 所有状态转换、流程分支、函数入口/出口、变量值变化 |
| `info` | 关键里程碑（如 Agent 创建完成、任务完成）、用户可见的状态变化 |
| `warn` | 非预期但可恢复的状态（如重试、fallback） |
| `error` | 操作失败、异常状态、panic |

## 7. 日志字段标准

### 6.3 示例日志条目

```json
{
  "ts": "2026-04-16T10:32:15.123Z",
  "level": "debug",
  "event": "slot.status.transition",
  "message": "alpha transitioned from Responding to Blocked",
  "run_id": "run-2026-04-16T10-30-00.123Z-tui-pid1234-1",
  "workplace_id": "wp-abc123",
  "agent_id": "agent_001",
  "codename": "alpha",
  "from_status": "responding",
  "to_status": "blocked",
  "reason": "api_design_unconfirmed",
  "trigger": "agent",
  "fields": {
    "task_id": "task-042",
    "blocked_reason_detail": "Waiting for API design review from user"
  }
}
```

```json
{
  "ts": "2026-04-16T10:32:18.456Z",
  "level": "debug",
  "event": "backlog.task.complete",
  "message": "Task task-042 completed successfully",
  "run_id": "run-2026-04-16T10-30-00.123Z-tui-pid1234-1",
  "workplace_id": "wp-abc123",
  "task_id": "task-042",
  "fields": {
    "old_status": "running",
    "new_status": "done",
    "completed_by": "agent_001",
    "summary": "API client implementation complete, 15 files changed"
  }
}
```

```json
{
  "ts": "2026-04-16T10:32:20.789Z",
  "level": "debug",
  "event": "session.route.broadcast",
  "message": "Broadcasting message to 2 agents",
  "run_id": "run-2026-04-16T10-30-00.123Z-tui-pid1234-1",
  "workplace_id": "wp-abc123",
  "fields": {
    "raw_input": "@alpha @bravo 你们协作一下这个任务",
    "targets": ["agent_001", "agent_002"],
    "prompt_preview": "你们协作一下这个任务..."
  }
}
```

## 7. 实现清单

### 7.1 AgentPool 日志注入点

- [ ] pool.spawn_agent() - 成功和失败
- [ ] pool.spawn_overview_agent() - 成功
- [ ] pool.stop_agent() - 成功
- [ ] pool.remove_agent() - 成功和失败
- [ ] pool.focus_agent_by_index() - 成功和失败
- [ ] pool.focus_agent() - 成功和失败
- [ ] pool.assign_task() / assign_task_with_backlog()
- [ ] pool.complete_task_with_backlog()
- [ ] pool.auto_assign_task()

### 7.2 AgentSlot 日志注入点

- [ ] transition_to() - 成功和失败
- [ ] assign_task() - 成功和失败
- [ ] clear_task()
- [ ] set_provider_thread()
- [ ] clear_provider_thread()
- [ ] take_thread_handle()
- [ ] set_session_handle()
- [ ] clear_session_handle()

### 7.3 Backlog 日志注入点

- [ ] push_task() / BacklogState.push_task()
- [ ] start_task()
- [ ] complete_task()
- [ ] fail_task()
- [ ] block_task()
- [ ] push_todo()
- [ ] find_task_mut() 中的状态变更
- [ ] task_queue_snapshot()

### 7.4 Multi-Agent Session 日志注入点

- [ ] EventAggregator.add_receiver()
- [ ] EventAggregator.remove_receiver()
- [ ] EventAggregator.poll_all()
- [ ] Agent 间消息发送
- [ ] @ 指令解析和路由

### 7.5 TUI 路由日志

- [ ] @ 指令解析
- [ ] 直接发送路由
- [ ] 广播路由
- [ ] 目标未找到

## 8. 与现有设计的关系

本文档扩展 `2026-04-13-debug-logging-and-observability-design.md`，新增：

1. Agent Pool 管理日志
2. Agent Slot 状态转换详细日志
3. Backlog/Kanban 任务状态变化日志
4. Multi-Agent Session 和 EventAggregator 日志
5. @ 指令路由日志

现有设计中的以下模块保持不变：
- 基础日志架构（core/src/logging.rs）
- 启动和环境日志
- Provider 通信日志（Claude/Codex）
- TUI 控制流日志

## 9. 测试策略

### 9.1 单元测试

每个模块添加日志后，验证：

- [ ] 日志事件名称正确
- [ ] 必需字段存在且格式正确
- [ ] 可选字段在适用时正确填充
- [ ] 状态转换包含 from/to 状态
- [ ] 错误日志包含错误详情

### 9.2 集成测试

- [ ] 多 Agent 场景的完整日志链
- [ ] 任务从创建到完成的完整日志追踪
- [ ] @ 指令路由的完整日志追踪
- [ ] 状态转换和 Backlog 状态变化的关联

### 9.3 日志验证测试

- [ ] 验证日志文件包含预期事件序列
- [ ] 验证 JSONL 格式正确
- [ ] 验证日志可以被解析和查询
