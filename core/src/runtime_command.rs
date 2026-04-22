//! RuntimeCommand — re-exported from agent-runtime-domain.
//!
//! The canonical implementation lives in `agent-runtime-domain`.
//! This file provides backward compatibility for code using `agent_core::runtime_command`.

pub use agent_runtime_domain::runtime_command::*;

// EffectHandler trait and implementations live in agent-behavior-infra.
// Re-export them here for backward compatibility.
pub use agent_behavior_infra::{EffectHandler, NoopEffectHandler, RecordingEffectHandler};
