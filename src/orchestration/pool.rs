//! Agent pool for multi-agent management.
//!
//! The `AgentPool` manages multiple concurrent agents, enforcing capacity
//! limits and providing lifecycle operations. It emits events for status
//! changes via a channel.

use crate::agent::{AgentId, AgentStatus};
use crate::error::Result;
use crate::tmux::Tmux;
use crate::workflow::TaskId;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

/// Events emitted by the agent pool for status changes.
///
/// These events allow external components to react to agent lifecycle
/// changes without polling.
#[derive(Debug, Clone)]
pub enum AgentEvent {
    /// An agent has started working on a task.
    Started {
        /// The agent that started.
        agent_id: AgentId,
        /// The task the agent is working on.
        task_id: TaskId,
    },
    /// An agent has completed its task.
    Completed {
        /// The agent that completed.
        agent_id: AgentId,
        /// The exit code from the agent process.
        exit_code: i32,
    },
    /// An agent has failed with an error.
    Failed {
        /// The agent that failed.
        agent_id: AgentId,
        /// Error message describing the failure.
        error: String,
    },
    /// An agent appears to be stuck.
    StuckDetected {
        /// The agent that appears stuck.
        agent_id: AgentId,
        /// How long the agent has been stuck.
        duration: Duration,
    },
    /// An agent was terminated.
    Terminated {
        /// The agent that was terminated.
        agent_id: AgentId,
    },
}

/// Output captured from an agent's tmux session.
///
/// This enum represents parsed output from an agent, categorized by content type.
/// The orchestrator uses this to determine how to respond to agent output.
#[derive(Debug, Clone, PartialEq)]
pub enum AgentOutput {
    /// Regular text output from the agent.
    Text(String),
    /// The agent is asking a question that needs a response.
    Question(String),
    /// The agent has completed its task successfully.
    Completed,
    /// The agent encountered an error.
    Error(String),
}

impl AgentOutput {
    /// Parse raw output content into an AgentOutput variant.
    ///
    /// Detection rules:
    /// - Questions: Contains "?" at end of line, "Do you want", "Would you like", etc.
    /// - Completed: Contains completion markers like "Task completed", "Done", etc.
    /// - Error: Contains error indicators like "Error:", "Failed:", etc.
    /// - Text: Default for all other output
    pub fn parse(content: &str) -> Self {
        let trimmed = content.trim();

        // Check for error patterns first (highest priority)
        if Self::contains_error_pattern(trimmed) {
            return AgentOutput::Error(trimmed.to_string());
        }

        // Check for completion patterns
        if Self::contains_completion_pattern(trimmed) {
            return AgentOutput::Completed;
        }

        // Check for question patterns
        if Self::contains_question_pattern(trimmed) {
            return AgentOutput::Question(trimmed.to_string());
        }

        // Default to text
        AgentOutput::Text(trimmed.to_string())
    }

    /// Check if content contains question patterns.
    fn contains_question_pattern(content: &str) -> bool {
        let lower = content.to_lowercase();

        // Check for question mark at end of a line
        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.ends_with('?') {
                return true;
            }
        }

        // Check for common question phrases
        let question_patterns = [
            "do you want",
            "would you like",
            "should i",
            "shall i",
            "can i",
            "may i",
            "please confirm",
            "please select",
            "choose one",
            "select an option",
            "enter your",
            "type your",
        ];

        question_patterns.iter().any(|p| lower.contains(p))
    }

    /// Check if content contains completion patterns.
    fn contains_completion_pattern(content: &str) -> bool {
        let lower = content.to_lowercase();

        let completion_patterns = [
            "task completed",
            "successfully completed",
            "all tests pass",
            "build successful",
            "implementation complete",
            "done!",
            "finished!",
            "complete!",
            "✓ all done",
            "✅ done",
        ];

        completion_patterns.iter().any(|p| lower.contains(p))
    }

    /// Check if content contains error patterns.
    fn contains_error_pattern(content: &str) -> bool {
        let lower = content.to_lowercase();

        let error_patterns = [
            "error:",
            "failed:",
            "failure:",
            "fatal:",
            "panic:",
            "exception:",
            "could not",
            "cannot ",
            "unable to",
            "❌",
            "✗",
        ];

        error_patterns.iter().any(|p| lower.contains(p))
    }
}

/// A handle to an agent in the pool.
///
/// Wraps a running agent instance, providing methods to send input and
/// read output from the agent's tmux session. Each agent runs in its own
/// tmux session and git worktree for isolation.
#[derive(Debug)]
pub struct AgentHandle {
    /// Unique identifier for this agent.
    pub id: AgentId,
    /// Current status of the agent.
    pub status: AgentStatus,
    /// The task this agent is assigned to (if any).
    pub task_id: Option<TaskId>,
    /// Name of the tmux session for this agent.
    pub tmux_session: String,
    /// Path to the agent's isolated git worktree.
    pub worktree_path: PathBuf,
    /// When the agent was started.
    pub started_at: Instant,
    /// Last time there was activity from this agent.
    pub last_activity: Instant,
    /// Cancellation token for graceful shutdown.
    cancel_token: CancellationToken,
}

impl Clone for AgentHandle {
    fn clone(&self) -> Self {
        Self {
            id: self.id,
            status: self.status.clone(),
            task_id: self.task_id,
            tmux_session: self.tmux_session.clone(),
            worktree_path: self.worktree_path.clone(),
            started_at: self.started_at,
            last_activity: self.last_activity,
            cancel_token: self.cancel_token.clone(),
        }
    }
}

impl AgentHandle {
    /// Create a new agent handle with the given ID.
    ///
    /// Creates an idle agent with default tmux session name derived from the agent ID.
    pub fn new(id: AgentId) -> Self {
        let now = Instant::now();
        Self {
            id,
            status: AgentStatus::Idle,
            task_id: None,
            tmux_session: format!("zen_agent_{}", id.short()),
            worktree_path: PathBuf::new(),
            started_at: now,
            last_activity: now,
            cancel_token: CancellationToken::new(),
        }
    }

    /// Create an agent handle that is running a task.
    ///
    /// # Arguments
    ///
    /// * `id` - The agent's unique identifier
    /// * `task_id` - The task this agent is assigned to
    pub fn with_task(id: AgentId, task_id: TaskId) -> Self {
        let now = Instant::now();
        Self {
            id,
            status: AgentStatus::Running { task_id },
            task_id: Some(task_id),
            tmux_session: format!("zen_agent_{}", id.short()),
            worktree_path: PathBuf::new(),
            started_at: now,
            last_activity: now,
            cancel_token: CancellationToken::new(),
        }
    }

    /// Create an agent handle with full configuration.
    ///
    /// # Arguments
    ///
    /// * `id` - The agent's unique identifier
    /// * `task_id` - The task this agent is assigned to
    /// * `tmux_session` - Name of the tmux session
    /// * `worktree_path` - Path to the agent's worktree
    pub fn with_config(
        id: AgentId,
        task_id: TaskId,
        tmux_session: String,
        worktree_path: PathBuf,
    ) -> Self {
        let now = Instant::now();
        Self {
            id,
            status: AgentStatus::Running { task_id },
            task_id: Some(task_id),
            tmux_session,
            worktree_path,
            started_at: now,
            last_activity: now,
            cancel_token: CancellationToken::new(),
        }
    }

    /// Get the path to the agent's worktree.
    pub fn worktree_path(&self) -> &Path {
        &self.worktree_path
    }

    /// Get the tmux session name.
    pub fn tmux_session(&self) -> &str {
        &self.tmux_session
    }

    /// Get a reference to the cancellation token.
    pub fn cancel_token(&self) -> &CancellationToken {
        &self.cancel_token
    }

    /// Check if this agent has been cancelled.
    pub fn is_cancelled(&self) -> bool {
        self.cancel_token.is_cancelled()
    }

    /// Cancel this agent's operation.
    pub fn cancel(&self) {
        self.cancel_token.cancel();
    }

    /// Send input to the agent's tmux session.
    ///
    /// Sends the input string followed by Enter to the agent's tmux pane.
    ///
    /// # Arguments
    ///
    /// * `input` - The text to send to the agent
    ///
    /// # Errors
    ///
    /// Returns an error if the tmux send-keys command fails.
    pub fn send(&self, input: &str) -> Result<()> {
        Tmux::send_keys_enter(&self.tmux_session, input)
    }

    /// Read and parse output from the agent's tmux session.
    ///
    /// Captures the current pane content and parses it to determine
    /// if the agent is asking a question, has completed, or has an error.
    ///
    /// # Returns
    ///
    /// An `AgentOutput` variant based on the parsed content.
    ///
    /// # Errors
    ///
    /// Returns an error if capturing the tmux pane fails.
    pub fn read_output(&self) -> Result<AgentOutput> {
        let content = Tmux::capture_pane_plain(&self.tmux_session)?;
        Ok(AgentOutput::parse(&content))
    }

    /// Read raw output from the agent's tmux session without parsing.
    ///
    /// # Returns
    ///
    /// The raw pane content as a string.
    ///
    /// # Errors
    ///
    /// Returns an error if capturing the tmux pane fails.
    pub fn read_raw_output(&self) -> Result<String> {
        Tmux::capture_pane_plain(&self.tmux_session)
    }

    /// Read the last N lines of output from the agent.
    ///
    /// This is more efficient than capturing the entire pane and helps
    /// avoid false positives from historical output.
    ///
    /// # Arguments
    ///
    /// * `lines` - Number of lines to capture
    ///
    /// # Returns
    ///
    /// An `AgentOutput` variant based on the parsed content.
    ///
    /// # Errors
    ///
    /// Returns an error if capturing the tmux pane fails.
    pub fn read_output_tail(&self, lines: u16) -> Result<AgentOutput> {
        let content = Tmux::capture_pane_tail(&self.tmux_session, lines)?;
        Ok(AgentOutput::parse(&content))
    }

    /// Get the last commit SHA from the agent's worktree.
    ///
    /// # Returns
    ///
    /// `Some(sha)` if the worktree exists and has commits, `None` otherwise.
    ///
    /// # Errors
    ///
    /// Returns an error if git operations fail.
    pub fn last_commit(&self) -> Result<Option<String>> {
        if !self.worktree_path.exists() {
            return Ok(None);
        }

        let output = std::process::Command::new("git")
            .args(["rev-parse", "HEAD"])
            .current_dir(&self.worktree_path)
            .output()?;

        if output.status.success() {
            let sha = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if sha.is_empty() {
                Ok(None)
            } else {
                Ok(Some(sha))
            }
        } else {
            Ok(None)
        }
    }

    /// Update the last activity timestamp to now.
    pub fn touch_activity(&mut self) {
        self.last_activity = Instant::now();
    }

    /// Get how long since the last activity.
    pub fn idle_duration(&self) -> Duration {
        self.last_activity.elapsed()
    }

    /// Get how long the agent has been running.
    pub fn running_duration(&self) -> Duration {
        self.started_at.elapsed()
    }
}

/// Manages a pool of concurrent agents.
///
/// The `AgentPool` tracks all active agents, enforces the max_concurrent
/// limit, and emits events for status changes via a channel.
///
/// # Example
///
/// ```ignore
/// use tokio::sync::mpsc;
/// use zen::orchestration::{AgentPool, AgentEvent};
///
/// let (tx, mut rx) = mpsc::channel(100);
/// let mut pool = AgentPool::new(3, tx);
///
/// assert!(pool.has_capacity());
/// assert_eq!(pool.active_count(), 0);
/// ```
pub struct AgentPool {
    /// Active agents indexed by their ID.
    agents: HashMap<AgentId, AgentHandle>,
    /// Maximum number of concurrent agents allowed.
    max_concurrent: usize,
    /// Channel for emitting agent events.
    event_tx: mpsc::Sender<AgentEvent>,
}

impl AgentPool {
    /// Create a new agent pool with the given capacity.
    ///
    /// # Arguments
    ///
    /// * `max_concurrent` - Maximum number of agents that can run simultaneously
    /// * `event_tx` - Channel sender for emitting agent events
    pub fn new(max_concurrent: usize, event_tx: mpsc::Sender<AgentEvent>) -> Self {
        Self {
            agents: HashMap::new(),
            max_concurrent,
            event_tx,
        }
    }

    /// Spawn a new agent for a task.
    ///
    /// This is a stub implementation. The full implementation will be
    /// added in Task 4.3 when AgentHandle is fully implemented.
    ///
    /// # Arguments
    ///
    /// * `task_id` - The task to assign to the agent
    /// * `_skill` - The skill the agent should run (unused in stub)
    ///
    /// # Returns
    ///
    /// The ID of the spawned agent, or an error if at capacity.
    ///
    /// # Errors
    ///
    /// Returns an error if the pool is at capacity.
    pub async fn spawn(&mut self, task_id: &TaskId, _skill: &str) -> Result<AgentId> {
        if !self.has_capacity() {
            return Err(crate::error::Error::AgentPoolFull {
                max: self.max_concurrent,
            });
        }

        let agent_id = AgentId::new();
        let handle = AgentHandle::with_task(agent_id, *task_id);
        self.agents.insert(agent_id, handle);

        // Emit started event
        let _ = self
            .event_tx
            .send(AgentEvent::Started {
                agent_id,
                task_id: *task_id,
            })
            .await;

        Ok(agent_id)
    }

    /// Terminate an agent by ID.
    ///
    /// Removes the agent from the pool and emits a Terminated event.
    ///
    /// # Arguments
    ///
    /// * `id` - The ID of the agent to terminate
    ///
    /// # Returns
    ///
    /// Ok(()) if the agent was found and removed, Err if not found.
    pub async fn terminate(&mut self, id: &AgentId) -> Result<()> {
        if self.agents.remove(id).is_none() {
            return Err(crate::error::Error::AgentNotFound { id: *id });
        }

        // Emit terminated event
        let _ = self
            .event_tx
            .send(AgentEvent::Terminated { agent_id: *id })
            .await;

        Ok(())
    }

    /// Get an agent by ID.
    ///
    /// # Arguments
    ///
    /// * `id` - The ID of the agent to retrieve
    ///
    /// # Returns
    ///
    /// A reference to the agent handle, or None if not found.
    pub fn get(&self, id: &AgentId) -> Option<&AgentHandle> {
        self.agents.get(id)
    }

    /// Get the number of active agents in the pool.
    pub fn active_count(&self) -> usize {
        self.agents.len()
    }

    /// Check if the pool has capacity for more agents.
    pub fn has_capacity(&self) -> bool {
        self.active_count() < self.max_concurrent
    }

    /// Get the maximum concurrent agents allowed.
    pub fn max_concurrent(&self) -> usize {
        self.max_concurrent
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Helper to create a pool with a receiver for testing
    fn create_test_pool(max_concurrent: usize) -> (AgentPool, mpsc::Receiver<AgentEvent>) {
        let (tx, rx) = mpsc::channel(100);
        let pool = AgentPool::new(max_concurrent, tx);
        (pool, rx)
    }

    // AgentEvent tests

    #[test]
    fn test_agent_event_started_has_required_fields() {
        let agent_id = AgentId::new();
        let task_id = TaskId::new();
        let event = AgentEvent::Started { agent_id, task_id };

        if let AgentEvent::Started {
            agent_id: aid,
            task_id: tid,
        } = event
        {
            assert_eq!(aid, agent_id);
            assert_eq!(tid, task_id);
        } else {
            panic!("Expected Started variant");
        }
    }

    #[test]
    fn test_agent_event_completed_has_exit_code() {
        let agent_id = AgentId::new();
        let event = AgentEvent::Completed {
            agent_id,
            exit_code: 0,
        };

        if let AgentEvent::Completed {
            agent_id: aid,
            exit_code,
        } = event
        {
            assert_eq!(aid, agent_id);
            assert_eq!(exit_code, 0);
        } else {
            panic!("Expected Completed variant");
        }
    }

    #[test]
    fn test_agent_event_failed_has_error() {
        let agent_id = AgentId::new();
        let event = AgentEvent::Failed {
            agent_id,
            error: "test error".to_string(),
        };

        if let AgentEvent::Failed {
            agent_id: aid,
            error,
        } = event
        {
            assert_eq!(aid, agent_id);
            assert_eq!(error, "test error");
        } else {
            panic!("Expected Failed variant");
        }
    }

    #[test]
    fn test_agent_event_stuck_detected_has_duration() {
        let agent_id = AgentId::new();
        let duration = Duration::from_secs(300);
        let event = AgentEvent::StuckDetected { agent_id, duration };

        if let AgentEvent::StuckDetected {
            agent_id: aid,
            duration: dur,
        } = event
        {
            assert_eq!(aid, agent_id);
            assert_eq!(dur, duration);
        } else {
            panic!("Expected StuckDetected variant");
        }
    }

    #[test]
    fn test_agent_event_terminated() {
        let agent_id = AgentId::new();
        let event = AgentEvent::Terminated { agent_id };

        if let AgentEvent::Terminated { agent_id: aid } = event {
            assert_eq!(aid, agent_id);
        } else {
            panic!("Expected Terminated variant");
        }
    }

    #[test]
    fn test_agent_event_debug_format() {
        let agent_id = AgentId::new();
        let event = AgentEvent::Started {
            agent_id,
            task_id: TaskId::new(),
        };
        let debug = format!("{:?}", event);
        assert!(debug.contains("Started"));
    }

    #[test]
    fn test_agent_event_clone() {
        let agent_id = AgentId::new();
        let event = AgentEvent::Failed {
            agent_id,
            error: "test".to_string(),
        };
        let cloned = event.clone();
        if let AgentEvent::Failed { error, .. } = cloned {
            assert_eq!(error, "test");
        } else {
            panic!("Expected Failed variant");
        }
    }

    // AgentHandle tests

    #[test]
    fn test_agent_handle_new() {
        let id = AgentId::new();
        let handle = AgentHandle::new(id);
        assert_eq!(handle.id, id);
        assert!(matches!(handle.status, AgentStatus::Idle));
        assert!(handle.task_id.is_none());
    }

    #[test]
    fn test_agent_handle_with_task() {
        let id = AgentId::new();
        let task_id = TaskId::new();
        let handle = AgentHandle::with_task(id, task_id);
        assert_eq!(handle.id, id);
        assert!(matches!(handle.status, AgentStatus::Running { .. }));
        assert_eq!(handle.task_id, Some(task_id));
    }

    #[test]
    fn test_agent_handle_debug() {
        let handle = AgentHandle::new(AgentId::new());
        let debug = format!("{:?}", handle);
        assert!(debug.contains("AgentHandle"));
    }

    #[test]
    fn test_agent_handle_clone() {
        let handle = AgentHandle::new(AgentId::new());
        let cloned = handle.clone();
        assert_eq!(handle.id, cloned.id);
    }

    // AgentPool creation tests

    #[test]
    fn test_agent_pool_new() {
        let (pool, _rx) = create_test_pool(3);
        assert_eq!(pool.max_concurrent(), 3);
    }

    #[test]
    fn test_agent_pool_new_with_capacity_zero() {
        let (pool, _rx) = create_test_pool(0);
        assert_eq!(pool.max_concurrent(), 0);
        assert!(!pool.has_capacity());
    }

    #[test]
    fn test_agent_pool_starts_empty() {
        let (pool, _rx) = create_test_pool(3);
        assert_eq!(pool.active_count(), 0);
    }

    // Capacity tests

    #[test]
    fn test_has_capacity_when_empty() {
        let (pool, _rx) = create_test_pool(3);
        assert!(pool.has_capacity());
    }

    #[tokio::test]
    async fn test_has_capacity_below_limit() {
        let (mut pool, _rx) = create_test_pool(3);
        let task_id = TaskId::new();
        pool.spawn(&task_id, "test").await.unwrap();
        assert!(pool.has_capacity());
    }

    #[tokio::test]
    async fn test_has_capacity_at_limit() {
        let (mut pool, _rx) = create_test_pool(3);
        for _ in 0..3 {
            let task_id = TaskId::new();
            pool.spawn(&task_id, "test").await.unwrap();
        }
        assert!(!pool.has_capacity());
    }

    #[test]
    fn test_active_count_empty() {
        let (pool, _rx) = create_test_pool(3);
        assert_eq!(pool.active_count(), 0);
    }

    #[tokio::test]
    async fn test_active_count_with_agents() {
        let (mut pool, _rx) = create_test_pool(5);
        for _ in 0..3 {
            let task_id = TaskId::new();
            pool.spawn(&task_id, "test").await.unwrap();
        }
        assert_eq!(pool.active_count(), 3);
    }

    // Agent lifecycle tests

    #[tokio::test]
    async fn test_spawn_returns_agent_id() {
        let (mut pool, _rx) = create_test_pool(3);
        let task_id = TaskId::new();
        let result = pool.spawn(&task_id, "test").await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_spawn_adds_to_pool() {
        let (mut pool, _rx) = create_test_pool(3);
        let task_id = TaskId::new();
        let agent_id = pool.spawn(&task_id, "test").await.unwrap();
        assert!(pool.get(&agent_id).is_some());
    }

    #[tokio::test]
    async fn test_spawn_sends_started_event() {
        let (mut pool, mut rx) = create_test_pool(3);
        let task_id = TaskId::new();
        let agent_id = pool.spawn(&task_id, "test").await.unwrap();

        let event = rx.recv().await.unwrap();
        if let AgentEvent::Started {
            agent_id: aid,
            task_id: tid,
        } = event
        {
            assert_eq!(aid, agent_id);
            assert_eq!(tid, task_id);
        } else {
            panic!("Expected Started event");
        }
    }

    #[tokio::test]
    async fn test_spawn_respects_capacity() {
        let (mut pool, _rx) = create_test_pool(2);
        let task1 = TaskId::new();
        let task2 = TaskId::new();
        let task3 = TaskId::new();

        pool.spawn(&task1, "test").await.unwrap();
        pool.spawn(&task2, "test").await.unwrap();
        let result = pool.spawn(&task3, "test").await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_terminate_removes_agent() {
        let (mut pool, _rx) = create_test_pool(3);
        let task_id = TaskId::new();
        let agent_id = pool.spawn(&task_id, "test").await.unwrap();

        pool.terminate(&agent_id).await.unwrap();
        assert!(pool.get(&agent_id).is_none());
    }

    #[tokio::test]
    async fn test_terminate_sends_event() {
        let (mut pool, mut rx) = create_test_pool(3);
        let task_id = TaskId::new();
        let agent_id = pool.spawn(&task_id, "test").await.unwrap();

        // Consume the Started event
        rx.recv().await.unwrap();

        pool.terminate(&agent_id).await.unwrap();

        let event = rx.recv().await.unwrap();
        if let AgentEvent::Terminated { agent_id: aid } = event {
            assert_eq!(aid, agent_id);
        } else {
            panic!("Expected Terminated event");
        }
    }

    #[tokio::test]
    async fn test_terminate_decreases_count() {
        let (mut pool, _rx) = create_test_pool(3);
        let task_id = TaskId::new();
        let agent_id = pool.spawn(&task_id, "test").await.unwrap();
        assert_eq!(pool.active_count(), 1);

        pool.terminate(&agent_id).await.unwrap();
        assert_eq!(pool.active_count(), 0);
    }

    #[tokio::test]
    async fn test_terminate_nonexistent_returns_error() {
        let (mut pool, _rx) = create_test_pool(3);
        let fake_id = AgentId::new();
        let result = pool.terminate(&fake_id).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_get_returns_agent() {
        let (mut pool, _rx) = create_test_pool(3);
        let task_id = TaskId::new();
        let agent_id = pool.spawn(&task_id, "test").await.unwrap();

        let handle = pool.get(&agent_id).unwrap();
        assert_eq!(handle.id, agent_id);
        assert_eq!(handle.task_id, Some(task_id));
    }

    #[test]
    fn test_get_nonexistent_returns_none() {
        let (pool, _rx) = create_test_pool(3);
        let fake_id = AgentId::new();
        assert!(pool.get(&fake_id).is_none());
    }

    // Additional tests for acceptance criteria

    #[tokio::test]
    async fn test_capacity_enforcement_given_max_3() {
        // Given AgentPool with max_concurrent=3
        let (mut pool, _rx) = create_test_pool(3);

        // When 3 agents are active
        for _ in 0..3 {
            pool.spawn(&TaskId::new(), "test").await.unwrap();
        }

        // Then has_capacity() returns false
        assert!(!pool.has_capacity());
    }

    #[tokio::test]
    async fn test_agent_tracking_via_get() {
        // Given agents spawned via the pool
        let (mut pool, _rx) = create_test_pool(5);
        let task_id = TaskId::new();
        let agent_id = pool.spawn(&task_id, "test").await.unwrap();

        // When get(id) is called
        let handle = pool.get(&agent_id);

        // Then the correct AgentHandle is returned
        assert!(handle.is_some());
        let handle = handle.unwrap();
        assert_eq!(handle.id, agent_id);
        assert_eq!(handle.task_id, Some(task_id));
    }

    #[tokio::test]
    async fn test_active_count_returns_correct_value() {
        // Given 2 active agents in the pool
        let (mut pool, _rx) = create_test_pool(5);
        pool.spawn(&TaskId::new(), "test").await.unwrap();
        pool.spawn(&TaskId::new(), "test").await.unwrap();

        // When active_count() is called
        let count = pool.active_count();

        // Then 2 is returned
        assert_eq!(count, 2);
    }

    #[tokio::test]
    async fn test_terminate_cleanup() {
        // Given an active agent
        let (mut pool, _rx) = create_test_pool(3);
        let agent_id = pool.spawn(&TaskId::new(), "test").await.unwrap();
        let initial_count = pool.active_count();

        // When terminate(id) is called
        pool.terminate(&agent_id).await.unwrap();

        // Then agent is removed and active_count decreases
        assert!(pool.get(&agent_id).is_none());
        assert_eq!(pool.active_count(), initial_count - 1);
    }

    #[tokio::test]
    async fn test_event_emission_on_spawn() {
        // Given an agent state change (spawn)
        let (mut pool, mut rx) = create_test_pool(3);
        let task_id = TaskId::new();

        // When the change occurs
        let agent_id = pool.spawn(&task_id, "test").await.unwrap();

        // Then appropriate AgentEvent is sent via event_tx
        let event = rx.recv().await.unwrap();
        assert!(matches!(
            event,
            AgentEvent::Started {
                agent_id: aid,
                task_id: tid
            } if aid == agent_id && tid == task_id
        ));
    }

    // ========== AgentOutput Tests ==========

    #[test]
    fn test_agent_output_text_variant() {
        let output = AgentOutput::Text("Hello world".to_string());
        if let AgentOutput::Text(content) = output {
            assert_eq!(content, "Hello world");
        } else {
            panic!("Expected Text variant");
        }
    }

    #[test]
    fn test_agent_output_question_variant() {
        let output = AgentOutput::Question("What is your name?".to_string());
        if let AgentOutput::Question(q) = output {
            assert_eq!(q, "What is your name?");
        } else {
            panic!("Expected Question variant");
        }
    }

    #[test]
    fn test_agent_output_completed_variant() {
        let output = AgentOutput::Completed;
        assert!(matches!(output, AgentOutput::Completed));
    }

    #[test]
    fn test_agent_output_error_variant() {
        let output = AgentOutput::Error("Something went wrong".to_string());
        if let AgentOutput::Error(msg) = output {
            assert_eq!(msg, "Something went wrong");
        } else {
            panic!("Expected Error variant");
        }
    }

    #[test]
    fn test_agent_output_debug_format() {
        let output = AgentOutput::Text("test".to_string());
        let debug = format!("{:?}", output);
        assert!(debug.contains("Text"));
    }

    #[test]
    fn test_agent_output_clone() {
        let output = AgentOutput::Question("Clone me?".to_string());
        let cloned = output.clone();
        assert_eq!(output, cloned);
    }

    #[test]
    fn test_agent_output_equality() {
        let a = AgentOutput::Completed;
        let b = AgentOutput::Completed;
        assert_eq!(a, b);

        let c = AgentOutput::Text("foo".to_string());
        let d = AgentOutput::Text("foo".to_string());
        assert_eq!(c, d);

        let e = AgentOutput::Text("foo".to_string());
        let f = AgentOutput::Text("bar".to_string());
        assert_ne!(e, f);
    }

    // ========== AgentOutput Parsing Tests ==========

    #[test]
    fn test_parse_output_detects_question_mark() {
        let content = "What would you like me to do?";
        let output = AgentOutput::parse(content);
        assert!(matches!(output, AgentOutput::Question(_)));
    }

    #[test]
    fn test_parse_output_detects_do_you_want() {
        let content = "Do you want me to continue with the implementation";
        let output = AgentOutput::parse(content);
        assert!(matches!(output, AgentOutput::Question(_)));
    }

    #[test]
    fn test_parse_output_detects_would_you_like() {
        let content = "Would you like me to add tests";
        let output = AgentOutput::parse(content);
        assert!(matches!(output, AgentOutput::Question(_)));
    }

    #[test]
    fn test_parse_output_detects_should_i() {
        let content = "Should I proceed with the refactoring";
        let output = AgentOutput::parse(content);
        assert!(matches!(output, AgentOutput::Question(_)));
    }

    #[test]
    fn test_parse_output_detects_completion_task_completed() {
        let content = "Task completed successfully";
        let output = AgentOutput::parse(content);
        assert!(matches!(output, AgentOutput::Completed));
    }

    #[test]
    fn test_parse_output_detects_completion_all_tests_pass() {
        let content = "Running tests...\nAll tests pass!";
        let output = AgentOutput::parse(content);
        assert!(matches!(output, AgentOutput::Completed));
    }

    #[test]
    fn test_parse_output_detects_completion_done() {
        let content = "Implementation is Done!";
        let output = AgentOutput::parse(content);
        assert!(matches!(output, AgentOutput::Completed));
    }

    #[test]
    fn test_parse_output_detects_error_pattern() {
        let content = "Error: file not found";
        let output = AgentOutput::parse(content);
        assert!(matches!(output, AgentOutput::Error(_)));
    }

    #[test]
    fn test_parse_output_detects_failed_pattern() {
        let content = "Failed: compilation error";
        let output = AgentOutput::parse(content);
        assert!(matches!(output, AgentOutput::Error(_)));
    }

    #[test]
    fn test_parse_output_detects_cannot_pattern() {
        let content = "Cannot find the specified module";
        let output = AgentOutput::parse(content);
        assert!(matches!(output, AgentOutput::Error(_)));
    }

    #[test]
    fn test_parse_output_returns_text_default() {
        let content = "Working on the implementation...";
        let output = AgentOutput::parse(content);
        assert!(matches!(output, AgentOutput::Text(_)));
    }

    #[test]
    fn test_parse_output_trims_whitespace() {
        let content = "  \n  Hello world  \n  ";
        let output = AgentOutput::parse(content);
        if let AgentOutput::Text(text) = output {
            assert_eq!(text, "Hello world");
        } else {
            panic!("Expected Text variant");
        }
    }

    #[test]
    fn test_parse_output_error_priority_over_question() {
        // Error should be detected even if there's also a question mark
        let content = "Error: Failed to process your request?";
        let output = AgentOutput::parse(content);
        assert!(matches!(output, AgentOutput::Error(_)));
    }

    #[test]
    fn test_parse_output_multiline_question() {
        let content = "I've analyzed the code.\nWould you like me to proceed?";
        let output = AgentOutput::parse(content);
        assert!(matches!(output, AgentOutput::Question(_)));
    }

    // ========== Enhanced AgentHandle Tests ==========

    #[test]
    fn test_agent_handle_has_tmux_session() {
        let id = AgentId::new();
        let handle = AgentHandle::new(id);
        assert!(handle.tmux_session.starts_with("zen_agent_"));
        assert!(handle.tmux_session.contains(&id.short()));
    }

    #[test]
    fn test_agent_handle_has_worktree_path() {
        let id = AgentId::new();
        let handle = AgentHandle::new(id);
        // New handle has empty worktree path
        assert!(handle.worktree_path.as_os_str().is_empty());
    }

    #[test]
    fn test_agent_handle_worktree_path_accessor() {
        let id = AgentId::new();
        let task_id = TaskId::new();
        let handle = AgentHandle::with_config(
            id,
            task_id,
            "test_session".to_string(),
            PathBuf::from("/tmp/worktree"),
        );
        assert_eq!(handle.worktree_path(), Path::new("/tmp/worktree"));
    }

    #[test]
    fn test_agent_handle_tmux_session_accessor() {
        let id = AgentId::new();
        let task_id = TaskId::new();
        let handle = AgentHandle::with_config(
            id,
            task_id,
            "my_session".to_string(),
            PathBuf::new(),
        );
        assert_eq!(handle.tmux_session(), "my_session");
    }

    #[test]
    fn test_agent_handle_has_started_at() {
        let id = AgentId::new();
        let handle = AgentHandle::new(id);
        // started_at should be very recent
        assert!(handle.started_at.elapsed().as_millis() < 100);
    }

    #[test]
    fn test_agent_handle_has_last_activity() {
        let id = AgentId::new();
        let handle = AgentHandle::new(id);
        // last_activity should be very recent
        assert!(handle.last_activity.elapsed().as_millis() < 100);
    }

    #[test]
    fn test_agent_handle_cancel_token() {
        let id = AgentId::new();
        let handle = AgentHandle::new(id);
        assert!(!handle.is_cancelled());
        handle.cancel();
        assert!(handle.is_cancelled());
    }

    #[test]
    fn test_agent_handle_cancel_token_accessor() {
        let id = AgentId::new();
        let handle = AgentHandle::new(id);
        let token = handle.cancel_token();
        assert!(!token.is_cancelled());
    }

    #[test]
    fn test_agent_handle_with_config() {
        let id = AgentId::new();
        let task_id = TaskId::new();
        let handle = AgentHandle::with_config(
            id,
            task_id,
            "custom_session".to_string(),
            PathBuf::from("/custom/path"),
        );

        assert_eq!(handle.id, id);
        assert_eq!(handle.task_id, Some(task_id));
        assert_eq!(handle.tmux_session, "custom_session");
        assert_eq!(handle.worktree_path, PathBuf::from("/custom/path"));
        assert!(matches!(handle.status, AgentStatus::Running { .. }));
    }

    #[test]
    fn test_agent_handle_idle_duration() {
        let id = AgentId::new();
        let handle = AgentHandle::new(id);
        // Should be very small since we just created it
        assert!(handle.idle_duration().as_millis() < 100);
    }

    #[test]
    fn test_agent_handle_running_duration() {
        let id = AgentId::new();
        let handle = AgentHandle::new(id);
        // Should be very small since we just created it
        assert!(handle.running_duration().as_millis() < 100);
    }

    #[test]
    fn test_agent_handle_touch_activity() {
        let id = AgentId::new();
        let mut handle = AgentHandle::new(id);
        let original = handle.last_activity;

        // Wait a tiny bit
        std::thread::sleep(std::time::Duration::from_millis(5));

        handle.touch_activity();

        // last_activity should have been updated
        assert!(handle.last_activity > original);
    }

    #[test]
    fn test_agent_handle_last_commit_nonexistent_path() {
        let id = AgentId::new();
        let task_id = TaskId::new();
        let handle = AgentHandle::with_config(
            id,
            task_id,
            "session".to_string(),
            PathBuf::from("/nonexistent/path/that/does/not/exist"),
        );

        let result = handle.last_commit();
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }

    #[test]
    fn test_agent_handle_with_task_sets_tmux_session() {
        let id = AgentId::new();
        let task_id = TaskId::new();
        let handle = AgentHandle::with_task(id, task_id);

        assert!(handle.tmux_session.starts_with("zen_agent_"));
        assert!(handle.tmux_session.contains(&id.short()));
    }

    #[test]
    fn test_agent_handle_clone_preserves_fields() {
        let id = AgentId::new();
        let task_id = TaskId::new();
        let handle = AgentHandle::with_config(
            id,
            task_id,
            "session".to_string(),
            PathBuf::from("/path"),
        );

        let cloned = handle.clone();

        assert_eq!(handle.id, cloned.id);
        assert_eq!(handle.task_id, cloned.task_id);
        assert_eq!(handle.tmux_session, cloned.tmux_session);
        assert_eq!(handle.worktree_path, cloned.worktree_path);
    }
}
