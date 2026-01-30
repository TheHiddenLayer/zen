//! Agent pool for multi-agent management.
//!
//! The `AgentPool` manages multiple concurrent agents, enforcing capacity
//! limits and providing lifecycle operations. It emits events for status
//! changes via a channel.

use crate::agent::{AgentId, AgentStatus};
use crate::error::Result;
use crate::workflow::TaskId;
use std::collections::HashMap;
use std::time::Duration;
use tokio::sync::mpsc;

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

/// A handle to an agent in the pool.
///
/// This is a placeholder struct that will be fully implemented in Task 4.3.
/// For now it provides the basic structure needed by AgentPool.
#[derive(Debug, Clone)]
pub struct AgentHandle {
    /// Unique identifier for this agent.
    pub id: AgentId,
    /// Current status of the agent.
    pub status: AgentStatus,
    /// The task this agent is assigned to (if any).
    pub task_id: Option<TaskId>,
}

impl AgentHandle {
    /// Create a new agent handle with the given ID.
    pub fn new(id: AgentId) -> Self {
        Self {
            id,
            status: AgentStatus::Idle,
            task_id: None,
        }
    }

    /// Create an agent handle that is running a task.
    pub fn with_task(id: AgentId, task_id: TaskId) -> Self {
        Self {
            id,
            status: AgentStatus::Running { task_id },
            task_id: Some(task_id),
        }
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
}
