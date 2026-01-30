//! Orchestration layer for the Zen multi-agent system.
//!
//! This module provides components for coordinating multiple AI agents,
//! including the AI-as-Human proxy that autonomously answers skill
//! clarification questions.

mod ai_human;
mod pool;

pub use ai_human::{AIHumanProxy, ConversationContext};
pub use pool::{AgentEvent, AgentHandle, AgentOutput, AgentPool};
