//! Engine layer - decision execution implementations
//!
//! Provides various decision engines:
//! - DecisionEngine trait (base interface)
//! - RuleBasedDecisionEngine (simple situations)
//! - LLMDecisionEngine (complex situations)
//! - TieredDecisionEngine (complexity-based selection)
//! - CLIDecisionEngine (human decisions)
//! - MockDecisionEngine (testing)
//! - TaskDecisionEngine (task-specific decisions)
//! - LLMCaller trait (LLM provider interface)

pub mod engine;
pub mod rule_engine;
pub mod llm_engine;
pub mod tiered_engine;
pub mod cli_engine;
pub mod mock_engine;
pub mod task_engine;
pub mod llm_caller;

pub use engine::*;
pub use rule_engine::*;
pub use llm_engine::*;
pub use tiered_engine::*;
pub use cli_engine::*;
pub use mock_engine::*;
pub use task_engine::*;
pub use llm_caller::*;
