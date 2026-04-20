//! Profile Resolver
//!
//! Re-exported from agent-provider for backward compatibility.

pub use agent_provider::profile::{
    resolve_profile, resolve_profile_by_id, profile_to_launch_input,
    get_effective_profile, AgentType, create_launch_context_from_profile,
    create_launch_context_from_profile_with_session,
};
