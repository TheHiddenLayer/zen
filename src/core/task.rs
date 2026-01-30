//! Task data model for the execution DAG.
//!
//! Tasks are the atomic units of work assigned to agents. Each task
//! tracks its status, assignment, worktree location, and results.

use crate::agent::AgentId;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use uuid::Uuid;

/// Unique identifier for a task within a workflow.
///
/// Uses UUID v4 for generation and provides a short form display
/// for human-readable output.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct TaskId(pub Uuid);

impl TaskId {
    /// Create a new unique task identifier.
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    /// Return first 8 characters of the UUID for display.
    pub fn short(&self) -> String {
        self.0.to_string()[..8].to_string()
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

impl std::str::FromStr for TaskId {
    type Err = uuid::Error;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        Ok(Self(Uuid::parse_str(s)?))
    }
}

/// Task status in its lifecycle.
///
/// Tasks progress through these states as they are scheduled,
/// executed, and completed by agents.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "state")]
pub enum TaskStatus {
    /// Task created but not yet ready for execution.
    Pending,
    /// Task dependencies satisfied, ready to be scheduled.
    Ready,
    /// Task is currently being executed by an agent.
    Running,
    /// Task completed successfully.
    Completed,
    /// Task failed with an error.
    Failed {
        /// Error message describing the failure.
        error: String,
    },
    /// Task blocked and cannot proceed.
    Blocked {
        /// Reason why the task is blocked.
        reason: String,
    },
}

impl Default for TaskStatus {
    fn default() -> Self {
        Self::Pending
    }
}

impl std::fmt::Display for TaskStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TaskStatus::Pending => write!(f, "pending"),
            TaskStatus::Ready => write!(f, "ready"),
            TaskStatus::Running => write!(f, "running"),
            TaskStatus::Completed => write!(f, "completed"),
            TaskStatus::Failed { error } => write!(f, "failed: {}", error),
            TaskStatus::Blocked { reason } => write!(f, "blocked: {}", reason),
        }
    }
}

/// A single task in the execution DAG.
///
/// Tasks are the atomic units of work assigned to agents. They track
/// status, assignment, worktree location, timing, and results.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    /// Unique identifier for this task.
    pub id: TaskId,
    /// Human-readable name for the task.
    pub name: String,
    /// Detailed description of what the task should accomplish.
    pub description: String,
    /// Current execution status.
    pub status: TaskStatus,
    /// Path to the git worktree for this task.
    pub worktree_path: Option<PathBuf>,
    /// Name of the branch for this task.
    pub branch_name: Option<String>,
    /// ID of the agent assigned to this task.
    pub agent_id: Option<AgentId>,
    /// When the task was created.
    pub created_at: DateTime<Utc>,
    /// When the task started execution.
    pub started_at: Option<DateTime<Utc>>,
    /// When the task completed (success or failure).
    pub completed_at: Option<DateTime<Utc>>,
    /// Git commit hash of the task's work.
    pub commit_hash: Option<String>,
}

impl Task {
    /// Create a new task with the given name and description.
    ///
    /// The task is created with Pending status, a generated ID,
    /// and current timestamp. All optional fields are set to None.
    pub fn new(name: &str, description: &str) -> Self {
        Self {
            id: TaskId::new(),
            name: name.to_string(),
            description: description.to_string(),
            status: TaskStatus::Pending,
            worktree_path: None,
            branch_name: None,
            agent_id: None,
            created_at: Utc::now(),
            started_at: None,
            completed_at: None,
            commit_hash: None,
        }
    }

    /// Start the task execution.
    ///
    /// Transitions status to Running and records the start time.
    pub fn start(&mut self) {
        self.status = TaskStatus::Running;
        self.started_at = Some(Utc::now());
    }

    /// Mark the task as successfully completed.
    ///
    /// Transitions status to Completed and records the completion time.
    pub fn complete(&mut self) {
        self.status = TaskStatus::Completed;
        self.completed_at = Some(Utc::now());
    }

    /// Mark the task as failed with an error message.
    ///
    /// Transitions status to Failed and records the completion time.
    pub fn fail(&mut self, error: &str) {
        self.status = TaskStatus::Failed {
            error: error.to_string(),
        };
        self.completed_at = Some(Utc::now());
    }

    /// Mark the task as ready for execution.
    ///
    /// Transitions status from Pending to Ready when dependencies are satisfied.
    pub fn mark_ready(&mut self) {
        self.status = TaskStatus::Ready;
    }

    /// Mark the task as blocked.
    ///
    /// Transitions status to Blocked with a reason.
    pub fn block(&mut self, reason: &str) {
        self.status = TaskStatus::Blocked {
            reason: reason.to_string(),
        };
    }

    /// Assign an agent to this task.
    pub fn assign_agent(&mut self, agent_id: AgentId) {
        self.agent_id = Some(agent_id);
    }

    /// Set the worktree path for this task.
    pub fn set_worktree(&mut self, path: PathBuf, branch: &str) {
        self.worktree_path = Some(path);
        self.branch_name = Some(branch.to_string());
    }

    /// Record the commit hash from the task's work.
    pub fn set_commit(&mut self, hash: &str) {
        self.commit_hash = Some(hash.to_string());
    }

    /// Check if the task is in a terminal state (Completed or Failed).
    pub fn is_finished(&self) -> bool {
        matches!(
            self.status,
            TaskStatus::Completed | TaskStatus::Failed { .. }
        )
    }

    /// Check if the task can be started (Pending or Ready).
    pub fn can_start(&self) -> bool {
        matches!(self.status, TaskStatus::Pending | TaskStatus::Ready)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn test_task_id_short() {
        let id = TaskId::new();
        let short = id.short();
        assert_eq!(short.len(), 8);
    }

    #[test]
    fn test_task_id_display() {
        let id = TaskId::new();
        let display = format!("{}", id);
        assert_eq!(display, id.0.to_string());
    }

    #[test]
    fn test_task_id_from_str() {
        let id = TaskId::new();
        let s = id.to_string();
        let parsed: TaskId = s.parse().unwrap();
        assert_eq!(id, parsed);
    }

    #[test]
    fn test_task_id_from_str_invalid() {
        let result: std::result::Result<TaskId, _> = "invalid".parse();
        assert!(result.is_err());
    }

    #[test]
    fn test_task_id_serialization() {
        let id = TaskId::new();
        let json = serde_json::to_string(&id).unwrap();
        let parsed: TaskId = serde_json::from_str(&json).unwrap();
        assert_eq!(id, parsed);
    }

    #[test]
    fn test_task_id_equality() {
        let uuid = Uuid::new_v4();
        let id1 = TaskId(uuid);
        let id2 = TaskId(uuid);
        assert_eq!(id1, id2);
    }

    #[test]
    fn test_task_id_hash() {
        use std::collections::HashSet;

        let uuid = Uuid::new_v4();
        let id1 = TaskId(uuid);
        let id2 = TaskId(uuid);

        let mut set = HashSet::new();
        set.insert(id1);
        assert!(set.contains(&id2));
    }

    // TaskStatus tests

    #[test]
    fn test_task_status_default() {
        let status = TaskStatus::default();
        assert_eq!(status, TaskStatus::Pending);
    }

    #[test]
    fn test_task_status_display_pending() {
        let status = TaskStatus::Pending;
        assert_eq!(format!("{}", status), "pending");
    }

    #[test]
    fn test_task_status_display_ready() {
        let status = TaskStatus::Ready;
        assert_eq!(format!("{}", status), "ready");
    }

    #[test]
    fn test_task_status_display_running() {
        let status = TaskStatus::Running;
        assert_eq!(format!("{}", status), "running");
    }

    #[test]
    fn test_task_status_display_completed() {
        let status = TaskStatus::Completed;
        assert_eq!(format!("{}", status), "completed");
    }

    #[test]
    fn test_task_status_display_failed() {
        let status = TaskStatus::Failed {
            error: "connection timeout".to_string(),
        };
        assert_eq!(format!("{}", status), "failed: connection timeout");
    }

    #[test]
    fn test_task_status_display_blocked() {
        let status = TaskStatus::Blocked {
            reason: "waiting for dependency".to_string(),
        };
        assert_eq!(format!("{}", status), "blocked: waiting for dependency");
    }

    #[test]
    fn test_task_status_serialization_pending() {
        let status = TaskStatus::Pending;
        let json = serde_json::to_string(&status).unwrap();
        assert!(json.contains("pending"));
        let parsed: TaskStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(status, parsed);
    }

    #[test]
    fn test_task_status_serialization_failed() {
        let status = TaskStatus::Failed {
            error: "test error".to_string(),
        };
        let json = serde_json::to_string(&status).unwrap();
        assert!(json.contains("failed"));
        assert!(json.contains("test error"));
        let parsed: TaskStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(status, parsed);
    }

    #[test]
    fn test_task_status_serialization_blocked() {
        let status = TaskStatus::Blocked {
            reason: "blocked reason".to_string(),
        };
        let json = serde_json::to_string(&status).unwrap();
        assert!(json.contains("blocked"));
        assert!(json.contains("blocked reason"));
        let parsed: TaskStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(status, parsed);
    }

    #[test]
    fn test_task_status_failed_stores_error() {
        let error = "process exited with code 1".to_string();
        let status = TaskStatus::Failed {
            error: error.clone(),
        };
        if let TaskStatus::Failed { error: e } = status {
            assert_eq!(e, error);
        } else {
            panic!("Expected Failed variant");
        }
    }

    #[test]
    fn test_task_status_blocked_stores_reason() {
        let reason = "waiting for task-001".to_string();
        let status = TaskStatus::Blocked {
            reason: reason.clone(),
        };
        if let TaskStatus::Blocked { reason: r } = status {
            assert_eq!(r, reason);
        } else {
            panic!("Expected Blocked variant");
        }
    }

    // Task tests

    #[test]
    fn test_task_new() {
        let task = Task::new("create-user-model", "Create the user model");

        assert!(!task.id.0.is_nil());
        assert_eq!(task.name, "create-user-model");
        assert_eq!(task.description, "Create the user model");
        assert_eq!(task.status, TaskStatus::Pending);
        assert!(task.worktree_path.is_none());
        assert!(task.branch_name.is_none());
        assert!(task.agent_id.is_none());
        assert!(task.started_at.is_none());
        assert!(task.completed_at.is_none());
        assert!(task.commit_hash.is_none());
    }

    #[test]
    fn test_task_start() {
        let mut task = Task::new("test-task", "Test description");

        assert_eq!(task.status, TaskStatus::Pending);
        assert!(task.started_at.is_none());

        task.start();

        assert_eq!(task.status, TaskStatus::Running);
        assert!(task.started_at.is_some());
    }

    #[test]
    fn test_task_complete() {
        let mut task = Task::new("test-task", "Test description");
        task.start();

        task.complete();

        assert_eq!(task.status, TaskStatus::Completed);
        assert!(task.completed_at.is_some());
    }

    #[test]
    fn test_task_fail() {
        let mut task = Task::new("test-task", "Test description");
        task.start();

        task.fail("compilation error");

        assert!(matches!(task.status, TaskStatus::Failed { error } if error == "compilation error"));
        assert!(task.completed_at.is_some());
    }

    #[test]
    fn test_task_lifecycle_pending_to_running_to_completed() {
        let mut task = Task::new("test-task", "Test description");

        assert_eq!(task.status, TaskStatus::Pending);
        assert!(task.started_at.is_none());
        assert!(task.completed_at.is_none());

        task.start();

        assert_eq!(task.status, TaskStatus::Running);
        assert!(task.started_at.is_some());
        assert!(task.completed_at.is_none());

        task.complete();

        assert_eq!(task.status, TaskStatus::Completed);
        assert!(task.started_at.is_some());
        assert!(task.completed_at.is_some());

        // Verify timing order
        assert!(task.started_at.unwrap() <= task.completed_at.unwrap());
    }

    #[test]
    fn test_task_lifecycle_pending_to_running_to_failed() {
        let mut task = Task::new("test-task", "Test description");

        task.start();
        task.fail("test failed");

        assert!(matches!(task.status, TaskStatus::Failed { .. }));
        assert!(task.started_at.is_some());
        assert!(task.completed_at.is_some());
    }

    #[test]
    fn test_task_mark_ready() {
        let mut task = Task::new("test-task", "Test description");

        task.mark_ready();

        assert_eq!(task.status, TaskStatus::Ready);
    }

    #[test]
    fn test_task_block() {
        let mut task = Task::new("test-task", "Test description");

        task.block("dependency not met");

        assert!(matches!(task.status, TaskStatus::Blocked { reason } if reason == "dependency not met"));
    }

    #[test]
    fn test_task_assign_agent() {
        let mut task = Task::new("test-task", "Test description");
        let agent_id = AgentId::new();

        task.assign_agent(agent_id);

        assert_eq!(task.agent_id, Some(agent_id));
    }

    #[test]
    fn test_task_set_worktree() {
        let mut task = Task::new("test-task", "Test description");
        let path = PathBuf::from("/Users/test/.zen/worktrees/task-001");

        task.set_worktree(path.clone(), "zen/task/task-001");

        assert_eq!(task.worktree_path, Some(path));
        assert_eq!(task.branch_name, Some("zen/task/task-001".to_string()));
    }

    #[test]
    fn test_task_set_commit() {
        let mut task = Task::new("test-task", "Test description");

        task.set_commit("a1b2c3d4e5f6");

        assert_eq!(task.commit_hash, Some("a1b2c3d4e5f6".to_string()));
    }

    #[test]
    fn test_task_is_finished() {
        let mut task = Task::new("test-task", "Test description");

        assert!(!task.is_finished());

        task.start();
        assert!(!task.is_finished());

        task.complete();
        assert!(task.is_finished());
    }

    #[test]
    fn test_task_is_finished_on_failure() {
        let mut task = Task::new("test-task", "Test description");
        task.start();
        task.fail("error");

        assert!(task.is_finished());
    }

    #[test]
    fn test_task_can_start() {
        let mut task = Task::new("test-task", "Test description");

        assert!(task.can_start());

        task.mark_ready();
        assert!(task.can_start());

        task.start();
        assert!(!task.can_start());
    }

    #[test]
    fn test_task_serialization() {
        let mut task = Task::new("create-user-model", "Create User model");
        task.set_worktree(
            PathBuf::from("/Users/alice/.zen/worktrees/task-001"),
            "zen/task/task-001",
        );
        let agent_id = AgentId::new();
        task.assign_agent(agent_id);
        task.start();
        task.complete();
        task.set_commit("a1b2c3d4e5f6");

        let json = serde_json::to_string(&task).unwrap();
        let parsed: Task = serde_json::from_str(&json).unwrap();

        assert_eq!(task.id, parsed.id);
        assert_eq!(task.name, parsed.name);
        assert_eq!(task.description, parsed.description);
        assert_eq!(task.status, parsed.status);
        assert_eq!(task.worktree_path, parsed.worktree_path);
        assert_eq!(task.branch_name, parsed.branch_name);
        assert_eq!(task.agent_id, parsed.agent_id);
        assert_eq!(task.commit_hash, parsed.commit_hash);
    }

    #[test]
    fn test_task_serialization_json_format() {
        let task = Task::new("create-user-model", "Create User model");

        let json = serde_json::to_string_pretty(&task).unwrap();

        // Verify key fields are present in JSON
        assert!(json.contains("\"id\""));
        assert!(json.contains("\"name\""));
        assert!(json.contains("\"description\""));
        assert!(json.contains("\"status\""));
        assert!(json.contains("\"created_at\""));
        assert!(json.contains("create-user-model"));
        assert!(json.contains("Create User model"));
    }

    #[test]
    fn test_task_clone() {
        let task = Task::new("test-task", "Test description");
        let cloned = task.clone();

        assert_eq!(task.id, cloned.id);
        assert_eq!(task.name, cloned.name);
        assert_eq!(task.description, cloned.description);
        assert_eq!(task.status, cloned.status);
    }

    #[test]
    fn test_task_debug() {
        let task = Task::new("test-task", "Test description");
        let debug = format!("{:?}", task);
        assert!(debug.contains("Task"));
        assert!(debug.contains("test-task"));
    }
}
