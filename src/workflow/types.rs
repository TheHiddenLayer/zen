//! Core workflow type definitions.

use chrono::{DateTime, Utc};
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
    /// Workflow accepted and merged to main
    Accepted,
    /// Workflow rejected and discarded
    Rejected,
}

impl std::fmt::Display for WorkflowStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WorkflowStatus::Pending => write!(f, "pending"),
            WorkflowStatus::Running => write!(f, "running"),
            WorkflowStatus::Paused => write!(f, "paused"),
            WorkflowStatus::Completed => write!(f, "completed"),
            WorkflowStatus::Failed => write!(f, "failed"),
            WorkflowStatus::Accepted => write!(f, "accepted"),
            WorkflowStatus::Rejected => write!(f, "rejected"),
        }
    }
}

/// Unique identifier for a task within a workflow.
///
/// This is a placeholder type that will be fully implemented in Step 8.
/// For now it provides the basic structure needed by the Workflow type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct TaskId(pub Uuid);

impl TaskId {
    /// Create a new unique task identifier.
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for TaskId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for TaskId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Configuration options for a workflow execution.
///
/// Controls behavior like documentation updates and parallelism limits.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowConfig {
    /// Whether to run /codebase-summary phase for documentation updates.
    pub update_docs: bool,
    /// Maximum number of concurrent agents during implementation phase.
    pub max_parallel_agents: usize,
    /// Prefix for staging branches (e.g., "zen/staging/").
    pub staging_branch_prefix: String,
}

impl Default for WorkflowConfig {
    fn default() -> Self {
        Self {
            update_docs: true,
            max_parallel_agents: 4,
            staging_branch_prefix: String::from("zen/staging/"),
        }
    }
}

/// A workflow representing a complete orchestration run.
///
/// Each workflow tracks the original user prompt, current execution phase,
/// status, timestamps, configuration, and associated task IDs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Workflow {
    /// Unique identifier for this workflow.
    pub id: WorkflowId,
    /// Human-readable name derived from the prompt.
    pub name: String,
    /// Original user prompt that initiated this workflow.
    pub prompt: String,
    /// Current execution phase.
    pub phase: WorkflowPhase,
    /// Current workflow status.
    pub status: WorkflowStatus,
    /// Configuration options for this workflow.
    pub config: WorkflowConfig,
    /// When the workflow was created.
    pub created_at: DateTime<Utc>,
    /// When the workflow started executing.
    pub started_at: Option<DateTime<Utc>>,
    /// When the workflow completed (success or failure).
    pub completed_at: Option<DateTime<Utc>>,
    /// IDs of tasks associated with this workflow.
    #[serde(rename = "tasks")]
    pub task_ids: Vec<TaskId>,
}

impl Workflow {
    /// Create a new workflow from a user prompt.
    ///
    /// The workflow starts in the Pending status with the Planning phase.
    /// A name is automatically derived from the first few words of the prompt.
    pub fn new(prompt: &str, config: WorkflowConfig) -> Self {
        Self {
            id: WorkflowId::new(),
            name: Self::derive_name(prompt),
            prompt: prompt.to_string(),
            phase: WorkflowPhase::Planning,
            status: WorkflowStatus::Pending,
            config,
            created_at: Utc::now(),
            started_at: None,
            completed_at: None,
            task_ids: Vec::new(),
        }
    }

    /// Derive a kebab-case name from the prompt.
    ///
    /// Takes the first few words, lowercases them, and joins with hyphens.
    fn derive_name(prompt: &str) -> String {
        prompt
            .split_whitespace()
            .take(4)
            .map(|w| w.to_lowercase())
            .map(|w| w.chars().filter(|c| c.is_alphanumeric()).collect::<String>())
            .filter(|w| !w.is_empty())
            .collect::<Vec<_>>()
            .join("-")
    }

    /// Start the workflow execution.
    ///
    /// Sets status to Running and records the start time.
    pub fn start(&mut self) {
        self.status = WorkflowStatus::Running;
        self.started_at = Some(Utc::now());
    }

    /// Mark the workflow as successfully completed.
    ///
    /// Sets status to Completed, phase to Complete, and records completion time.
    pub fn complete(&mut self) {
        self.status = WorkflowStatus::Completed;
        self.phase = WorkflowPhase::Complete;
        self.completed_at = Some(Utc::now());
    }

    /// Mark the workflow as failed.
    ///
    /// Sets status to Failed and records completion time.
    pub fn fail(&mut self) {
        self.status = WorkflowStatus::Failed;
        self.completed_at = Some(Utc::now());
    }

    /// Mark the workflow as accepted (merged to main).
    ///
    /// Sets status to Accepted. Should only be called after workflow is Completed.
    pub fn accept(&mut self) {
        self.status = WorkflowStatus::Accepted;
    }

    /// Mark the workflow as rejected (discarded).
    ///
    /// Sets status to Rejected. Can be called on any completed or failed workflow.
    pub fn reject(&mut self) {
        self.status = WorkflowStatus::Rejected;
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
    fn test_workflow_status_display_accepted() {
        assert_eq!(format!("{}", WorkflowStatus::Accepted), "accepted");
    }

    #[test]
    fn test_workflow_status_display_rejected() {
        assert_eq!(format!("{}", WorkflowStatus::Rejected), "rejected");
    }

    #[test]
    fn test_workflow_status_serialization() {
        let statuses = [
            WorkflowStatus::Pending,
            WorkflowStatus::Running,
            WorkflowStatus::Paused,
            WorkflowStatus::Completed,
            WorkflowStatus::Failed,
            WorkflowStatus::Accepted,
            WorkflowStatus::Rejected,
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
        assert_eq!(serde_json::to_string(&WorkflowStatus::Accepted).unwrap(), r#""accepted""#);
        assert_eq!(serde_json::to_string(&WorkflowStatus::Rejected).unwrap(), r#""rejected""#);
    }

    // TaskId tests

    #[test]
    fn test_task_id_new() {
        let id1 = TaskId::new();
        let id2 = TaskId::new();
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_task_id_default() {
        let id = TaskId::default();
        assert!(!id.0.is_nil());
    }

    #[test]
    fn test_task_id_display() {
        let id = TaskId::new();
        let display = format!("{}", id);
        assert_eq!(display, id.0.to_string());
    }

    #[test]
    fn test_task_id_serialization() {
        let id = TaskId::new();
        let json = serde_json::to_string(&id).unwrap();
        let parsed: TaskId = serde_json::from_str(&json).unwrap();
        assert_eq!(id, parsed);
    }

    // WorkflowConfig tests

    #[test]
    fn test_workflow_config_default() {
        let config = WorkflowConfig::default();
        assert!(config.update_docs);
        assert_eq!(config.max_parallel_agents, 4);
        assert_eq!(config.staging_branch_prefix, "zen/staging/");
    }

    #[test]
    fn test_workflow_config_custom_values() {
        let config = WorkflowConfig {
            update_docs: false,
            max_parallel_agents: 8,
            staging_branch_prefix: String::from("custom/staging/"),
        };
        assert!(!config.update_docs);
        assert_eq!(config.max_parallel_agents, 8);
        assert_eq!(config.staging_branch_prefix, "custom/staging/");
    }

    #[test]
    fn test_workflow_config_serialization() {
        let config = WorkflowConfig::default();
        let json = serde_json::to_string(&config).unwrap();
        let parsed: WorkflowConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(config.update_docs, parsed.update_docs);
        assert_eq!(config.max_parallel_agents, parsed.max_parallel_agents);
        assert_eq!(config.staging_branch_prefix, parsed.staging_branch_prefix);
    }

    // Workflow tests

    #[test]
    fn test_workflow_new() {
        let config = WorkflowConfig::default();
        let workflow = Workflow::new("build user authentication", config);

        assert!(!workflow.id.0.is_nil());
        assert_eq!(workflow.prompt, "build user authentication");
        assert_eq!(workflow.status, WorkflowStatus::Pending);
        assert_eq!(workflow.phase, WorkflowPhase::Planning);
        assert!(workflow.started_at.is_none());
        assert!(workflow.completed_at.is_none());
        assert!(workflow.task_ids.is_empty());
    }

    #[test]
    fn test_workflow_name_derivation() {
        let config = WorkflowConfig::default();
        let workflow = Workflow::new("build user authentication", config);
        assert_eq!(workflow.name, "build-user-authentication");
    }

    #[test]
    fn test_workflow_name_derivation_long_prompt() {
        let config = WorkflowConfig::default();
        let workflow = Workflow::new("create a complete user management system with roles", config);
        // Only takes first 4 words
        assert_eq!(workflow.name, "create-a-complete-user");
    }

    #[test]
    fn test_workflow_name_derivation_special_chars() {
        let config = WorkflowConfig::default();
        let workflow = Workflow::new("Build the API!", config);
        assert_eq!(workflow.name, "build-the-api");
    }

    #[test]
    fn test_workflow_start() {
        let config = WorkflowConfig::default();
        let mut workflow = Workflow::new("test prompt", config);

        assert_eq!(workflow.status, WorkflowStatus::Pending);
        assert!(workflow.started_at.is_none());

        workflow.start();

        assert_eq!(workflow.status, WorkflowStatus::Running);
        assert!(workflow.started_at.is_some());
    }

    #[test]
    fn test_workflow_complete() {
        let config = WorkflowConfig::default();
        let mut workflow = Workflow::new("test prompt", config);
        workflow.start();

        workflow.complete();

        assert_eq!(workflow.status, WorkflowStatus::Completed);
        assert_eq!(workflow.phase, WorkflowPhase::Complete);
        assert!(workflow.completed_at.is_some());
    }

    #[test]
    fn test_workflow_fail() {
        let config = WorkflowConfig::default();
        let mut workflow = Workflow::new("test prompt", config);
        workflow.start();

        workflow.fail();

        assert_eq!(workflow.status, WorkflowStatus::Failed);
        assert!(workflow.completed_at.is_some());
    }

    #[test]
    fn test_workflow_accept() {
        let config = WorkflowConfig::default();
        let mut workflow = Workflow::new("test prompt", config);
        workflow.start();
        workflow.complete();

        workflow.accept();

        assert_eq!(workflow.status, WorkflowStatus::Accepted);
    }

    #[test]
    fn test_workflow_reject() {
        let config = WorkflowConfig::default();
        let mut workflow = Workflow::new("test prompt", config);
        workflow.start();
        workflow.complete();

        workflow.reject();

        assert_eq!(workflow.status, WorkflowStatus::Rejected);
    }

    #[test]
    fn test_workflow_reject_after_fail() {
        let config = WorkflowConfig::default();
        let mut workflow = Workflow::new("test prompt", config);
        workflow.start();
        workflow.fail();

        // Can reject a failed workflow
        workflow.reject();

        assert_eq!(workflow.status, WorkflowStatus::Rejected);
    }

    #[test]
    fn test_workflow_with_custom_config() {
        let config = WorkflowConfig {
            update_docs: false,
            max_parallel_agents: 2,
            staging_branch_prefix: String::from("test/staging/"),
        };
        let workflow = Workflow::new("test prompt", config);

        assert!(!workflow.config.update_docs);
        assert_eq!(workflow.config.max_parallel_agents, 2);
        assert_eq!(workflow.config.staging_branch_prefix, "test/staging/");
    }

    #[test]
    fn test_workflow_serialization() {
        let config = WorkflowConfig::default();
        let workflow = Workflow::new("build user authentication", config);

        let json = serde_json::to_string(&workflow).unwrap();
        let parsed: Workflow = serde_json::from_str(&json).unwrap();

        assert_eq!(workflow.id, parsed.id);
        assert_eq!(workflow.name, parsed.name);
        assert_eq!(workflow.prompt, parsed.prompt);
        assert_eq!(workflow.status, parsed.status);
        assert_eq!(workflow.phase, parsed.phase);
    }

    #[test]
    fn test_workflow_serialization_has_tasks_field() {
        let config = WorkflowConfig::default();
        let workflow = Workflow::new("test", config);

        let json = serde_json::to_string(&workflow).unwrap();

        // Verify the field is named "tasks" not "task_ids" in JSON
        assert!(json.contains(r#""tasks":"#));
        assert!(!json.contains(r#""task_ids":"#));
    }
}
