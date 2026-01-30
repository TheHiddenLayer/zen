//! Orchestration layer for the Zen multi-agent system.
//!
//! This module provides components for coordinating multiple AI agents,
//! including the AI-as-Human proxy that autonomously answers skill
//! clarification questions, the Claude headless executor for
//! programmatic interaction with Claude Code, the central
//! SkillsOrchestrator that drives the workflow phases, the
//! Scheduler for parallel task execution, the ConflictResolver
//! for merging worktrees and resolving conflicts, and the HealthMonitor
//! for detecting stuck or failing agents.

mod ai_human;
mod claude;
pub mod detection;
mod health;
mod pool;
mod resolver;
mod scheduler;
mod skills;

pub use ai_human::{AIHumanProxy, ClaudeBackendConfig, ConversationContext};
pub use claude::{ClaudeHeadless, ClaudeResponse, ResultType, SessionManager, DEFAULT_TIMEOUT_SECS};
pub use health::{HealthConfig, HealthEvent, HealthMonitor, RecoveryAction, RetryTracker};
pub use pool::{AgentEvent, AgentHandle, AgentOutput, AgentPool};
pub use resolver::{ConflictFile, ConflictResolver, MergeResult};
pub use scheduler::{ImplResult, Scheduler, SchedulerEvent};
pub use skills::{MonitorConfig, PDDResult, PhaseController, PhaseEvent, SkillResult, SkillsOrchestrator, WorkflowResult};
