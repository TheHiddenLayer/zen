//! Workflow management types for the Zen orchestrator.
//!
//! This module provides the core type definitions for tracking workflow
//! lifecycle and phases in the parallel multi-agent orchestration system.

mod types;

pub use types::{TaskId, Workflow, WorkflowConfig, WorkflowId, WorkflowPhase, WorkflowStatus};
