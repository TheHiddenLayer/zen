//! Scheduler for parallel task execution.
//!
//! The Scheduler manages the execution of tasks from the DAG, dispatching
//! ready tasks to agents while respecting dependencies and capacity limits.
//! It is the execution engine that drives Phase 3 (Implementation).

use crate::agent::AgentId;
use crate::core::dag::TaskDAG;
use crate::core::task::TaskId;
use crate::error::Result;
use crate::orchestration::pool::{AgentEvent, AgentPool};
use crate::workflow::TaskId as WorkflowTaskId;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};

/// Convert from core::task::TaskId to workflow::TaskId.
///
/// Both types are UUID-based newtypes, so we can convert via the underlying UUID.
fn to_workflow_task_id(id: &TaskId) -> WorkflowTaskId {
    WorkflowTaskId(id.0)
}

/// Events emitted by the scheduler for task lifecycle changes.
///
/// These events allow external components (like the TUI) to react to
/// task state changes without polling.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SchedulerEvent {
    /// A task has been assigned to an agent and started.
    TaskStarted {
        /// The task that was started.
        task_id: TaskId,
        /// The agent assigned to the task.
        agent_id: AgentId,
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
    /// All tasks in the DAG have completed.
    AllTasksComplete,
}

/// Result of a task implementation.
///
/// Captures the outcome of a single task execution, including
/// the worktree location and final commit.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImplResult {
    /// The task that was implemented.
    pub task_id: TaskId,
    /// Path to the worktree where the implementation was done.
    pub worktree: PathBuf,
    /// The commit hash of the implementation.
    pub commit: String,
}

impl ImplResult {
    /// Create a new implementation result.
    pub fn new(task_id: TaskId, worktree: PathBuf, commit: String) -> Self {
        Self {
            task_id,
            worktree,
            commit,
        }
    }
}

/// Scheduler for parallel task execution.
///
/// The Scheduler monitors the DAG for ready tasks, spawns agents for them
/// (up to capacity), and handles completions/failures. It emits events
/// for each state change.
///
/// # Example
///
/// ```ignore
/// use tokio::sync::{mpsc, RwLock};
/// use std::sync::Arc;
/// use zen::core::dag::TaskDAG;
/// use zen::orchestration::{AgentPool, Scheduler, SchedulerEvent};
///
/// let dag = Arc::new(RwLock::new(TaskDAG::new()));
/// let (pool_tx, _) = mpsc::channel(100);
/// let pool = Arc::new(RwLock::new(AgentPool::new(4, pool_tx)));
/// let (event_tx, mut event_rx) = mpsc::channel(100);
///
/// let mut scheduler = Scheduler::new(dag, pool, event_tx);
/// let results = scheduler.run().await?;
/// ```
pub struct Scheduler {
    /// The task dependency graph.
    dag: Arc<RwLock<TaskDAG>>,
    /// Pool of agents for task execution.
    agent_pool: Arc<RwLock<AgentPool>>,
    /// Channel for emitting scheduler events.
    event_tx: mpsc::Sender<SchedulerEvent>,
    /// Set of completed task IDs.
    completed: HashSet<TaskId>,
    /// Mapping from agent ID to task ID for tracking active tasks.
    agent_tasks: HashMap<AgentId, TaskId>,
    /// Results collected from completed tasks.
    results: Vec<ImplResult>,
}

impl Scheduler {
    /// Create a new scheduler.
    ///
    /// # Arguments
    ///
    /// * `dag` - The task dependency graph to execute
    /// * `agent_pool` - Pool of agents for task execution
    /// * `event_tx` - Channel for emitting scheduler events
    pub fn new(
        dag: Arc<RwLock<TaskDAG>>,
        agent_pool: Arc<RwLock<AgentPool>>,
        event_tx: mpsc::Sender<SchedulerEvent>,
    ) -> Self {
        Self {
            dag,
            agent_pool,
            event_tx,
            completed: HashSet::new(),
            agent_tasks: HashMap::new(),
            results: Vec::new(),
        }
    }

    /// Get the set of completed task IDs.
    pub fn completed(&self) -> &HashSet<TaskId> {
        &self.completed
    }

    /// Check if all tasks are complete.
    pub async fn all_complete(&self) -> bool {
        let dag = self.dag.read().await;
        dag.all_complete(&self.completed)
    }

    /// Get the number of active (in-progress) tasks.
    pub fn active_count(&self) -> usize {
        self.agent_tasks.len()
    }

    /// Get ready tasks that can be scheduled.
    ///
    /// Returns tasks whose dependencies are satisfied and that
    /// are not already running.
    pub async fn get_ready_tasks(&self) -> Vec<TaskId> {
        let dag = self.dag.read().await;
        let ready = dag.ready_tasks(&self.completed);

        // Filter out tasks that are already running
        ready
            .into_iter()
            .filter(|task| !self.agent_tasks.values().any(|tid| tid == &task.id))
            .map(|task| task.id)
            .collect()
    }

    /// Dispatch ready tasks to available agents.
    ///
    /// Spawns agents for ready tasks up to the pool's capacity.
    /// Returns the number of tasks dispatched.
    pub async fn dispatch_ready_tasks(&mut self) -> Result<usize> {
        let ready_tasks = self.get_ready_tasks().await;
        let mut dispatched = 0;

        for task_id in ready_tasks {
            // Check if pool has capacity
            let has_capacity = {
                let pool = self.agent_pool.read().await;
                pool.has_capacity()
            };

            if !has_capacity {
                break;
            }

            // Spawn agent for task
            // Convert TaskId to WorkflowTaskId for AgentPool compatibility
            let workflow_task_id = to_workflow_task_id(&task_id);
            let agent_id = {
                let mut pool = self.agent_pool.write().await;
                pool.spawn(&workflow_task_id, "code-assist").await?
            };

            // Track the assignment
            self.agent_tasks.insert(agent_id, task_id);

            // Mark task as started in DAG
            {
                let mut dag = self.dag.write().await;
                if let Some(task) = dag.get_task_mut(&task_id) {
                    task.start();
                    task.assign_agent(agent_id);
                }
            }

            // Emit event
            let _ = self
                .event_tx
                .send(SchedulerEvent::TaskStarted { task_id, agent_id })
                .await;

            dispatched += 1;
        }

        Ok(dispatched)
    }

    /// Handle a task completion.
    ///
    /// Updates the completed set, records the result, and emits events.
    pub async fn handle_completion(
        &mut self,
        agent_id: AgentId,
        commit: String,
    ) -> Result<()> {
        // Get the task ID for this agent
        let task_id = match self.agent_tasks.remove(&agent_id) {
            Some(id) => id,
            None => return Ok(()), // Agent not tracked, ignore
        };

        // Mark task complete in DAG
        {
            let mut dag = self.dag.write().await;
            dag.complete_task(&task_id)?;
            if let Some(task) = dag.get_task_mut(&task_id) {
                task.set_commit(&commit);
            }
        }

        // Add to completed set
        self.completed.insert(task_id);

        // Get worktree path for result
        let worktree = {
            let dag = self.dag.read().await;
            dag.get_task(&task_id)
                .and_then(|t| t.worktree_path.clone())
                .unwrap_or_default()
        };

        // Record result
        self.results.push(ImplResult::new(task_id, worktree, commit.clone()));

        // Terminate the agent in the pool to free capacity
        {
            let mut pool = self.agent_pool.write().await;
            let _ = pool.terminate(&agent_id).await;
        }

        // Emit completion event
        let _ = self
            .event_tx
            .send(SchedulerEvent::TaskCompleted { task_id, commit })
            .await;

        // Check if all complete
        if self.all_complete().await {
            let _ = self.event_tx.send(SchedulerEvent::AllTasksComplete).await;
        }

        Ok(())
    }

    /// Handle a task failure.
    ///
    /// Updates the task status and emits a failure event.
    pub async fn handle_failure(&mut self, agent_id: AgentId, error: String) -> Result<()> {
        // Get the task ID for this agent
        let task_id = match self.agent_tasks.remove(&agent_id) {
            Some(id) => id,
            None => return Ok(()), // Agent not tracked, ignore
        };

        // Mark task failed in DAG
        {
            let mut dag = self.dag.write().await;
            if let Some(task) = dag.get_task_mut(&task_id) {
                task.fail(&error);
            }
        }

        // Terminate the agent in the pool to free capacity
        {
            let mut pool = self.agent_pool.write().await;
            let _ = pool.terminate(&agent_id).await;
        }

        // Emit failure event
        let _ = self
            .event_tx
            .send(SchedulerEvent::TaskFailed {
                task_id,
                error,
            })
            .await;

        Ok(())
    }

    /// Run the scheduling loop until all tasks complete.
    ///
    /// This is the main entry point for task execution. It:
    /// 1. Dispatches ready tasks to agents
    /// 2. Waits for agent events (via the agent pool's channel)
    /// 3. Processes completions and failures
    /// 4. Repeats until all tasks are done
    ///
    /// # Arguments
    ///
    /// * `agent_rx` - Receiver for agent events from the pool
    ///
    /// # Returns
    ///
    /// A vector of implementation results for all completed tasks.
    pub async fn run(
        &mut self,
        agent_rx: &mut mpsc::Receiver<AgentEvent>,
    ) -> Result<Vec<ImplResult>> {
        // Main scheduling loop
        loop {
            // Check if all tasks are complete
            if self.all_complete().await {
                break;
            }

            // Dispatch ready tasks
            self.dispatch_ready_tasks().await?;

            // If no tasks are active and no tasks dispatched, we might be stuck
            // (e.g., all remaining tasks have unmet dependencies that can't be satisfied)
            if self.active_count() == 0 {
                let ready = self.get_ready_tasks().await;
                if ready.is_empty() && !self.all_complete().await {
                    // No more tasks can be scheduled - exit
                    break;
                }
            }

            // Wait for an agent event
            if let Some(event) = agent_rx.recv().await {
                match event {
                    AgentEvent::Completed { agent_id, exit_code } => {
                        if exit_code == 0 {
                            // Get commit from agent handle if available
                            let commit = {
                                let pool = self.agent_pool.read().await;
                                pool.get(&agent_id)
                                    .and_then(|h| h.last_commit().ok().flatten())
                                    .unwrap_or_else(|| "unknown".to_string())
                            };
                            self.handle_completion(agent_id, commit).await?;
                        } else {
                            self.handle_failure(
                                agent_id,
                                format!("Agent exited with code {}", exit_code),
                            )
                            .await?;
                        }
                    }
                    AgentEvent::Failed { agent_id, error } => {
                        self.handle_failure(agent_id, error).await?;
                    }
                    AgentEvent::Terminated { agent_id } => {
                        // Treat termination as failure if task was in progress
                        if self.agent_tasks.contains_key(&agent_id) {
                            self.handle_failure(agent_id, "Agent terminated".to_string())
                                .await?;
                        }
                    }
                    _ => {
                        // Ignore other events (Started, StuckDetected handled elsewhere)
                    }
                }
            }
        }

        Ok(std::mem::take(&mut self.results))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::task::Task;

    // Helper to create test components
    fn create_test_scheduler(
        max_agents: usize,
    ) -> (
        Scheduler,
        Arc<RwLock<TaskDAG>>,
        Arc<RwLock<AgentPool>>,
        mpsc::Receiver<SchedulerEvent>,
        mpsc::Receiver<AgentEvent>,
    ) {
        let dag = Arc::new(RwLock::new(TaskDAG::new()));
        let (pool_tx, pool_rx) = mpsc::channel(100);
        let pool = Arc::new(RwLock::new(AgentPool::new(max_agents, pool_tx)));
        let (event_tx, event_rx) = mpsc::channel(100);

        let scheduler = Scheduler::new(Arc::clone(&dag), Arc::clone(&pool), event_tx);
        (scheduler, dag, pool, event_rx, pool_rx)
    }

    // Helper to create a test task
    fn test_task(name: &str) -> Task {
        Task::new(name, &format!("{} description", name))
    }

    // ========== SchedulerEvent Tests ==========

    #[test]
    fn test_scheduler_event_task_started() {
        let task_id = TaskId::new();
        let agent_id = AgentId::new();
        let event = SchedulerEvent::TaskStarted { task_id, agent_id };

        if let SchedulerEvent::TaskStarted {
            task_id: tid,
            agent_id: aid,
        } = event
        {
            assert_eq!(tid, task_id);
            assert_eq!(aid, agent_id);
        } else {
            panic!("Expected TaskStarted variant");
        }
    }

    #[test]
    fn test_scheduler_event_task_completed() {
        let task_id = TaskId::new();
        let commit = "abc123".to_string();
        let event = SchedulerEvent::TaskCompleted {
            task_id,
            commit: commit.clone(),
        };

        if let SchedulerEvent::TaskCompleted {
            task_id: tid,
            commit: c,
        } = event
        {
            assert_eq!(tid, task_id);
            assert_eq!(c, commit);
        } else {
            panic!("Expected TaskCompleted variant");
        }
    }

    #[test]
    fn test_scheduler_event_task_failed() {
        let task_id = TaskId::new();
        let error = "compilation error".to_string();
        let event = SchedulerEvent::TaskFailed {
            task_id,
            error: error.clone(),
        };

        if let SchedulerEvent::TaskFailed {
            task_id: tid,
            error: e,
        } = event
        {
            assert_eq!(tid, task_id);
            assert_eq!(e, error);
        } else {
            panic!("Expected TaskFailed variant");
        }
    }

    #[test]
    fn test_scheduler_event_all_tasks_complete() {
        let event = SchedulerEvent::AllTasksComplete;
        assert!(matches!(event, SchedulerEvent::AllTasksComplete));
    }

    #[test]
    fn test_scheduler_event_debug() {
        let event = SchedulerEvent::AllTasksComplete;
        let debug = format!("{:?}", event);
        assert!(debug.contains("AllTasksComplete"));
    }

    #[test]
    fn test_scheduler_event_clone() {
        let task_id = TaskId::new();
        let event = SchedulerEvent::TaskCompleted {
            task_id,
            commit: "abc".to_string(),
        };
        let cloned = event.clone();
        assert_eq!(event, cloned);
    }

    #[test]
    fn test_scheduler_event_equality() {
        let task_id = TaskId::new();
        let agent_id = AgentId::new();

        let e1 = SchedulerEvent::TaskStarted { task_id, agent_id };
        let e2 = SchedulerEvent::TaskStarted { task_id, agent_id };
        assert_eq!(e1, e2);

        let e3 = SchedulerEvent::AllTasksComplete;
        let e4 = SchedulerEvent::AllTasksComplete;
        assert_eq!(e3, e4);
    }

    // ========== ImplResult Tests ==========

    #[test]
    fn test_impl_result_new() {
        let task_id = TaskId::new();
        let worktree = PathBuf::from("/tmp/worktree");
        let commit = "abc123".to_string();

        let result = ImplResult::new(task_id, worktree.clone(), commit.clone());

        assert_eq!(result.task_id, task_id);
        assert_eq!(result.worktree, worktree);
        assert_eq!(result.commit, commit);
    }

    #[test]
    fn test_impl_result_debug() {
        let result = ImplResult::new(TaskId::new(), PathBuf::new(), "abc".to_string());
        let debug = format!("{:?}", result);
        assert!(debug.contains("ImplResult"));
    }

    #[test]
    fn test_impl_result_clone() {
        let result = ImplResult::new(TaskId::new(), PathBuf::from("/tmp"), "abc".to_string());
        let cloned = result.clone();
        assert_eq!(result.task_id, cloned.task_id);
        assert_eq!(result.worktree, cloned.worktree);
        assert_eq!(result.commit, cloned.commit);
    }

    #[test]
    fn test_impl_result_serialization() {
        let result = ImplResult::new(
            TaskId::new(),
            PathBuf::from("/tmp/worktree"),
            "abc123def456".to_string(),
        );

        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("task_id"));
        assert!(json.contains("worktree"));
        assert!(json.contains("commit"));

        let parsed: ImplResult = serde_json::from_str(&json).unwrap();
        assert_eq!(result.task_id, parsed.task_id);
        assert_eq!(result.commit, parsed.commit);
    }

    // ========== Scheduler Creation Tests ==========

    #[tokio::test]
    async fn test_scheduler_new() {
        let (scheduler, _, _, _, _) = create_test_scheduler(4);
        assert!(scheduler.completed().is_empty());
        assert_eq!(scheduler.active_count(), 0);
    }

    #[tokio::test]
    async fn test_scheduler_all_complete_empty_dag() {
        let (scheduler, _, _, _, _) = create_test_scheduler(4);
        assert!(scheduler.all_complete().await);
    }

    #[tokio::test]
    async fn test_scheduler_all_complete_with_tasks() {
        let (scheduler, dag, _, _, _) = create_test_scheduler(4);

        // Add tasks to DAG
        {
            let mut dag = dag.write().await;
            dag.add_task(test_task("task-a"));
            dag.add_task(test_task("task-b"));
        }

        assert!(!scheduler.all_complete().await);
    }

    // ========== Ready Task Tests ==========

    #[tokio::test]
    async fn test_get_ready_tasks_independent() {
        let (scheduler, dag, _, _, _) = create_test_scheduler(4);

        // Add independent tasks
        let task_a = test_task("task-a");
        let task_b = test_task("task-b");
        let task_c = test_task("task-c");
        let id_a = task_a.id;
        let id_b = task_b.id;
        let id_c = task_c.id;

        {
            let mut dag = dag.write().await;
            dag.add_task(task_a);
            dag.add_task(task_b);
            dag.add_task(task_c);
        }

        let ready = scheduler.get_ready_tasks().await;

        // All 3 tasks should be ready
        assert_eq!(ready.len(), 3);
        assert!(ready.contains(&id_a));
        assert!(ready.contains(&id_b));
        assert!(ready.contains(&id_c));
    }

    #[tokio::test]
    async fn test_get_ready_tasks_with_dependencies() {
        let (scheduler, dag, _, _, _) = create_test_scheduler(4);

        // Add tasks with dependencies: A -> C, B -> C
        let task_a = test_task("task-a");
        let task_b = test_task("task-b");
        let task_c = test_task("task-c");
        let id_a = task_a.id;
        let id_b = task_b.id;
        let id_c = task_c.id;

        {
            let mut dag = dag.write().await;
            dag.add_task(task_a);
            dag.add_task(task_b);
            dag.add_task(task_c);
            dag.add_dependency(&id_a, &id_c, crate::core::dag::DependencyType::DataDependency)
                .unwrap();
            dag.add_dependency(&id_b, &id_c, crate::core::dag::DependencyType::DataDependency)
                .unwrap();
        }

        let ready = scheduler.get_ready_tasks().await;

        // Only A and B should be ready (C depends on both)
        assert_eq!(ready.len(), 2);
        assert!(ready.contains(&id_a));
        assert!(ready.contains(&id_b));
        assert!(!ready.contains(&id_c));
    }

    // ========== Dispatch Tests ==========

    #[tokio::test]
    async fn test_dispatch_ready_tasks() {
        let (mut scheduler, dag, _, mut event_rx, _) = create_test_scheduler(4);

        // Add 3 independent tasks
        let task_a = test_task("task-a");
        let task_b = test_task("task-b");
        let task_c = test_task("task-c");

        {
            let mut dag = dag.write().await;
            dag.add_task(task_a);
            dag.add_task(task_b);
            dag.add_task(task_c);
        }

        // Dispatch should spawn all 3 (capacity is 4)
        let dispatched = scheduler.dispatch_ready_tasks().await.unwrap();

        assert_eq!(dispatched, 3);
        assert_eq!(scheduler.active_count(), 3);

        // Should have received 3 TaskStarted events
        for _ in 0..3 {
            let event = event_rx.recv().await.unwrap();
            assert!(matches!(event, SchedulerEvent::TaskStarted { .. }));
        }
    }

    #[tokio::test]
    async fn test_dispatch_respects_capacity() {
        let (mut scheduler, dag, _, _, _) = create_test_scheduler(2);

        // Add 3 independent tasks but capacity is only 2
        let task_a = test_task("task-a");
        let task_b = test_task("task-b");
        let task_c = test_task("task-c");

        {
            let mut dag = dag.write().await;
            dag.add_task(task_a);
            dag.add_task(task_b);
            dag.add_task(task_c);
        }

        let dispatched = scheduler.dispatch_ready_tasks().await.unwrap();

        // Only 2 should be dispatched (capacity limit)
        assert_eq!(dispatched, 2);
        assert_eq!(scheduler.active_count(), 2);
    }

    #[tokio::test]
    async fn test_dispatch_respects_dependencies() {
        let (mut scheduler, dag, _, _, _) = create_test_scheduler(4);

        // Chain: A -> B -> C
        let task_a = test_task("task-a");
        let task_b = test_task("task-b");
        let task_c = test_task("task-c");
        let id_a = task_a.id;
        let id_b = task_b.id;
        let id_c = task_c.id;

        {
            let mut dag = dag.write().await;
            dag.add_task(task_a);
            dag.add_task(task_b);
            dag.add_task(task_c);
            dag.add_dependency(&id_a, &id_b, crate::core::dag::DependencyType::DataDependency)
                .unwrap();
            dag.add_dependency(&id_b, &id_c, crate::core::dag::DependencyType::DataDependency)
                .unwrap();
        }

        let dispatched = scheduler.dispatch_ready_tasks().await.unwrap();

        // Only A should be dispatched (B and C have unmet deps)
        assert_eq!(dispatched, 1);
    }

    // ========== Completion Handling Tests ==========

    #[tokio::test]
    async fn test_handle_completion() {
        let (mut scheduler, dag, _, mut event_rx, _) = create_test_scheduler(4);

        // Add and dispatch a task
        let task = test_task("task-a");
        let task_id = task.id;

        {
            let mut dag = dag.write().await;
            dag.add_task(task);
        }

        scheduler.dispatch_ready_tasks().await.unwrap();

        // Get the agent ID from the event
        let agent_id = match event_rx.recv().await.unwrap() {
            SchedulerEvent::TaskStarted { agent_id, .. } => agent_id,
            _ => panic!("Expected TaskStarted"),
        };

        // Handle completion
        scheduler
            .handle_completion(agent_id, "abc123".to_string())
            .await
            .unwrap();

        // Verify completion
        assert!(scheduler.completed().contains(&task_id));
        assert_eq!(scheduler.active_count(), 0);

        // Should receive TaskCompleted event
        let event = event_rx.recv().await.unwrap();
        assert!(matches!(
            event,
            SchedulerEvent::TaskCompleted { commit, .. } if commit == "abc123"
        ));

        // And AllTasksComplete since it's the only task
        let event = event_rx.recv().await.unwrap();
        assert!(matches!(event, SchedulerEvent::AllTasksComplete));
    }

    #[tokio::test]
    async fn test_completion_unlocks_dependents() {
        let (mut scheduler, dag, _, mut event_rx, _) = create_test_scheduler(4);

        // A -> B dependency
        let task_a = test_task("task-a");
        let task_b = test_task("task-b");
        let id_a = task_a.id;
        let id_b = task_b.id;

        {
            let mut dag = dag.write().await;
            dag.add_task(task_a);
            dag.add_task(task_b);
            dag.add_dependency(&id_a, &id_b, crate::core::dag::DependencyType::DataDependency)
                .unwrap();
        }

        // Initially only A is ready
        let ready = scheduler.get_ready_tasks().await;
        assert_eq!(ready.len(), 1);
        assert!(ready.contains(&id_a));

        // Dispatch A
        scheduler.dispatch_ready_tasks().await.unwrap();

        // Get agent ID
        let agent_id = match event_rx.recv().await.unwrap() {
            SchedulerEvent::TaskStarted { agent_id, .. } => agent_id,
            _ => panic!("Expected TaskStarted"),
        };

        // Complete A
        scheduler
            .handle_completion(agent_id, "commit-a".to_string())
            .await
            .unwrap();

        // Now B should be ready
        let ready = scheduler.get_ready_tasks().await;
        assert_eq!(ready.len(), 1);
        assert!(ready.contains(&id_b));
    }

    // ========== Failure Handling Tests ==========

    #[tokio::test]
    async fn test_handle_failure() {
        let (mut scheduler, dag, _, mut event_rx, _) = create_test_scheduler(4);

        // Add and dispatch a task
        let task = test_task("task-a");
        let task_id = task.id;

        {
            let mut dag = dag.write().await;
            dag.add_task(task);
        }

        scheduler.dispatch_ready_tasks().await.unwrap();

        // Get the agent ID
        let agent_id = match event_rx.recv().await.unwrap() {
            SchedulerEvent::TaskStarted { agent_id, .. } => agent_id,
            _ => panic!("Expected TaskStarted"),
        };

        // Handle failure
        scheduler
            .handle_failure(agent_id, "test error".to_string())
            .await
            .unwrap();

        // Task should not be in completed set
        assert!(!scheduler.completed().contains(&task_id));
        assert_eq!(scheduler.active_count(), 0);

        // Should receive TaskFailed event
        let event = event_rx.recv().await.unwrap();
        assert!(matches!(
            event,
            SchedulerEvent::TaskFailed { error, .. } if error == "test error"
        ));

        // Verify task status in DAG
        let dag = dag.read().await;
        let task = dag.get_task(&task_id).unwrap();
        assert!(matches!(
            task.status,
            crate::core::task::TaskStatus::Failed { .. }
        ));
    }

    // ========== Integration Tests ==========

    #[tokio::test]
    async fn test_full_dag_execution() {
        let (mut scheduler, dag, _, mut event_rx, mut agent_rx) = create_test_scheduler(2);

        // Create a diamond DAG: A, B -> C
        let task_a = test_task("task-a");
        let task_b = test_task("task-b");
        let task_c = test_task("task-c");
        let id_a = task_a.id;
        let id_b = task_b.id;
        let id_c = task_c.id;

        {
            let mut dag = dag.write().await;
            dag.add_task(task_a);
            dag.add_task(task_b);
            dag.add_task(task_c);
            dag.add_dependency(&id_a, &id_c, crate::core::dag::DependencyType::DataDependency)
                .unwrap();
            dag.add_dependency(&id_b, &id_c, crate::core::dag::DependencyType::DataDependency)
                .unwrap();
        }

        // Dispatch first batch (A and B)
        let dispatched = scheduler.dispatch_ready_tasks().await.unwrap();
        assert_eq!(dispatched, 2);

        // Collect agent IDs from events
        let mut agent_ids = Vec::new();
        for _ in 0..2 {
            if let SchedulerEvent::TaskStarted { agent_id, .. } = event_rx.recv().await.unwrap() {
                agent_ids.push(agent_id);
            }
        }

        // Complete A
        scheduler
            .handle_completion(agent_ids[0], "commit-a".to_string())
            .await
            .unwrap();

        // Drain TaskCompleted event for A
        let _event_a = event_rx.recv().await.unwrap();

        // C still not ready (needs B)
        let ready = scheduler.get_ready_tasks().await;
        assert!(ready.is_empty());

        // Complete B
        scheduler
            .handle_completion(agent_ids[1], "commit-b".to_string())
            .await
            .unwrap();

        // Drain TaskCompleted event for B
        let _event_b = event_rx.recv().await.unwrap();

        // Now C is ready
        let ready = scheduler.get_ready_tasks().await;
        assert_eq!(ready.len(), 1);
        assert!(ready.contains(&id_c));

        // Dispatch C
        let dispatched = scheduler.dispatch_ready_tasks().await.unwrap();
        assert_eq!(dispatched, 1);

        // Get C's agent ID
        let agent_c = match event_rx.recv().await.unwrap() {
            SchedulerEvent::TaskStarted { agent_id, .. } => agent_id,
            other => panic!("Expected TaskStarted, got {:?}", other),
        };

        // Complete C
        scheduler
            .handle_completion(agent_c, "commit-c".to_string())
            .await
            .unwrap();

        // All should be complete
        assert!(scheduler.all_complete().await);
        assert_eq!(scheduler.completed().len(), 3);
    }

    #[tokio::test]
    async fn test_acceptance_ready_task_dispatch() {
        // Given 3 ready tasks and capacity for 2
        let (mut scheduler, dag, _, _, _) = create_test_scheduler(2);

        {
            let mut dag = dag.write().await;
            dag.add_task(test_task("task-a"));
            dag.add_task(test_task("task-b"));
            dag.add_task(test_task("task-c"));
        }

        // When scheduler runs dispatch
        let dispatched = scheduler.dispatch_ready_tasks().await.unwrap();

        // Then 2 tasks are started (up to capacity)
        assert_eq!(dispatched, 2);
        assert_eq!(scheduler.active_count(), 2);
    }

    #[tokio::test]
    async fn test_acceptance_dependency_respect() {
        // Given A->B dependency
        let (scheduler, dag, _, _, _) = create_test_scheduler(4);

        let task_a = test_task("task-a");
        let task_b = test_task("task-b");
        let id_a = task_a.id;
        let id_b = task_b.id;

        {
            let mut dag = dag.write().await;
            dag.add_task(task_a);
            dag.add_task(task_b);
            dag.add_dependency(&id_a, &id_b, crate::core::dag::DependencyType::DataDependency)
                .unwrap();
        }

        // When A is not complete
        let ready = scheduler.get_ready_tasks().await;

        // Then B is not started (not in ready list)
        assert!(!ready.contains(&id_b));
    }

    #[tokio::test]
    async fn test_acceptance_completion_handling() {
        // Given a task completes
        let (mut scheduler, dag, _, mut event_rx, _) = create_test_scheduler(4);

        let task_a = test_task("task-a");
        let task_b = test_task("task-b");
        let id_a = task_a.id;
        let id_b = task_b.id;

        {
            let mut dag = dag.write().await;
            dag.add_task(task_a);
            dag.add_task(task_b);
            dag.add_dependency(&id_a, &id_b, crate::core::dag::DependencyType::DataDependency)
                .unwrap();
        }

        scheduler.dispatch_ready_tasks().await.unwrap();

        let agent_id = match event_rx.recv().await.unwrap() {
            SchedulerEvent::TaskStarted { agent_id, .. } => agent_id,
            _ => panic!("Expected TaskStarted"),
        };

        // When scheduler processes completion
        scheduler
            .handle_completion(agent_id, "commit".to_string())
            .await
            .unwrap();

        // Then completed set is updated and new tasks may become ready
        assert!(scheduler.completed().contains(&id_a));
        let ready = scheduler.get_ready_tasks().await;
        assert!(ready.contains(&id_b));
    }

    #[tokio::test]
    async fn test_acceptance_all_complete_detection() {
        // Given all tasks complete
        let (mut scheduler, dag, _, mut event_rx, _) = create_test_scheduler(4);

        let task = test_task("task-a");
        let task_id = task.id;

        {
            let mut dag = dag.write().await;
            dag.add_task(task);
        }

        scheduler.dispatch_ready_tasks().await.unwrap();

        let agent_id = match event_rx.recv().await.unwrap() {
            SchedulerEvent::TaskStarted { agent_id, .. } => agent_id,
            _ => panic!("Expected TaskStarted"),
        };

        scheduler
            .handle_completion(agent_id, "commit".to_string())
            .await
            .unwrap();

        // When scheduler checks
        // Then AllTasksComplete event is emitted
        // Skip TaskCompleted event
        event_rx.recv().await.unwrap();

        let event = event_rx.recv().await.unwrap();
        assert!(matches!(event, SchedulerEvent::AllTasksComplete));
    }

    #[tokio::test]
    async fn test_acceptance_event_emission() {
        // Given task state changes
        let (mut scheduler, dag, _, mut event_rx, _) = create_test_scheduler(4);

        {
            let mut dag = dag.write().await;
            dag.add_task(test_task("task-a"));
        }

        // When changes occur (dispatch)
        scheduler.dispatch_ready_tasks().await.unwrap();

        // Then appropriate SchedulerEvents are sent
        let event = event_rx.recv().await.unwrap();
        assert!(matches!(event, SchedulerEvent::TaskStarted { .. }));
    }
}
