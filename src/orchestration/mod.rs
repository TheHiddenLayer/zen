//! Orchestration layer for the Zen multi-agent system.
//!
//! This module provides components for coordinating multiple AI agents,
//! including the AI-as-Human proxy that autonomously answers skill
//! clarification questions, the Claude headless executor for
//! programmatic interaction with Claude Code, and the central
//! SkillsOrchestrator that drives the workflow phases.

mod ai_human;
mod claude;
mod pool;
mod skills;

pub use ai_human::{AIHumanProxy, ClaudeBackendConfig, ConversationContext};
pub use claude::{ClaudeHeadless, ClaudeResponse, ResultType, SessionManager, DEFAULT_TIMEOUT_SECS};
pub use pool::{AgentEvent, AgentHandle, AgentOutput, AgentPool};
pub use skills::{PhaseController, PhaseEvent, SkillsOrchestrator, WorkflowResult};
