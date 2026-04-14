# Test-Driven Development Specification

## Overview

This document defines the test-driven development approach for the decision layer. Every implementation task must have corresponding test tasks defined upfront.

## TDD Philosophy

1. **Write test first**: Before implementing any feature, define the test case
2. **Make test fail**: Verify the test correctly captures the requirement
3. **Implement minimum**: Write just enough code to pass the test
4. **Refactor**: Clean up implementation while keeping tests passing

## Test Categories

| Category | Purpose | Location |
|----------|---------|----------|
| **Unit Tests** | Single function/struct behavior | `decision/src/**/*.rs` inline |
| **Integration Tests** | Multi-component interaction | `decision/tests/` |
| **Mock Tests** | Simulated provider scenarios | `decision/tests/mock/` |
| **Sample Tests** | Real provider output validation | `decision/tests/samples/` |

---

## Sprint 1: Core Types - Test Tasks

### Story 1.1: ProviderStatus and ProviderOutputType

| Test ID | Test Name | Priority | Definition |
|---------|-----------|----------|------------|
| T1.1.T1 | `test_provider_output_type_running` | P0 | Verify Running variant stores event correctly |
| T1.1.T2 | `test_provider_output_type_finished` | P0 | Verify Finished variant stores status correctly |
| T1.1.T3 | `test_provider_status_waiting_for_choice` | P0 | Verify options Vec serialization |
| T1.1.T4 | `test_provider_status_claims_completion` | P0 | Verify summary and reflection_rounds fields |
| T1.1.T5 | `test_provider_status_partial_completion` | P0 | Verify progress completed/remaining items |
| T1.1.T6 | `test_provider_status_error_variants` | P0 | Verify all ErrorType variants serialize |
| T1.1.T7 | `test_choice_option_serde_roundtrip` | P0 | JSON serialize/deserialize preserves fields |

**Test Implementation Priority**: Implement T1.1.T1-T1.1.T7 BEFORE implementing types.

### Story 1.2: DecisionOutput and DecisionContext

| Test ID | Test Name | Priority | Definition |
|---------|-----------|----------|------------|
| T1.2.T1 | `test_decision_output_choice` | P0 | Choice variant with selected and reason |
| T1.2.T2 | `test_decision_output_reflection_request` | P0 | ReflectionRequest with prompt |
| T1.2.T3 | `test_decision_output_completion_confirm` | P0 | CompletionConfirm with submit_pr and next_task |
| T1.2.T4 | `test_decision_output_continue_instruction` | P0 | ContinueInstruction with focus_items |
| T1.2.T5 | `test_decision_output_retry_instruction` | P0 | RetryInstruction with cooldown_ms |
| T1.2.T6 | `test_decision_context_default` | P0 | Default DecisionContext has correct defaults |
| T1.2.T7 | `test_running_context_cache_fields` | P0 | All RunningContextCache fields present |
| T1.2.T8 | `test_tool_call_record_timestamp` | P0 | Timestamp correctly set |

### Story 1.3: DecisionAgentConfig

| Test ID | Test Name | Priority | Definition |
|---------|-----------|----------|------------|
| T1.3.T1 | `test_config_default_values` | P0 | All defaults match specification |
| T1.3.T2 | `test_config_engine_type_variants` | P0 | All engine types parse correctly |
| T1.3.T3 | `test_config_toml_parse` | P0 | TOML parsing for all fields |
| T1.3.T4 | `test_config_validation` | P1 | Invalid config rejected with error |
| T1.3.T5 | `test_creation_policy_variants` | P0 | Eager/Lazy/Configured work correctly |

### Story 1.4: Human Intervention Types

| Test ID | Test Name | Priority | Definition |
|---------|-----------|----------|------------|
| T1.4.T1 | `test_critical_criteria_evaluation` | P0 | Each criterion evaluated correctly |
| T1.4.T2 | `test_criticality_score_calculation` | P0 | Score weights match specification |
| T1.4.T3 | `test_criticality_threshold_check` | P0 | Threshold comparison works |
| T1.4.T4 | `test_human_decision_request_fields` | P0 | All request fields present |
| T1.4.T5 | `test_human_selection_variants` | P0 | All HumanSelection variants work |
| T1.4.T6 | `test_recommendation_confidence_range` | P0 | Confidence 0.0-1.0 validated |

---

## Sprint 2: Output Classifier - Test Tasks

### Story 2.1: Claude Classifier

| Test ID | Test Name | Priority | Definition |
|---------|-----------|----------|------------|
| T2.1.T1 | `test_claude_assistant_chunk_running` | P0 | AssistantChunk → Running |
| T2.1.T2 | `test_claude_thinking_chunk_running` | P0 | ThinkingChunk → Running |
| T2.1.T3 | `test_claude_tool_call_running` | P0 | GenericToolCallStarted/Finished → Running |
| T2.1.T4 | `test_claude_finished_claims_completion` | P0 | Finished event → ClaimsCompletion |
| T2.1.T5 | `test_claude_error_status` | P0 | Error event → Error status |
| T2.1.T6 | `test_claude_session_handle_info` | P0 | SessionHandle → Running (info) |
| T2.1.T7 | `test_claude_no_waiting_for_choice` | P0 | Never returns WaitingForChoice |
| T2.1.T8 | `test_claude_real_sample_1` | P1 | Real Claude output sample test |

**Sample Requirement**: Collect 5 real Claude Finished event samples for T2.1.T8.

### Story 2.2: Codex Classifier

| Test ID | Test Name | Priority | Definition |
|---------|-----------|----------|------------|
| T2.2.T1 | `test_codex_exec_command_approval` | P0 | execCommandApproval → WaitingForChoice |
| T2.2.T2 | `test_codex_apply_patch_approval` | P0 | applyPatchApproval → WaitingForChoice |
| T2.2.T3 | `test_codex_request_user_input` | P0 | requestUserInput → WaitingForChoice |
| T2.2.T4 | `test_codex_permissions_approval` | P0 | permissionsApproval → WaitingForChoice |
| T2.2.T5 | `test_codex_review_decision_options` | P0 | ReviewDecision options parsed correctly |
| T2.2.T6 | `test_codex_non_approval_running` | P0 | Non-approval requests → Running |
| T2.2.T7 | `test_codex_real_sample_1` | P1 | Real Codex approval request sample |

**Sample Requirement**: Collect 3 real Codex approval request samples.

### Story 2.3: ACP Classifier

| Test ID | Test Name | Priority | Definition |
|---------|-----------|----------|------------|
| T2.3.T1 | `test_acp_permission_asked` | P0 | permission.asked → WaitingForChoice |
| T2.3.T2 | `test_acp_permission_options` | P0 | once/always/reject options parsed |
| T2.3.T3 | `test_acp_session_status_idle` | P0 | session.status.idle → ClaimsCompletion |
| T2.3.T4 | `test_acp_session_status_busy` | P0 | session.status.busy → Running |
| T2.3.T5 | `test_acp_session_status_retry` | P0 | retry (attempt <= 3) → Running |
| T2.3.T6 | `test_acp_session_status_retry_exhausted` | P0 | retry (attempt > 3) → Error |
| T2.3.T7 | `test_acp_error_notification` | P0 | ACPError → Error status |
| T2.3.T8 | `test_acp_real_sample_opencode` | P1 | Real OpenCode permission.asked sample |
| T2.3.T9 | `test_acp_real_sample_kimi` | P1 | Real Kimi permission.asked sample |

**Sample Requirement**: Collect 3 ACP samples from OpenCode, 3 from Kimi.

### Story 2.4: ClassifierRegistry

| Test ID | Test Name | Priority | Definition |
|---------|-----------|----------|------------|
| T2.4.T1 | `test_registry_claude_dispatch` | P0 | Claude events dispatch to ClaudeClassifier |
| T2.4.T2 | `test_registry_codex_dispatch` | P0 | Codex events dispatch to CodexClassifier |
| T2.4.T3 | `test_registry_acp_dispatch` | P0 | ACP events dispatch to ACPClassifier |
| T2.4.T4 | `test_registry_fallback` | P0 | Unknown provider uses FallbackClassifier |
| T2.4.T5 | `test_registry_all_providers` | P0 | All ProviderKind variants handled |

---

## Sprint 3: Decision Engine - Test Tasks

### Story 3.1: DecisionEngine Trait

| Test ID | Test Name | Priority | Definition |
|---------|-----------|----------|------------|
| T3.1.T1 | `test_trait_engine_type` | P0 | engine_type() returns correct type |
| T3.1.T2 | `test_trait_decide_signature` | P0 | decide() takes Context, returns Output |
| T3.1.T3 | `test_trait_build_prompt` | P0 | build_prompt() generates valid prompt |
| T3.1.T4 | `test_trait_session_handle` | P0 | session_handle() returns Option |
| T3.1.T5 | `test_trait_is_healthy` | P0 | is_healthy() returns bool |
| T3.1.T6 | `test_trait_reset` | P0 | reset() clears state |

### Story 3.2: LLM Decision Engine

| Test ID | Test Name | Priority | Definition |
|---------|-----------|----------|------------|
| T3.2.T1 | `test_llm_choice_prompt_format` | P0 | Choice prompt contains all required sections |
| T3.2.T2 | `test_llm_reflection_prompt_format` | P0 | Reflection prompt contains correct round |
| T3.2.T3 | `test_llm_verification_prompt_format` | P0 | Verification prompt for DoD check |
| T3.2.T4 | `test_llm_continue_prompt_format` | P0 | Continue prompt with focus_items |
| T3.2.T5 | `test_llm_retry_prompt_format` | P0 | Retry prompt for each ErrorType |
| T3.2.T6 | `test_llm_response_parse_choice` | P0 | Parse LLM response to Choice output |
| T3.2.T7 | `test_llm_response_parse_reflection` | P0 | Parse LLM response to ReflectionRequest |
| T3.2.T8 | `test_llm_response_parse_completion` | P0 | Parse LLM response to CompletionConfirm |
| T3.2.T9 | `test_llm_timeout_handling` | P0 | Timeout returns error or fallback |
| T3.2.T10 | `test_llm_session_persist_restore` | P0 | Session persists and restores |
| T3.2.T11 | `test_llm_mock_response` | P1 | Mock LLM responses for testing |

### Story 3.3: CLI Decision Engine

| Test ID | Test Name | Priority | Definition |
|---------|-----------|----------|------------|
| T3.3.T1 | `test_cli_session_independence` | P0 | CLI session different from main agent |
| T3.3.T2 | `test_cli_parent_agent_id` | P0 | parent_agent_id stored correctly |
| T3.3.T3 | `test_cli_provider_spawn` | P0 | Provider thread spawns correctly |
| T3.3.T4 | `test_cli_event_channel` | P0 | Events received via channel |
| T3.3.T5 | `test_cli_output_collect` | P0 | Output collected until blocked |
| T3.3.T6 | `test_cli_parse_output` | P0 | Provider output parsed to DecisionOutput |
| T3.3.T7 | `test_cli_session_create` | P0 | New session created for decision |
| T3.3.T8 | `test_cli_session_resume` | P0 | Existing session resumed |

### Story 3.4: Rule-Based Engine

| Test ID | Test Name | Priority | Definition |
|---------|-----------|----------|------------|
| T3.4.T1 | `test_rule_match_waiting_for_choice` | P0 | Rules match WaitingForChoice status |
| T3.4.T2 | `test_rule_match_claims_completion` | P0 | Rules match ClaimsCompletion status |
| T3.4.T3 | `test_rule_match_error` | P0 | Rules match Error status |
| T3.4.T4 | `test_rule_project_keywords` | P0 | Project keyword matching works |
| T3.4.T5 | `test_rule_default_rules` | P0 | Default rules cover common scenarios |
| T3.4.T6 | `test_rule_custom_rules_load` | P0 | Custom rules load from file |
| T3.4.T7 | `test_rule_no_match_fallback` | P0 | No matching rule returns continue |

### Story 3.5: Mock Engine

| Test ID | Test Name | Priority | Definition |
|---------|-----------|----------|------------|
| T3.5.T1 | `test_mock_choice_first_option` | P0 | Returns first option for choice |
| T3.5.T2 | `test_mock_reflection_rounds` | P0 | Reflection for rounds < 2 |
| T3.5.T3 | `test_mock_completion_confirm` | P0 | Completion for rounds >= 2 |
| T3.5.T4 | `test_mock_retry_on_error` | P0 | Retry on Error status |
| T3.5.T5 | `test_mock_history_record` | P0 | History recorded for each decision |
| T3.5.T6 | `test_mock_always_healthy` | P0 | is_healthy() always true |

---

## Sprint 4: Context Cache - Test Tasks

### Story 4.1: Size Limits

| Test ID | Test Name | Priority | Definition |
|---------|-----------|----------|------------|
| T4.1.T1 | `test_cache_tool_call_limit` | P0 | Max tool calls respected |
| T4.1.T2 | `test_cache_file_change_limit` | P0 | Max file changes respected |
| T4.1.T3 | `test_cache_key_output_limit` | P0 | Max key outputs respected |
| T4.1.T4 | `test_cache_total_size_limit` | P0 | Total bytes limit respected |
| T4.1.T5 | `test_cache_overflow_removes_oldest` | P0 | Overflow removes oldest entry |
| T4.1.T6 | `test_cache_size_estimation` | P0 | estimate_size() accurate within 10% |

### Story 4.2: Compression

| Test ID | Test Name | Priority | Definition |
|---------|-----------|----------|------------|
| T4.2.T1 | `test_compress_reduces_size` | P0 | compress() reduces to below limit |
| T4.2.T2 | `test_compress_priority_retention` | P0 | High priority info retained |
| T4.2.T3 | `test_compress_thinking_truncate` | P0 | Thinking truncated to half |
| T4.2.T4 | `test_compress_file_diff_remove` | P0 | File diffs removed, paths kept |
| T4.2.T5 | `test_is_key_info_detection` | P0 | Key info correctly identified |
| T4.2.T6 | `test_generate_summary_format` | P0 | Summary has correct format |

### Story 4.3: Persistence

| Test ID | Test Name | Priority | Definition |
|---------|-----------|----------|------------|
| T4.3.T1 | `test_persist_json_format` | P0 | JSON format valid |
| T4.3.T2 | `test_restore_preserves_entries` | P0 | All entries restored |
| T4.3.T3 | `test_restore_truncate_if_limits_changed` | P0 | Truncate if config changed |
| T4.3.T4 | `test_persist_roundtrip` | P0 | Persist → restore matches |

---

## Sprint 5: Lifecycle - Test Tasks

### Story 5.1: Creation Policies

| Test ID | Test Name | Priority | Definition |
|---------|-----------|----------|------------|
| T5.1.T1 | `test_eager_creation_on_spawn` | P0 | Decision Agent created at Main Agent spawn |
| T5.1.T2 | `test_lazy_creation_on_blocked` | P0 | Decision Agent created on first blocked |
| T5.1.T3 | `test_configured_policy` | P0 | Policy follows config setting |
| T5.1.T4 | `test_creation_config_override` | P0 | Config overrides default |

### Story 5.2: Destruction

| Test ID | Test Name | Priority | Definition |
|---------|-----------|----------|------------|
| T5.2.T1 | `test_story_complete_cleanup` | P0 | Session released on story complete |
| T5.2.T2 | `test_idle_timeout_cleanup` | P0 | Full cleanup on idle timeout |
| T5.2.T3 | `test_manual_stop_cleanup` | P0 | Cleanup on manual stop |
| T5.2.T4 | `test_session_release` | P0 | Provider session correctly closed |
| T5.2.T5 | `test_transcript_archived` | P0 | Transcript saved before destruction |

### Story 5.3: Task Switching

| Test ID | Test Name | Priority | Definition |
|---------|-----------|----------|------------|
| T5.3.T1 | `test_switch_task_context_save` | P0 | Current context saved on switch |
| T5.3.T2 | `test_switch_task_context_restore` | P0 | New context restored correctly |
| T5.3.T3 | `test_session_continuation` | P0 | Session continues across tasks |
| T5.3.T4 | `test_history_archived` | P0 | Completed task history archived |

### Story 5.4: Session Persistence

| Test ID | Test Name | Priority | Definition |
|---------|-----------|----------|------------|
| T5.4.T1 | `test_state_json_format` | P0 | state.json valid format |
| T5.4.T2 | `test_restore_all_fields` | P0 | All fields restored correctly |
| T5.4.T3 | `test_session_validation` | P0 | Stale session detected |
| T5.4.T4 | `test_transcript_persist` | P0 | transcript.json valid |

---

## Sprint 6: Human Intervention - Test Tasks

### Story 6.1: Criteria Evaluation

| Test ID | Test Name | Priority | Definition |
|---------|-----------|----------|------------|
| T6.1.T1 | `test_multi_agent_impact_detection` | P0 | Multi-agent impact detected |
| T6.1.T2 | `test_irreversible_detection` | P0 | Irreversible operations detected |
| T6.1.T3 | `test_high_risk_detection` | P0 | High risk operations detected |
| T6.1.T4 | `test_low_confidence_detection` | P0 | Low confidence detected |
| T6.1.T5 | `test_project_rule_detection` | P0 | Requires human rule detected |
| T6.1.T6 | `test_criticality_score_weights` | P0 | Score weights match spec |

### Story 6.2: Decision Queue

| Test ID | Test Name | Priority | Definition |
|---------|-----------|----------|------------|
| T6.2.T1 | `test_queue_push` | P0 | Request added to correct priority queue |
| T6.2.T2 | `test_queue_pop_priority_order` | P0 | High > Medium > Low order |
| T6.2.T3 | `test_queue_timeout_check` | P0 | Expired requests detected |
| T6.2.T4 | `test_queue_complete` | P0 | Request removed, response archived |
| T6.2.T5 | `test_queue_history` | P0 | History preserved |

### Story 6.3: Notification

| Test ID | Test Name | Priority | Definition |
|---------|-----------|----------|------------|
| T6.3.T1 | `test_new_request_notification` | P0 | NewRequest notification sent |
| T6.3.T2 | `test_timeout_warning_notification` | P0 | ApproachingTimeout sent |
| T6.3.T3 | `test_timeout_expired_notification` | P0 | TimeoutExpired sent |
| T6.3.T4 | `test_urgent_notification` | P0 | UrgentRequest sent |

### Story 6.4: TUI Interface

| Test ID | Test Name | Priority | Definition |
|---------|-----------|----------|------------|
| T6.4.T1 | `test_modal_display_request` | P0 | Request details displayed |
| T6.4.T2 | `test_modal_display_analysis` | P0 | Decision Agent analysis shown |
| T6.4.T3 | `test_modal_option_selection` | P0 | Options selectable via keyboard |
| T6.4.T4 | `test_modal_recommendation_accept` | P0 | Recommendation accepted |
| T6.4.T5 | `test_modal_custom_input` | P0 | Custom instruction input works |
| T6.4.T6 | `test_modal_skip_cancel` | P0 | Skip and cancel work |

### Story 6.5: CLI Commands

| Test ID | Test Name | Priority | Definition |
|---------|-----------|----------|------------|
| T6.5.T1 | `test_cli_list_pending` | P0 | Pending requests listed correctly |
| T6.5.T2 | `test_cli_show_details` | P0 | Request details displayed |
| T6.5.T3 | `test_cli_respond_select` | P0 | Select option works |
| T6.5.T4 | `test_cli_respond_accept` | P0 | Accept recommendation works |
| T6.5.T5 | `test_cli_respond_custom` | P0 | Custom instruction works |
| T6.5.T6 | `test_cli_respond_skip` | P0 | Skip task works |

---

## Sprint 7: Error Recovery - Test Tasks

### Story 7.1: Recovery Levels

| Test ID | Test Name | Priority | Definition |
|---------|-----------|----------|------------|
| T7.1.T1 | `test_level_auto_retry` | P0 | Level 0: AutoRetry triggered |
| T7.1.T2 | `test_level_adjusted_retry` | P0 | Level 1: AdjustedRetry triggered |
| T7.1.T3 | `test_level_switch_engine` | P0 | Level 2: SwitchEngine triggered |
| T7.1.T4 | `test_level_human_intervention` | P0 | Level 3: HumanIntervention triggered |
| T7.1.T5 | `test_level_task_failed` | P0 | Level 4: TaskFailed triggered |
| T7.1.T6 | `test_escalation_order` | P0 | Escalation follows correct order |

### Story 7.2: Timeout Handling

| Test ID | Test Name | Priority | Definition |
|---------|-----------|----------|------------|
| T7.2.T1 | `test_timeout_detection` | P0 | Timeout detected correctly |
| T7.2.T2 | `test_timeout_retry` | P0 | Timeout retry works |
| T7.2.T3 | `test_timeout_fallback_rule_based` | P0 | RuleBased fallback works |
| T7.2.T4 | `test_timeout_fallback_default` | P0 | Default decision fallback works |
| T7.2.T5 | `test_timeout_fallback_human` | P0 | Human fallback works |

### Story 7.3: Self-Error Recovery

| Test ID | Test Name | Priority | Definition |
|---------|-----------|----------|------------|
| T7.3.T1 | `test_engine_error_recovery` | P0 | Engine error switches engine |
| T7.3.T2 | `test_session_lost_recovery` | P0 | Session recreated |
| T7.3.T3 | `test_context_parse_error_recovery` | P0 | Context rebuilt |
| T7.3.T4 | `test_internal_error_reset` | P0 | Full reset on internal error |

### Story 7.4: Health Check

| Test ID | Test Name | Priority | Definition |
|---------|-----------|----------|------------|
| T7.4.T1 | `test_health_success_rate` | P0 | Success rate calculated correctly |
| T7.4.T2 | `test_health_avg_duration` | P0 | Avg duration calculated |
| T7.4.T3 | `test_health_consecutive_failures` | P0 | Failures tracked |
| T7.4.T4 | `test_is_healthy_criteria` | P0 | Healthy criteria checked |
| T7.4.T5 | `test_auto_recover_trigger` | P0 | Auto-recovery triggered when unhealthy |

---

## Sprint 8: Integration - Test Tasks

### Story 8.1: AgentPool Integration

| Test ID | Test Name | Priority | Definition |
|---------|-----------|----------|------------|
| T8.1.T1 | `test_blocked_status_set` | P0 | BlockedForHumanDecision status set |
| T8.1.T2 | `test_blocked_task_keep_assigned` | P0 | Task stays with blocked agent |
| T8.1.T3 | `test_blocked_task_reassign` | P0 | Task reassigned to idle agent |
| T8.1.T4 | `test_blocked_mail_notification` | P0 | Other agents notified via mail |
| T8.1.T5 | `test_blocked_status_clear` | P0 | Status cleared on human response |
| T8.1.T6 | `test_decision_execution` | P0 | Decision executed on main agent |

### Story 8.2: Kanban Integration

| Test ID | Test Name | Priority | Definition |
|---------|-----------|----------|------------|
| T8.2.T1 | `test_task_complete_kanban_update` | P0 | Kanban moved to Done |
| T8.2.T2 | `test_task_failed_kanban_update` | P0 | Kanban moved to Failed |
| T8.2.T3 | `test_next_task_selection` | P0 | Next task selected from Todo |
| T8.2.T4 | `test_story_definition_load` | P0 | Story definition loaded correctly |
| T8.2.T5 | `test_task_definition_load` | P0 | Task definition loaded correctly |

### Story 8.3: WorkplaceStore Integration

| Test ID | Test Name | Priority | Definition |
|---------|-----------|----------|------------|
| T8.3.T1 | `test_decision_path_create` | P0 | Decision directory created |
| T8.3.T2 | `test_decision_persist` | P0 | Decision state persisted |
| T8.3.T3 | `test_decision_restore` | P0 | Decision agent restored |
| T8.3.T4 | `test_project_rules_load` | P0 | CLAUDE.md rules loaded |

### Story 8.4: Observability

| Test ID | Test Name | Priority | Definition |
|---------|-----------|----------|------------|
| T8.4.T1 | `test_metrics_total_decisions` | P0 | Total decisions tracked |
| T8.4.T2 | `test_metrics_success_rate` | P0 | Success rate calculated |
| T8.4.T3 | `test_metrics_by_type` | P0 | Decisions by type tracked |
| T8.4.T4 | `test_log_format` | P0 | Log format valid JSON |
| T8.4.T5 | `test_cli_metrics_command` | P0 | CLI metrics output valid |

### Story 8.5: Cost Optimization

| Test ID | Test Name | Priority | Definition |
|---------|-----------|----------|------------|
| T8.5.T1 | `test_tiered_engine_complexity_threshold` | P0 | Complexity threshold works |
| T8.5.T2 | `test_tiered_engine_rule_first` | P0 | Rule engine used for low complexity |
| T8.5.T3 | `test_tiered_engine_llm_second` | P0 | LLM engine used for high complexity |
| T8.5.T4 | `test_decision_cache_hit` | P0 | Cache returns cached decision |
| T8.5.T5 | `test_budget_tracking` | P0 | Budget tracked correctly |

---

## Provider Output Sample Collection Plan

Before implementing classifiers, collect real samples:

| Sample ID | Provider | Scenario | Source | Status |
|-----------|----------|----------|--------|--------|
| S-CL-001 | Claude | Finished event | Production run | Todo |
| S-CL-002 | Claude | Error event | Production run | Todo |
| S-CL-003 | Claude | AssistantChunk stream | Production run | Todo |
| S-CX-001 | Codex | execCommandApproval | Production run | Todo |
| S-CX-002 | Codex | applyPatchApproval | Production run | Todo |
| S-CX-003 | Codex | requestUserInput | Production run | Todo |
| S-OC-001 | OpenCode | permission.asked (write) | ACP session | Todo |
| S-OC-002 | OpenCode | permission.asked (exec) | ACP session | Todo |
| S-OC-003 | OpenCode | session.status.idle | ACP session | Todo |
| S-KM-001 | Kimi | permission.asked (edit) | ACP session | Todo |
| S-KM-002 | Kimi | session.status.idle | ACP session | Todo |
| S-KM-003 | Kimi | AUTH_REQUIRED error | ACP session | Todo |

---

## Test Execution Order

For each Sprint, execute tests in this order:

1. **TDD Phase**: Write failing tests first
2. **Implementation Phase**: Implement minimum code to pass
3. **Refactor Phase**: Clean up while keeping tests passing
4. **Sample Validation Phase**: Run real provider sample tests

---

## References

- [Decision Layer README](README.md)
- [Sprint 1: Core Types](sprint-01-core-types.md)
- All sprint specification documents