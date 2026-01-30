//! Core domain models for Zen v2 orchestration.
//!
//! This module contains the fundamental data structures used throughout
//! the orchestration system, including tasks and the execution DAG.

pub mod dag;
pub mod task;

pub use dag::{DependencyType, TaskDAG};
pub use task::{Task, TaskId, TaskStatus};
