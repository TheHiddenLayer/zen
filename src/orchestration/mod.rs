//! Orchestration layer for the Zen multi-agent system.
//!
//! This module provides components for coordinating multiple AI agents,
//! including the AI-as-Human proxy that autonomously answers skill
//! clarification questions and the Claude headless executor for
//! programmatic interaction with Claude Code.

mod ai_human;
mod claude;
mod pool;

pub use ai_human::{AIHumanProxy, ConversationContext};
pub use claude::{ClaudeHeadless, ClaudeResponse, ResultType, DEFAULT_TIMEOUT_SECS};
pub use pool::{AgentEvent, AgentHandle, AgentOutput, AgentPool};
