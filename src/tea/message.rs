//! Messages for the TEA (The Elm Architecture) pattern.
//!
//! Messages are inputs to the update function - they come from external sources
//! like keyboard events, background actors, or command completion callbacks.

use crossterm::event::KeyEvent;

use crate::agent::AgentId;
use crate::core::task::TaskId;
use crate::session::{Session, SessionId};

/// Input messages to the update function.
#[derive(Debug)]
pub enum Message {
    // Keyboard/terminal events
    Key(KeyEvent),
    Resize(u16, u16),

    // From background actors
    /// Preview content updated with pane activity timestamp
    PreviewUpdated(SessionId, String, u64),
    PromptDetected(SessionId, bool, bool),
    /// Auto-approve prompt for a session (trust mode)
    AutoApprovePrompt(SessionId),

    // Command completion callbacks
    SessionCreated(Session),
    SessionCreateFailed(String, String),
    SessionDeleted(SessionId),
    SessionLocked(SessionId),
    SessionLockFailed(SessionId, String),
    SessionUnlocked(SessionId),
    SessionUnlockFailed(SessionId, String),

    // State persistence
    StateSaved,
    StateSaveFailed(String),

    // Task execution events (from Scheduler)
    /// A task has been started by an agent.
    TaskStarted {
        /// The task that was started.
        task_id: TaskId,
        /// The agent assigned to the task.
        agent_id: AgentId,
    },
    /// Progress update for task execution.
    TaskProgress {
        /// Number of completed tasks.
        completed: usize,
        /// Total number of tasks.
        total: usize,
    },
    /// A task completed successfully.
    TaskCompleted {
        /// The task that completed.
        task_id: TaskId,
        /// The commit hash from the task's work.
        commit: String,
    },
    /// A task failed with an error.
    TaskFailed {
        /// The task that failed.
        task_id: TaskId,
        /// Error message describing the failure.
        error: String,
    },
}
