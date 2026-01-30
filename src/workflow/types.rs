//! Core workflow type definitions.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Unique identifier for a workflow instance.
///
/// Uses UUID v4 for generation and provides a short form display
/// for human-readable output.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct WorkflowId(pub Uuid);

impl WorkflowId {
    /// Create a new unique workflow identifier.
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    /// Return first 8 characters of the UUID for display.
    pub fn short(&self) -> String {
        self.0.to_string()[..8].to_string()
    }
}

impl Default for WorkflowId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for WorkflowId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::str::FromStr for WorkflowId {
    type Err = uuid::Error;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        Ok(Self(Uuid::parse_str(s)?))
    }
}

/// Phases of the workflow execution pipeline.
///
/// The phases follow the Skills-driven workflow:
/// 1. Planning - Run /pdd to generate design and plan
/// 2. TaskGeneration - Run /code-task-generator to create tasks
/// 3. Implementation - Run /code-assist in parallel for each task
/// 4. Merging - Merge worktrees and resolve conflicts
/// 5. Documentation - Optional /codebase-summary for documentation
/// 6. Complete - All phases finished
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowPhase {
    /// Phase 1: Run /pdd to generate design and plan
    Planning,
    /// Phase 2: Run /code-task-generator to create tasks
    TaskGeneration,
    /// Phase 3: Run /code-assist in parallel for each task
    Implementation,
    /// Phase 4: Merge worktrees and resolve conflicts
    Merging,
    /// Phase 5: Optional /codebase-summary for documentation
    Documentation,
    /// All phases complete
    Complete,
}

impl std::fmt::Display for WorkflowPhase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WorkflowPhase::Planning => write!(f, "planning"),
            WorkflowPhase::TaskGeneration => write!(f, "task_generation"),
            WorkflowPhase::Implementation => write!(f, "implementation"),
            WorkflowPhase::Merging => write!(f, "merging"),
            WorkflowPhase::Documentation => write!(f, "documentation"),
            WorkflowPhase::Complete => write!(f, "complete"),
        }
    }
}

/// Status of a workflow in its lifecycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowStatus {
    /// Workflow created but not yet started
    #[default]
    Pending,
    /// Workflow is actively executing
    Running,
    /// Workflow execution paused
    Paused,
    /// Workflow completed successfully
    Completed,
    /// Workflow failed with error
    Failed,
}

impl std::fmt::Display for WorkflowStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WorkflowStatus::Pending => write!(f, "pending"),
            WorkflowStatus::Running => write!(f, "running"),
            WorkflowStatus::Paused => write!(f, "paused"),
            WorkflowStatus::Completed => write!(f, "completed"),
            WorkflowStatus::Failed => write!(f, "failed"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // WorkflowId tests

    #[test]
    fn test_workflow_id_new() {
        let id1 = WorkflowId::new();
        let id2 = WorkflowId::new();
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_workflow_id_default() {
        let id = WorkflowId::default();
        assert!(!id.0.is_nil());
    }

    #[test]
    fn test_workflow_id_short() {
        let id = WorkflowId::new();
        let short = id.short();
        assert_eq!(short.len(), 8);
    }

    #[test]
    fn test_workflow_id_display() {
        let id = WorkflowId::new();
        let display = format!("{}", id);
        assert_eq!(display, id.0.to_string());
    }

    #[test]
    fn test_workflow_id_from_str() {
        let id = WorkflowId::new();
        let s = id.to_string();
        let parsed: WorkflowId = s.parse().unwrap();
        assert_eq!(id, parsed);
    }

    #[test]
    fn test_workflow_id_from_str_invalid() {
        let result: std::result::Result<WorkflowId, _> = "invalid".parse();
        assert!(result.is_err());
    }

    #[test]
    fn test_workflow_id_serialization() {
        let id = WorkflowId::new();
        let json = serde_json::to_string(&id).unwrap();
        let parsed: WorkflowId = serde_json::from_str(&json).unwrap();
        assert_eq!(id, parsed);
    }

    #[test]
    fn test_workflow_id_equality() {
        let uuid = Uuid::new_v4();
        let id1 = WorkflowId(uuid);
        let id2 = WorkflowId(uuid);
        assert_eq!(id1, id2);
    }

    #[test]
    fn test_workflow_id_hash() {
        use std::collections::HashSet;

        let uuid = Uuid::new_v4();
        let id1 = WorkflowId(uuid);
        let id2 = WorkflowId(uuid);

        let mut set = HashSet::new();
        set.insert(id1);
        assert!(set.contains(&id2));
    }

    // WorkflowPhase tests

    #[test]
    fn test_workflow_phase_ordering() {
        assert!(WorkflowPhase::Planning < WorkflowPhase::TaskGeneration);
        assert!(WorkflowPhase::TaskGeneration < WorkflowPhase::Implementation);
        assert!(WorkflowPhase::Implementation < WorkflowPhase::Merging);
        assert!(WorkflowPhase::Merging < WorkflowPhase::Documentation);
        assert!(WorkflowPhase::Documentation < WorkflowPhase::Complete);
    }

    #[test]
    fn test_workflow_phase_display_planning() {
        assert_eq!(format!("{}", WorkflowPhase::Planning), "planning");
    }

    #[test]
    fn test_workflow_phase_display_task_generation() {
        assert_eq!(format!("{}", WorkflowPhase::TaskGeneration), "task_generation");
    }

    #[test]
    fn test_workflow_phase_display_implementation() {
        assert_eq!(format!("{}", WorkflowPhase::Implementation), "implementation");
    }

    #[test]
    fn test_workflow_phase_display_merging() {
        assert_eq!(format!("{}", WorkflowPhase::Merging), "merging");
    }

    #[test]
    fn test_workflow_phase_display_documentation() {
        assert_eq!(format!("{}", WorkflowPhase::Documentation), "documentation");
    }

    #[test]
    fn test_workflow_phase_display_complete() {
        assert_eq!(format!("{}", WorkflowPhase::Complete), "complete");
    }

    #[test]
    fn test_workflow_phase_serialization() {
        let phases = [
            WorkflowPhase::Planning,
            WorkflowPhase::TaskGeneration,
            WorkflowPhase::Implementation,
            WorkflowPhase::Merging,
            WorkflowPhase::Documentation,
            WorkflowPhase::Complete,
        ];

        for phase in phases {
            let json = serde_json::to_string(&phase).unwrap();
            let parsed: WorkflowPhase = serde_json::from_str(&json).unwrap();
            assert_eq!(phase, parsed);
        }
    }

    #[test]
    fn test_workflow_phase_serialization_format() {
        assert_eq!(serde_json::to_string(&WorkflowPhase::Planning).unwrap(), r#""planning""#);
        assert_eq!(serde_json::to_string(&WorkflowPhase::TaskGeneration).unwrap(), r#""task_generation""#);
        assert_eq!(serde_json::to_string(&WorkflowPhase::Implementation).unwrap(), r#""implementation""#);
        assert_eq!(serde_json::to_string(&WorkflowPhase::Merging).unwrap(), r#""merging""#);
        assert_eq!(serde_json::to_string(&WorkflowPhase::Documentation).unwrap(), r#""documentation""#);
        assert_eq!(serde_json::to_string(&WorkflowPhase::Complete).unwrap(), r#""complete""#);
    }

    #[test]
    fn test_workflow_phase_equality() {
        assert_eq!(WorkflowPhase::Planning, WorkflowPhase::Planning);
        assert_ne!(WorkflowPhase::Planning, WorkflowPhase::Complete);
    }

    // WorkflowStatus tests

    #[test]
    fn test_workflow_status_default() {
        let status = WorkflowStatus::default();
        assert_eq!(status, WorkflowStatus::Pending);
    }

    #[test]
    fn test_workflow_status_display_pending() {
        assert_eq!(format!("{}", WorkflowStatus::Pending), "pending");
    }

    #[test]
    fn test_workflow_status_display_running() {
        assert_eq!(format!("{}", WorkflowStatus::Running), "running");
    }

    #[test]
    fn test_workflow_status_display_paused() {
        assert_eq!(format!("{}", WorkflowStatus::Paused), "paused");
    }

    #[test]
    fn test_workflow_status_display_completed() {
        assert_eq!(format!("{}", WorkflowStatus::Completed), "completed");
    }

    #[test]
    fn test_workflow_status_display_failed() {
        assert_eq!(format!("{}", WorkflowStatus::Failed), "failed");
    }

    #[test]
    fn test_workflow_status_serialization() {
        let statuses = [
            WorkflowStatus::Pending,
            WorkflowStatus::Running,
            WorkflowStatus::Paused,
            WorkflowStatus::Completed,
            WorkflowStatus::Failed,
        ];

        for status in statuses {
            let json = serde_json::to_string(&status).unwrap();
            let parsed: WorkflowStatus = serde_json::from_str(&json).unwrap();
            assert_eq!(status, parsed);
        }
    }

    #[test]
    fn test_workflow_status_serialization_format() {
        assert_eq!(serde_json::to_string(&WorkflowStatus::Pending).unwrap(), r#""pending""#);
        assert_eq!(serde_json::to_string(&WorkflowStatus::Running).unwrap(), r#""running""#);
        assert_eq!(serde_json::to_string(&WorkflowStatus::Paused).unwrap(), r#""paused""#);
        assert_eq!(serde_json::to_string(&WorkflowStatus::Completed).unwrap(), r#""completed""#);
        assert_eq!(serde_json::to_string(&WorkflowStatus::Failed).unwrap(), r#""failed""#);
    }
}
