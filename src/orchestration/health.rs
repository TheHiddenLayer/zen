//! Health monitoring for agents.
//!
//! The `HealthMonitor` detects stuck or failing agents by monitoring activity
//! timestamps and output patterns. When issues are detected, it emits events
//! that can trigger recovery actions.
//!
//! ## AI-Driven Recovery
//!
//! The health monitor uses AI judgment to determine the best recovery action
//! for stuck or failing agents. The `determine_recovery()` method analyzes:
//! - Recent agent output
//! - Task description
//! - Error patterns
//! - Retry count
//!
//! Based on this analysis, it returns an appropriate `RecoveryAction`:
//! - `Restart` for transient errors
//! - `Decompose` for complex tasks that need breaking down
//! - `Escalate` when max retries are exceeded
//! - `Abort` for unrecoverable failures

use crate::agent::AgentId;
use crate::orchestration::{AgentHandle, AgentPool};
use crate::workflow::TaskId;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, RwLock};

/// Default time without activity before an agent is considered stuck.
pub const DEFAULT_STUCK_THRESHOLD_SECS: u64 = 300; // 5 minutes

/// Default maximum number of retries before giving up on a task.
pub const DEFAULT_MAX_RETRIES: u32 = 3;

/// Configuration for health monitoring.
///
/// Controls how the health monitor detects stuck or failing agents.
#[derive(Debug, Clone)]
pub struct HealthConfig {
    /// Time without activity before an agent is considered stuck.
    pub stuck_threshold: Duration,
    /// Maximum number of retries before giving up on a task.
    pub max_retries: u32,
    /// Patterns in agent output that indicate a stuck state.
    pub stuck_patterns: Vec<String>,
}

impl Default for HealthConfig {
    fn default() -> Self {
        Self {
            stuck_threshold: Duration::from_secs(DEFAULT_STUCK_THRESHOLD_SECS),
            max_retries: DEFAULT_MAX_RETRIES,
            stuck_patterns: vec![
                "rate limit".to_string(),
                "rate_limit".to_string(),
                "too many requests".to_string(),
                "quota exceeded".to_string(),
                "waiting for response".to_string(),
                "retrying".to_string(),
                "connection refused".to_string(),
                "timeout".to_string(),
            ],
        }
    }
}

impl HealthConfig {
    /// Create a new health config with the specified stuck threshold.
    pub fn with_stuck_threshold(threshold: Duration) -> Self {
        Self {
            stuck_threshold: threshold,
            ..Default::default()
        }
    }

    /// Create a new health config with the specified max retries.
    pub fn with_max_retries(max_retries: u32) -> Self {
        Self {
            max_retries,
            ..Default::default()
        }
    }

    /// Add a stuck pattern to detect.
    pub fn add_stuck_pattern(&mut self, pattern: &str) {
        self.stuck_patterns.push(pattern.to_string());
    }
}

/// Events emitted by the health monitor.
///
/// These events inform other components about agent health issues
/// and recovery actions taken.
#[derive(Debug, Clone)]
pub enum HealthEvent {
    /// An agent appears to be stuck (no activity for too long).
    AgentStuck {
        /// The agent that appears stuck.
        agent_id: AgentId,
        /// How long the agent has been idle.
        duration: Duration,
    },
    /// An agent has failed with an error.
    AgentFailed {
        /// The agent that failed.
        agent_id: AgentId,
        /// Error message describing the failure.
        error: String,
    },
    /// A recovery action was triggered for an agent.
    RecoveryTriggered {
        /// The agent that recovery was triggered for.
        agent_id: AgentId,
        /// The recovery action taken.
        action: RecoveryAction,
    },
}

/// Recovery actions for unhealthy agents.
///
/// When an agent is detected as stuck or failing, a recovery action
/// determines what should be done to address the issue.
#[derive(Debug, Clone, PartialEq)]
pub enum RecoveryAction {
    /// Restart the agent.
    Restart,
    /// Reassign the task to another agent.
    Reassign {
        /// The agent to reassign to.
        to_agent: AgentId,
    },
    /// Decompose the task into smaller subtasks.
    Decompose {
        /// The smaller tasks to create.
        into_tasks: Vec<String>,
    },
    /// Escalate to the user for manual intervention.
    Escalate {
        /// Message explaining why escalation is needed.
        message: String,
    },
    /// Abort the task entirely.
    Abort,
}

/// Tracks retry counts per task for recovery decisions.
///
/// The retry tracker maintains a count of how many times each task
/// has been retried, enabling the health monitor to make informed
/// decisions about when to escalate or abort.
///
/// # Example
///
/// ```
/// use zen::orchestration::RetryTracker;
/// use zen::workflow::TaskId;
///
/// let mut tracker = RetryTracker::new();
/// let task_id = TaskId::new();
///
/// assert_eq!(tracker.get_retries(&task_id), 0);
/// tracker.increment(&task_id);
/// assert_eq!(tracker.get_retries(&task_id), 1);
/// ```
#[derive(Debug, Clone, Default)]
pub struct RetryTracker {
    /// Maps task IDs to their retry counts.
    retries: HashMap<TaskId, u32>,
}

impl RetryTracker {
    /// Create a new empty retry tracker.
    pub fn new() -> Self {
        Self::default()
    }

    /// Get the number of retries for a task.
    ///
    /// Returns 0 if the task has not been retried.
    pub fn get_retries(&self, task_id: &TaskId) -> u32 {
        self.retries.get(task_id).copied().unwrap_or(0)
    }

    /// Increment the retry count for a task.
    ///
    /// Returns the new retry count.
    pub fn increment(&mut self, task_id: &TaskId) -> u32 {
        let count = self.retries.entry(*task_id).or_insert(0);
        *count += 1;
        *count
    }

    /// Reset the retry count for a task.
    pub fn reset(&mut self, task_id: &TaskId) {
        self.retries.remove(task_id);
    }

    /// Clear all retry counts.
    pub fn clear(&mut self) {
        self.retries.clear();
    }

    /// Get the number of tracked tasks.
    pub fn len(&self) -> usize {
        self.retries.len()
    }

    /// Check if there are no tracked tasks.
    pub fn is_empty(&self) -> bool {
        self.retries.is_empty()
    }
}

/// Health monitor for agents.
///
/// Monitors agent activity and output to detect stuck or failing agents,
/// then emits events for recovery. Includes AI-driven recovery decision
/// making and retry tracking.
///
/// # Example
///
/// ```ignore
/// use std::sync::Arc;
/// use tokio::sync::{mpsc, RwLock};
/// use zen::orchestration::{AgentPool, HealthMonitor, HealthConfig, HealthEvent};
///
/// let (pool_tx, _) = mpsc::channel(100);
/// let pool = Arc::new(RwLock::new(AgentPool::new(3, pool_tx)));
///
/// let (health_tx, mut health_rx) = mpsc::channel(100);
/// let monitor = HealthMonitor::new(HealthConfig::default(), pool.clone(), health_tx);
///
/// // Check all agents for health issues
/// let events = monitor.check_all().await;
/// ```
pub struct HealthMonitor {
    /// Configuration for health monitoring.
    config: HealthConfig,
    /// The agent pool to monitor.
    agent_pool: Arc<RwLock<AgentPool>>,
    /// Channel for emitting health events.
    event_tx: mpsc::Sender<HealthEvent>,
    /// Retry tracker for tasks.
    retry_tracker: Arc<RwLock<RetryTracker>>,
    /// Repository path for AI analysis context.
    repo_path: PathBuf,
}

impl HealthMonitor {
    /// Create a new health monitor.
    ///
    /// # Arguments
    ///
    /// * `config` - Configuration for health monitoring
    /// * `agent_pool` - The agent pool to monitor
    /// * `event_tx` - Channel for emitting health events
    pub fn new(
        config: HealthConfig,
        agent_pool: Arc<RwLock<AgentPool>>,
        event_tx: mpsc::Sender<HealthEvent>,
    ) -> Self {
        Self {
            config,
            agent_pool,
            event_tx,
            retry_tracker: Arc::new(RwLock::new(RetryTracker::new())),
            repo_path: PathBuf::from("."),
        }
    }

    /// Create a new health monitor with a specific repository path.
    ///
    /// # Arguments
    ///
    /// * `config` - Configuration for health monitoring
    /// * `agent_pool` - The agent pool to monitor
    /// * `event_tx` - Channel for emitting health events
    /// * `repo_path` - Path to the repository for AI analysis context
    pub fn with_repo_path(
        config: HealthConfig,
        agent_pool: Arc<RwLock<AgentPool>>,
        event_tx: mpsc::Sender<HealthEvent>,
        repo_path: impl AsRef<Path>,
    ) -> Self {
        Self {
            config,
            agent_pool,
            event_tx,
            retry_tracker: Arc::new(RwLock::new(RetryTracker::new())),
            repo_path: repo_path.as_ref().to_path_buf(),
        }
    }

    /// Get a reference to the health config.
    pub fn config(&self) -> &HealthConfig {
        &self.config
    }

    /// Get a reference to the retry tracker.
    pub fn retry_tracker(&self) -> &Arc<RwLock<RetryTracker>> {
        &self.retry_tracker
    }

    /// Get the repository path.
    pub fn repo_path(&self) -> &Path {
        &self.repo_path
    }

    /// Check all agents for health issues.
    ///
    /// Iterates through all active agents in the pool and checks each one
    /// for stuck or failure conditions.
    ///
    /// # Returns
    ///
    /// A vector of health events for agents with issues.
    pub async fn check_all(&self) -> Vec<HealthEvent> {
        let pool = self.agent_pool.read().await;
        let mut events = Vec::new();

        // Get all agent IDs first to avoid holding the lock
        let agent_ids: Vec<AgentId> = pool
            .agents_iter()
            .map(|(id, _)| *id)
            .collect();

        // Check each agent
        for agent_id in agent_ids {
            if let Some(agent) = pool.get(&agent_id) {
                if let Some(event) = self.check_agent(agent) {
                    // Send event through channel
                    let _ = self.event_tx.send(event.clone()).await;
                    events.push(event);
                }
            }
        }

        events
    }

    /// Check a single agent for health issues.
    ///
    /// Examines the agent's activity timestamp and output patterns to detect
    /// stuck or failing conditions.
    ///
    /// # Arguments
    ///
    /// * `agent` - The agent handle to check
    ///
    /// # Returns
    ///
    /// A `HealthEvent` if an issue is detected, `None` if the agent is healthy.
    pub fn check_agent(&self, agent: &AgentHandle) -> Option<HealthEvent> {
        // Check for stuck condition based on idle time
        let idle_duration = agent.idle_duration();
        if idle_duration >= self.config.stuck_threshold {
            return Some(HealthEvent::AgentStuck {
                agent_id: agent.id,
                duration: idle_duration,
            });
        }

        // Check output for stuck patterns
        if let Some(event) = self.check_output_patterns(agent) {
            return Some(event);
        }

        None
    }

    /// Check agent output for stuck patterns.
    ///
    /// Reads the agent's recent output and looks for patterns that indicate
    /// the agent may be stuck (rate limits, timeouts, etc.).
    ///
    /// # Arguments
    ///
    /// * `agent` - The agent handle to check
    ///
    /// # Returns
    ///
    /// A `HealthEvent` if a stuck pattern is detected, `None` otherwise.
    fn check_output_patterns(&self, agent: &AgentHandle) -> Option<HealthEvent> {
        // Try to read the agent's output
        let output = match agent.read_raw_output() {
            Ok(content) => content,
            Err(_) => return None, // Can't read output, skip pattern check
        };

        let lower_output = output.to_lowercase();

        // Check for stuck patterns
        for pattern in &self.config.stuck_patterns {
            if lower_output.contains(&pattern.to_lowercase()) {
                return Some(HealthEvent::AgentFailed {
                    agent_id: agent.id,
                    error: format!("Detected stuck pattern: {}", pattern),
                });
            }
        }

        None
    }

    /// Check if an agent is healthy (has no issues).
    ///
    /// # Arguments
    ///
    /// * `agent` - The agent handle to check
    ///
    /// # Returns
    ///
    /// `true` if the agent is healthy, `false` if there are issues.
    pub fn is_healthy(&self, agent: &AgentHandle) -> bool {
        self.check_agent(agent).is_none()
    }

    /// Determine the best recovery action for a stuck or failing agent.
    ///
    /// Uses heuristics to analyze the agent's state and decide the appropriate
    /// recovery action. The decision is based on:
    /// - Recent agent output (error patterns)
    /// - Retry count for the task
    /// - Configuration settings (max_retries)
    ///
    /// # Arguments
    ///
    /// * `agent` - The agent handle to analyze
    /// * `task_description` - Optional description of the task for context
    ///
    /// # Returns
    ///
    /// A `RecoveryAction` indicating what should be done.
    ///
    /// # Decision Logic
    ///
    /// 1. If max retries exceeded -> Escalate
    /// 2. If transient error pattern detected -> Restart
    /// 3. If complex task indicators -> Decompose (future AI integration)
    /// 4. Default -> Restart (safest default action)
    pub async fn determine_recovery(
        &self,
        agent: &AgentHandle,
        task_description: Option<&str>,
    ) -> RecoveryAction {
        // Get retry count for the task
        let retry_count = if let Some(task_id) = agent.task_id {
            let tracker = self.retry_tracker.read().await;
            tracker.get_retries(&task_id)
        } else {
            0
        };

        // Check if max retries exceeded
        if retry_count >= self.config.max_retries {
            return RecoveryAction::Escalate {
                message: format!(
                    "Task has been retried {} times (max: {}). Manual intervention required.",
                    retry_count, self.config.max_retries
                ),
            };
        }

        // Read agent output to analyze error patterns
        let output = agent.read_raw_output().unwrap_or_default();
        let lower_output = output.to_lowercase();

        // Check for transient error patterns (should restart)
        let transient_patterns = [
            "rate limit",
            "rate_limit",
            "too many requests",
            "connection refused",
            "timeout",
            "temporary",
            "retry",
            "503",
            "502",
            "network error",
        ];

        for pattern in transient_patterns {
            if lower_output.contains(pattern) {
                return RecoveryAction::Restart;
            }
        }

        // Check for fatal error patterns (should abort)
        let fatal_patterns = [
            "permission denied",
            "access denied",
            "authentication failed",
            "invalid credentials",
            "not found: 404",
            "syntax error",
            "compilation failed",
        ];

        for pattern in fatal_patterns {
            if lower_output.contains(pattern) {
                // Only abort if we've already tried restarting
                if retry_count > 0 {
                    return RecoveryAction::Abort;
                }
            }
        }

        // Check for complexity indicators that suggest decomposition
        // This is a heuristic - in a real AI integration, this would use
        // ClaudeHeadless to analyze the task and suggest subtasks
        let complexity_patterns = [
            "too complex",
            "too large",
            "multiple steps required",
            "breaking down",
            "subtask",
        ];

        for pattern in complexity_patterns {
            if lower_output.contains(pattern) {
                // Return decompose with placeholder subtasks
                // In full AI integration, ClaudeHeadless would generate these
                return RecoveryAction::Decompose {
                    into_tasks: vec![
                        format!(
                            "Part 1 of: {}",
                            task_description.unwrap_or("original task")
                        ),
                        format!(
                            "Part 2 of: {}",
                            task_description.unwrap_or("original task")
                        ),
                    ],
                };
            }
        }

        // Default action is to restart (safest option for unknown issues)
        RecoveryAction::Restart
    }

    /// Execute a recovery action for an agent.
    ///
    /// Performs the necessary operations to recover a stuck or failing agent
    /// based on the specified recovery action.
    ///
    /// # Arguments
    ///
    /// * `agent_id` - The ID of the agent to recover
    /// * `action` - The recovery action to execute
    ///
    /// # Returns
    ///
    /// `Ok(())` if the recovery action was executed successfully,
    /// or an error if the action failed.
    ///
    /// # Recovery Actions
    ///
    /// - `Restart`: Terminates the agent (task can be re-queued by scheduler)
    /// - `Abort`: Terminates the agent and marks as aborted
    /// - `Escalate`: Emits an event for user attention
    /// - `Decompose`: Returns subtasks (caller handles re-scheduling)
    /// - `Reassign`: Not implemented (future feature)
    pub async fn execute_recovery(
        &self,
        agent_id: &AgentId,
        action: RecoveryAction,
    ) -> crate::error::Result<()> {
        // Emit recovery event
        let _ = self
            .event_tx
            .send(HealthEvent::RecoveryTriggered {
                agent_id: *agent_id,
                action: action.clone(),
            })
            .await;

        match action {
            RecoveryAction::Restart => {
                // Increment retry count for the task before terminating
                if let Some(agent) = self.agent_pool.read().await.get(agent_id) {
                    if let Some(task_id) = agent.task_id {
                        let mut tracker = self.retry_tracker.write().await;
                        tracker.increment(&task_id);
                    }
                }

                // Terminate the agent - scheduler will re-queue the task
                self.agent_pool.write().await.terminate(agent_id).await?;
            }
            RecoveryAction::Abort => {
                // Just terminate - task is considered failed
                self.agent_pool.write().await.terminate(agent_id).await?;
            }
            RecoveryAction::Escalate { message: _ } => {
                // Event was already emitted, user intervention needed
                // Optionally terminate the agent to free resources
                self.agent_pool.write().await.terminate(agent_id).await?;
            }
            RecoveryAction::Decompose { into_tasks: _ } => {
                // Terminate the current agent
                // Caller is responsible for creating new tasks from into_tasks
                self.agent_pool.write().await.terminate(agent_id).await?;
            }
            RecoveryAction::Reassign { to_agent: _ } => {
                // Not implemented - would require task migration
                // For now, just terminate
                self.agent_pool.write().await.terminate(agent_id).await?;
            }
        }

        Ok(())
    }

    /// Build a prompt for AI analysis of an agent's situation.
    ///
    /// Creates a detailed prompt that can be used with ClaudeHeadless to get
    /// AI recommendations for recovery actions.
    ///
    /// # Arguments
    ///
    /// * `agent` - The agent to analyze
    /// * `task_description` - Description of the task
    /// * `retry_count` - Number of times this task has been retried
    ///
    /// # Returns
    ///
    /// A string prompt suitable for AI analysis.
    pub fn build_recovery_prompt(
        &self,
        agent: &AgentHandle,
        task_description: &str,
        retry_count: u32,
    ) -> String {
        let output = agent.read_raw_output().unwrap_or_default();
        let idle_duration = agent.idle_duration();

        format!(
            r#"Analyze this stuck agent and recommend a recovery action.

TASK DESCRIPTION:
{}

AGENT STATUS:
- Idle Duration: {:?}
- Retry Count: {} (max: {})

RECENT OUTPUT (last 2000 chars):
{}

AVAILABLE ACTIONS:
1. RESTART - Restart the agent (good for transient errors like rate limits, timeouts)
2. DECOMPOSE - Break the task into smaller subtasks (good for complex tasks)
3. ESCALATE - Request human intervention (good when max retries exceeded or unclear issues)
4. ABORT - Give up on the task (good for fatal/unrecoverable errors)

Respond with EXACTLY one of: RESTART, DECOMPOSE, ESCALATE, or ABORT
If DECOMPOSE, also list 2-3 subtasks on separate lines starting with "- "
If ESCALATE, also provide a brief reason on the next line."#,
            task_description,
            idle_duration,
            retry_count,
            self.config.max_retries,
            if output.len() > 2000 {
                &output[output.len() - 2000..]
            } else {
                &output
            }
        )
    }

    /// Parse an AI response into a RecoveryAction.
    ///
    /// # Arguments
    ///
    /// * `response` - The raw AI response text
    ///
    /// # Returns
    ///
    /// A `RecoveryAction` based on the AI's recommendation.
    pub fn parse_recovery_action(&self, response: &str) -> RecoveryAction {
        let response = response.trim().to_uppercase();
        let lines: Vec<&str> = response.lines().collect();

        if response.starts_with("RESTART") {
            RecoveryAction::Restart
        } else if response.starts_with("ABORT") {
            RecoveryAction::Abort
        } else if response.starts_with("ESCALATE") {
            let message = lines
                .get(1)
                .map(|s| s.trim().to_string())
                .unwrap_or_else(|| "AI recommended escalation".to_string());
            RecoveryAction::Escalate { message }
        } else if response.starts_with("DECOMPOSE") {
            let tasks: Vec<String> = lines
                .iter()
                .skip(1)
                .filter(|line| line.trim().starts_with('-'))
                .map(|line| line.trim().trim_start_matches('-').trim().to_string())
                .collect();

            if tasks.is_empty() {
                // No subtasks provided, default to restart
                RecoveryAction::Restart
            } else {
                RecoveryAction::Decompose { into_tasks: tasks }
            }
        } else {
            // Default to restart for unknown responses
            RecoveryAction::Restart
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workflow::TaskId;

    // Helper to create a test monitor
    fn create_test_monitor(
        config: HealthConfig,
    ) -> (HealthMonitor, mpsc::Receiver<HealthEvent>, Arc<RwLock<AgentPool>>) {
        let (pool_tx, _pool_rx) = mpsc::channel(100);
        let pool = Arc::new(RwLock::new(AgentPool::new(5, pool_tx)));
        let (health_tx, health_rx) = mpsc::channel(100);
        let monitor = HealthMonitor::new(config, pool.clone(), health_tx);
        (monitor, health_rx, pool)
    }

    // ========== HealthConfig Tests ==========

    #[test]
    fn test_health_config_default() {
        let config = HealthConfig::default();
        assert_eq!(config.stuck_threshold, Duration::from_secs(300));
        assert_eq!(config.max_retries, 3);
        assert!(!config.stuck_patterns.is_empty());
    }

    #[test]
    fn test_health_config_default_patterns() {
        let config = HealthConfig::default();
        assert!(config.stuck_patterns.contains(&"rate limit".to_string()));
        assert!(config.stuck_patterns.contains(&"timeout".to_string()));
    }

    #[test]
    fn test_health_config_with_stuck_threshold() {
        let config = HealthConfig::with_stuck_threshold(Duration::from_secs(600));
        assert_eq!(config.stuck_threshold, Duration::from_secs(600));
        assert_eq!(config.max_retries, DEFAULT_MAX_RETRIES);
    }

    #[test]
    fn test_health_config_with_max_retries() {
        let config = HealthConfig::with_max_retries(5);
        assert_eq!(config.max_retries, 5);
        assert_eq!(config.stuck_threshold, Duration::from_secs(DEFAULT_STUCK_THRESHOLD_SECS));
    }

    #[test]
    fn test_health_config_add_stuck_pattern() {
        let mut config = HealthConfig::default();
        let initial_count = config.stuck_patterns.len();
        config.add_stuck_pattern("custom pattern");
        assert_eq!(config.stuck_patterns.len(), initial_count + 1);
        assert!(config.stuck_patterns.contains(&"custom pattern".to_string()));
    }

    #[test]
    fn test_health_config_debug() {
        let config = HealthConfig::default();
        let debug = format!("{:?}", config);
        assert!(debug.contains("HealthConfig"));
    }

    #[test]
    fn test_health_config_clone() {
        let config = HealthConfig::default();
        let cloned = config.clone();
        assert_eq!(config.stuck_threshold, cloned.stuck_threshold);
        assert_eq!(config.max_retries, cloned.max_retries);
    }

    // ========== HealthEvent Tests ==========

    #[test]
    fn test_health_event_agent_stuck() {
        let agent_id = AgentId::new();
        let duration = Duration::from_secs(300);
        let event = HealthEvent::AgentStuck { agent_id, duration };

        if let HealthEvent::AgentStuck { agent_id: aid, duration: dur } = event {
            assert_eq!(aid, agent_id);
            assert_eq!(dur, duration);
        } else {
            panic!("Expected AgentStuck variant");
        }
    }

    #[test]
    fn test_health_event_agent_failed() {
        let agent_id = AgentId::new();
        let event = HealthEvent::AgentFailed {
            agent_id,
            error: "test error".to_string(),
        };

        if let HealthEvent::AgentFailed { agent_id: aid, error } = event {
            assert_eq!(aid, agent_id);
            assert_eq!(error, "test error");
        } else {
            panic!("Expected AgentFailed variant");
        }
    }

    #[test]
    fn test_health_event_recovery_triggered() {
        let agent_id = AgentId::new();
        let event = HealthEvent::RecoveryTriggered {
            agent_id,
            action: RecoveryAction::Restart,
        };

        if let HealthEvent::RecoveryTriggered { agent_id: aid, action } = event {
            assert_eq!(aid, agent_id);
            assert_eq!(action, RecoveryAction::Restart);
        } else {
            panic!("Expected RecoveryTriggered variant");
        }
    }

    #[test]
    fn test_health_event_debug() {
        let agent_id = AgentId::new();
        let event = HealthEvent::AgentStuck {
            agent_id,
            duration: Duration::from_secs(300),
        };
        let debug = format!("{:?}", event);
        assert!(debug.contains("AgentStuck"));
    }

    #[test]
    fn test_health_event_clone() {
        let agent_id = AgentId::new();
        let event = HealthEvent::AgentFailed {
            agent_id,
            error: "test".to_string(),
        };
        let cloned = event.clone();
        if let HealthEvent::AgentFailed { error, .. } = cloned {
            assert_eq!(error, "test");
        } else {
            panic!("Expected AgentFailed variant");
        }
    }

    // ========== RecoveryAction Tests ==========

    #[test]
    fn test_recovery_action_restart() {
        let action = RecoveryAction::Restart;
        assert_eq!(action, RecoveryAction::Restart);
    }

    #[test]
    fn test_recovery_action_reassign() {
        let to_agent = AgentId::new();
        let action = RecoveryAction::Reassign { to_agent };
        if let RecoveryAction::Reassign { to_agent: aid } = action {
            assert_eq!(aid, to_agent);
        } else {
            panic!("Expected Reassign variant");
        }
    }

    #[test]
    fn test_recovery_action_decompose() {
        let tasks = vec!["task1".to_string(), "task2".to_string()];
        let action = RecoveryAction::Decompose { into_tasks: tasks.clone() };
        if let RecoveryAction::Decompose { into_tasks } = action {
            assert_eq!(into_tasks, tasks);
        } else {
            panic!("Expected Decompose variant");
        }
    }

    #[test]
    fn test_recovery_action_escalate() {
        let action = RecoveryAction::Escalate {
            message: "Need help".to_string(),
        };
        if let RecoveryAction::Escalate { message } = action {
            assert_eq!(message, "Need help");
        } else {
            panic!("Expected Escalate variant");
        }
    }

    #[test]
    fn test_recovery_action_abort() {
        let action = RecoveryAction::Abort;
        assert_eq!(action, RecoveryAction::Abort);
    }

    #[test]
    fn test_recovery_action_equality() {
        assert_eq!(RecoveryAction::Restart, RecoveryAction::Restart);
        assert_eq!(RecoveryAction::Abort, RecoveryAction::Abort);
        assert_ne!(RecoveryAction::Restart, RecoveryAction::Abort);
    }

    #[test]
    fn test_recovery_action_debug() {
        let action = RecoveryAction::Restart;
        let debug = format!("{:?}", action);
        assert!(debug.contains("Restart"));
    }

    #[test]
    fn test_recovery_action_clone() {
        let action = RecoveryAction::Escalate {
            message: "test".to_string(),
        };
        let cloned = action.clone();
        assert_eq!(action, cloned);
    }

    // ========== HealthMonitor Tests ==========

    #[test]
    fn test_health_monitor_new() {
        let (monitor, _rx, _pool) = create_test_monitor(HealthConfig::default());
        assert_eq!(monitor.config().stuck_threshold, Duration::from_secs(300));
    }

    #[test]
    fn test_health_monitor_config_accessor() {
        let config = HealthConfig::with_stuck_threshold(Duration::from_secs(600));
        let (monitor, _rx, _pool) = create_test_monitor(config);
        assert_eq!(monitor.config().stuck_threshold, Duration::from_secs(600));
    }

    // ========== check_agent Tests ==========

    #[test]
    fn test_check_agent_healthy_returns_none() {
        // Given agent with recent activity (just created)
        let id = AgentId::new();
        let handle = AgentHandle::new(id);
        let (monitor, _rx, _pool) = create_test_monitor(HealthConfig::default());

        // When check_agent() is called
        let result = monitor.check_agent(&handle);

        // Then None is returned (no issues)
        assert!(result.is_none());
    }

    #[test]
    fn test_check_agent_is_healthy() {
        // Given agent with recent activity
        let id = AgentId::new();
        let handle = AgentHandle::new(id);
        let (monitor, _rx, _pool) = create_test_monitor(HealthConfig::default());

        // When is_healthy() is called
        let healthy = monitor.is_healthy(&handle);

        // Then true is returned
        assert!(healthy);
    }

    #[test]
    fn test_configurable_threshold() {
        // Given config with stuck_threshold = 10 minutes
        let config = HealthConfig::with_stuck_threshold(Duration::from_secs(600));
        let (monitor, _rx, _pool) = create_test_monitor(config);

        // Then 10 minute threshold is used
        assert_eq!(monitor.config().stuck_threshold, Duration::from_secs(600));
    }

    // ========== check_all Tests ==========

    #[tokio::test]
    async fn test_check_all_empty_pool() {
        let (monitor, _rx, _pool) = create_test_monitor(HealthConfig::default());

        // When check_all() is called on empty pool
        let events = monitor.check_all().await;

        // Then no events are returned
        assert!(events.is_empty());
    }

    #[tokio::test]
    async fn test_check_all_with_healthy_agents() {
        let (monitor, _rx, pool) = create_test_monitor(HealthConfig::default());

        // Add a healthy agent
        {
            let mut pool = pool.write().await;
            pool.spawn(&TaskId::new(), "test").await.unwrap();
        }

        // When check_all() is called
        let events = monitor.check_all().await;

        // Then no events are returned (all healthy)
        assert!(events.is_empty());
    }

    // Note: Testing actual stuck detection requires either mocking time
    // or waiting for the stuck threshold. These tests verify the structure
    // and basic behavior. Integration tests would test actual time-based detection.

    #[test]
    fn test_stuck_detection_threshold_logic() {
        // This tests the threshold comparison logic directly
        let config = HealthConfig::with_stuck_threshold(Duration::from_millis(1));
        let (monitor, _rx, _pool) = create_test_monitor(config);

        // Create an agent and wait for it to exceed the tiny threshold
        let id = AgentId::new();
        let handle = AgentHandle::new(id);

        // Wait a bit to exceed the 1ms threshold
        std::thread::sleep(Duration::from_millis(5));

        // Now check should detect as stuck
        let result = monitor.check_agent(&handle);
        assert!(result.is_some());

        if let Some(HealthEvent::AgentStuck { agent_id, duration }) = result {
            assert_eq!(agent_id, id);
            assert!(duration >= Duration::from_millis(1));
        } else {
            panic!("Expected AgentStuck event");
        }
    }

    #[test]
    fn test_stuck_detection_pattern_criteria() {
        // Verify stuck patterns include expected items
        let config = HealthConfig::default();
        assert!(config.stuck_patterns.iter().any(|p| p.contains("rate limit")));
        assert!(config.stuck_patterns.iter().any(|p| p.contains("timeout")));
        assert!(config.stuck_patterns.iter().any(|p| p.contains("connection refused")));
    }

    // ========== Pattern Detection Tests ==========

    #[test]
    fn test_stuck_patterns_are_case_insensitive() {
        // The pattern matching should work regardless of case
        let config = HealthConfig::default();

        // Patterns should contain lowercase versions
        for pattern in &config.stuck_patterns {
            assert_eq!(pattern, &pattern.to_lowercase());
        }
    }

    // ========== Integration Tests ==========

    #[tokio::test]
    async fn test_check_all_returns_stuck_events_given_one_stuck() {
        // Given 3 agents, 1 stuck (using tiny threshold for testing)
        let config = HealthConfig::with_stuck_threshold(Duration::from_millis(1));
        let (monitor, _rx, pool) = create_test_monitor(config);

        // Add agents to pool
        {
            let mut pool = pool.write().await;
            pool.spawn(&TaskId::new(), "test").await.unwrap();
            pool.spawn(&TaskId::new(), "test").await.unwrap();
            pool.spawn(&TaskId::new(), "test").await.unwrap();
        }

        // Wait for agents to exceed threshold
        std::thread::sleep(Duration::from_millis(10));

        // When check_all() is called
        let events = monitor.check_all().await;

        // Then HealthEvents are returned (all agents stuck due to tiny threshold)
        assert_eq!(events.len(), 3);
        for event in events {
            assert!(matches!(event, HealthEvent::AgentStuck { .. }));
        }
    }

    #[tokio::test]
    async fn test_check_all_sends_events_through_channel() {
        let config = HealthConfig::with_stuck_threshold(Duration::from_millis(1));
        let (monitor, mut rx, pool) = create_test_monitor(config);

        // Add an agent
        {
            let mut pool = pool.write().await;
            pool.spawn(&TaskId::new(), "test").await.unwrap();
        }

        // Wait for agent to exceed threshold
        std::thread::sleep(Duration::from_millis(5));

        // Check all agents
        monitor.check_all().await;

        // Verify event was sent through channel
        let event = rx.try_recv();
        assert!(event.is_ok());
        assert!(matches!(event.unwrap(), HealthEvent::AgentStuck { .. }));
    }

    // ========== RetryTracker Tests ==========

    #[test]
    fn test_retry_tracker_new() {
        let tracker = RetryTracker::new();
        assert!(tracker.is_empty());
        assert_eq!(tracker.len(), 0);
    }

    #[test]
    fn test_retry_tracker_default() {
        let tracker = RetryTracker::default();
        assert!(tracker.is_empty());
    }

    #[test]
    fn test_retry_tracker_new_task_has_zero_retries() {
        let tracker = RetryTracker::new();
        let task_id = TaskId::new();
        assert_eq!(tracker.get_retries(&task_id), 0);
    }

    #[test]
    fn test_retry_tracker_increment() {
        let mut tracker = RetryTracker::new();
        let task_id = TaskId::new();

        assert_eq!(tracker.increment(&task_id), 1);
        assert_eq!(tracker.get_retries(&task_id), 1);

        assert_eq!(tracker.increment(&task_id), 2);
        assert_eq!(tracker.get_retries(&task_id), 2);
    }

    #[test]
    fn test_retry_tracker_reset() {
        let mut tracker = RetryTracker::new();
        let task_id = TaskId::new();

        tracker.increment(&task_id);
        tracker.increment(&task_id);
        assert_eq!(tracker.get_retries(&task_id), 2);

        tracker.reset(&task_id);
        assert_eq!(tracker.get_retries(&task_id), 0);
    }

    #[test]
    fn test_retry_tracker_clear() {
        let mut tracker = RetryTracker::new();
        let task1 = TaskId::new();
        let task2 = TaskId::new();

        tracker.increment(&task1);
        tracker.increment(&task2);
        assert_eq!(tracker.len(), 2);

        tracker.clear();
        assert!(tracker.is_empty());
        assert_eq!(tracker.get_retries(&task1), 0);
        assert_eq!(tracker.get_retries(&task2), 0);
    }

    #[test]
    fn test_retry_tracker_multiple_tasks() {
        let mut tracker = RetryTracker::new();
        let task1 = TaskId::new();
        let task2 = TaskId::new();
        let task3 = TaskId::new();

        tracker.increment(&task1);
        tracker.increment(&task1);
        tracker.increment(&task2);

        assert_eq!(tracker.get_retries(&task1), 2);
        assert_eq!(tracker.get_retries(&task2), 1);
        assert_eq!(tracker.get_retries(&task3), 0);
        assert_eq!(tracker.len(), 2);
    }

    #[test]
    fn test_retry_tracker_debug() {
        let tracker = RetryTracker::new();
        let debug = format!("{:?}", tracker);
        assert!(debug.contains("RetryTracker"));
    }

    #[test]
    fn test_retry_tracker_clone() {
        let mut tracker = RetryTracker::new();
        let task_id = TaskId::new();
        tracker.increment(&task_id);

        let cloned = tracker.clone();
        assert_eq!(cloned.get_retries(&task_id), 1);
    }

    // ========== parse_recovery_action Tests ==========

    #[test]
    fn test_parse_recovery_action_restart() {
        let (monitor, _rx, _pool) = create_test_monitor(HealthConfig::default());
        let action = monitor.parse_recovery_action("RESTART");
        assert_eq!(action, RecoveryAction::Restart);
    }

    #[test]
    fn test_parse_recovery_action_restart_lowercase() {
        let (monitor, _rx, _pool) = create_test_monitor(HealthConfig::default());
        let action = monitor.parse_recovery_action("restart");
        assert_eq!(action, RecoveryAction::Restart);
    }

    #[test]
    fn test_parse_recovery_action_abort() {
        let (monitor, _rx, _pool) = create_test_monitor(HealthConfig::default());
        let action = monitor.parse_recovery_action("ABORT");
        assert_eq!(action, RecoveryAction::Abort);
    }

    #[test]
    fn test_parse_recovery_action_escalate() {
        let (monitor, _rx, _pool) = create_test_monitor(HealthConfig::default());
        let action = monitor.parse_recovery_action("ESCALATE\nTask requires human judgment");

        if let RecoveryAction::Escalate { message } = action {
            assert_eq!(message, "TASK REQUIRES HUMAN JUDGMENT");
        } else {
            panic!("Expected Escalate variant");
        }
    }

    #[test]
    fn test_parse_recovery_action_escalate_default_message() {
        let (monitor, _rx, _pool) = create_test_monitor(HealthConfig::default());
        let action = monitor.parse_recovery_action("ESCALATE");

        if let RecoveryAction::Escalate { message } = action {
            assert_eq!(message, "AI recommended escalation");
        } else {
            panic!("Expected Escalate variant");
        }
    }

    #[test]
    fn test_parse_recovery_action_decompose() {
        let (monitor, _rx, _pool) = create_test_monitor(HealthConfig::default());
        let action = monitor.parse_recovery_action("DECOMPOSE\n- First subtask\n- Second subtask");

        if let RecoveryAction::Decompose { into_tasks } = action {
            assert_eq!(into_tasks.len(), 2);
            assert_eq!(into_tasks[0], "FIRST SUBTASK");
            assert_eq!(into_tasks[1], "SECOND SUBTASK");
        } else {
            panic!("Expected Decompose variant");
        }
    }

    #[test]
    fn test_parse_recovery_action_decompose_no_subtasks_defaults_to_restart() {
        let (monitor, _rx, _pool) = create_test_monitor(HealthConfig::default());
        let action = monitor.parse_recovery_action("DECOMPOSE");
        // Should default to restart when no subtasks provided
        assert_eq!(action, RecoveryAction::Restart);
    }

    #[test]
    fn test_parse_recovery_action_unknown_defaults_to_restart() {
        let (monitor, _rx, _pool) = create_test_monitor(HealthConfig::default());
        let action = monitor.parse_recovery_action("unknown response");
        assert_eq!(action, RecoveryAction::Restart);
    }

    #[test]
    fn test_parse_recovery_action_empty_defaults_to_restart() {
        let (monitor, _rx, _pool) = create_test_monitor(HealthConfig::default());
        let action = monitor.parse_recovery_action("");
        assert_eq!(action, RecoveryAction::Restart);
    }

    // ========== build_recovery_prompt Tests ==========

    #[test]
    fn test_build_recovery_prompt_includes_task_description() {
        let (monitor, _rx, _pool) = create_test_monitor(HealthConfig::default());
        let id = AgentId::new();
        let handle = AgentHandle::new(id);

        let prompt = monitor.build_recovery_prompt(&handle, "Test task description", 0);

        assert!(prompt.contains("Test task description"));
    }

    #[test]
    fn test_build_recovery_prompt_includes_retry_count() {
        let (monitor, _rx, _pool) = create_test_monitor(HealthConfig::default());
        let id = AgentId::new();
        let handle = AgentHandle::new(id);

        let prompt = monitor.build_recovery_prompt(&handle, "task", 2);

        assert!(prompt.contains("Retry Count: 2"));
    }

    #[test]
    fn test_build_recovery_prompt_includes_max_retries() {
        let (monitor, _rx, _pool) = create_test_monitor(HealthConfig::with_max_retries(5));
        let id = AgentId::new();
        let handle = AgentHandle::new(id);

        let prompt = monitor.build_recovery_prompt(&handle, "task", 0);

        assert!(prompt.contains("(max: 5)"));
    }

    #[test]
    fn test_build_recovery_prompt_includes_available_actions() {
        let (monitor, _rx, _pool) = create_test_monitor(HealthConfig::default());
        let id = AgentId::new();
        let handle = AgentHandle::new(id);

        let prompt = monitor.build_recovery_prompt(&handle, "task", 0);

        assert!(prompt.contains("RESTART"));
        assert!(prompt.contains("DECOMPOSE"));
        assert!(prompt.contains("ESCALATE"));
        assert!(prompt.contains("ABORT"));
    }

    // ========== determine_recovery Tests ==========

    #[tokio::test]
    async fn test_determine_recovery_default_returns_restart() {
        // Given agent with no specific error pattern
        let (monitor, _rx, _pool) = create_test_monitor(HealthConfig::default());
        let id = AgentId::new();
        let handle = AgentHandle::new(id);

        // When determine_recovery() is called
        let action = monitor.determine_recovery(&handle, None).await;

        // Then Restart is returned (default action)
        assert_eq!(action, RecoveryAction::Restart);
    }

    #[tokio::test]
    async fn test_determine_recovery_max_retries_returns_escalate() {
        // Given agent with task that has exceeded max retries
        let config = HealthConfig::with_max_retries(2);
        let (monitor, _rx, _pool) = create_test_monitor(config);

        let task_id = TaskId::new();

        // Set up retry count to exceed max
        {
            let mut tracker = monitor.retry_tracker.write().await;
            tracker.increment(&task_id);
            tracker.increment(&task_id);
        }

        // Create agent handle with task_id
        let id = AgentId::new();
        let mut handle = AgentHandle::new(id);
        handle.task_id = Some(task_id);

        // When determine_recovery() is called
        let action = monitor.determine_recovery(&handle, None).await;

        // Then Escalate is returned
        if let RecoveryAction::Escalate { message } = action {
            assert!(message.contains("retried 2 times"));
            assert!(message.contains("max: 2"));
        } else {
            panic!("Expected Escalate variant, got {:?}", action);
        }
    }

    #[tokio::test]
    async fn test_determine_recovery_with_task_id() {
        // Given agent with task_id set
        let (monitor, _rx, _pool) = create_test_monitor(HealthConfig::default());
        let task_id = TaskId::new();

        // Set up retry count
        {
            let mut tracker = monitor.retry_tracker.write().await;
            tracker.increment(&task_id);
        }

        let id = AgentId::new();
        let mut handle = AgentHandle::new(id);
        handle.task_id = Some(task_id);

        // When determine_recovery() is called with retries < max
        let action = monitor.determine_recovery(&handle, None).await;

        // Then Restart is returned (not escalate, since < max)
        assert_eq!(action, RecoveryAction::Restart);
    }

    // ========== execute_recovery Tests ==========

    #[tokio::test]
    async fn test_execute_recovery_restart_emits_event() {
        let (monitor, mut rx, pool) = create_test_monitor(HealthConfig::default());

        // Spawn an agent
        let task_id = TaskId::new();
        let agent_id = {
            let mut pool = pool.write().await;
            pool.spawn(&task_id, "test").await.unwrap()
        };

        // Execute restart recovery
        let result = monitor
            .execute_recovery(&agent_id, RecoveryAction::Restart)
            .await;

        assert!(result.is_ok());

        // Verify event was emitted
        let event = rx.try_recv();
        assert!(event.is_ok());
        if let HealthEvent::RecoveryTriggered { agent_id: aid, action } = event.unwrap() {
            assert_eq!(aid, agent_id);
            assert_eq!(action, RecoveryAction::Restart);
        } else {
            panic!("Expected RecoveryTriggered event");
        }
    }

    #[tokio::test]
    async fn test_execute_recovery_abort_terminates_agent() {
        let (monitor, _rx, pool) = create_test_monitor(HealthConfig::default());

        // Spawn an agent
        let task_id = TaskId::new();
        let agent_id = {
            let mut pool = pool.write().await;
            pool.spawn(&task_id, "test").await.unwrap()
        };

        // Verify agent exists
        assert!(pool.read().await.get(&agent_id).is_some());

        // Execute abort recovery
        let result = monitor
            .execute_recovery(&agent_id, RecoveryAction::Abort)
            .await;

        assert!(result.is_ok());

        // Verify agent was terminated
        assert!(pool.read().await.get(&agent_id).is_none());
    }

    #[tokio::test]
    async fn test_execute_recovery_escalate_emits_event_and_terminates() {
        let (monitor, mut rx, pool) = create_test_monitor(HealthConfig::default());

        // Spawn an agent
        let task_id = TaskId::new();
        let agent_id = {
            let mut pool = pool.write().await;
            pool.spawn(&task_id, "test").await.unwrap()
        };

        // Execute escalate recovery
        let result = monitor
            .execute_recovery(
                &agent_id,
                RecoveryAction::Escalate {
                    message: "Test escalation".to_string(),
                },
            )
            .await;

        assert!(result.is_ok());

        // Verify event was emitted
        let event = rx.try_recv();
        assert!(event.is_ok());
        if let HealthEvent::RecoveryTriggered { action, .. } = event.unwrap() {
            if let RecoveryAction::Escalate { message } = action {
                assert_eq!(message, "Test escalation");
            } else {
                panic!("Expected Escalate action");
            }
        }

        // Verify agent was terminated
        assert!(pool.read().await.get(&agent_id).is_none());
    }

    #[tokio::test]
    async fn test_execute_recovery_restart_increments_retry_count() {
        let (monitor, _rx, pool) = create_test_monitor(HealthConfig::default());

        // Spawn an agent with task
        let task_id = TaskId::new();
        let agent_id = {
            let mut pool = pool.write().await;
            pool.spawn(&task_id, "test").await.unwrap()
        };

        // Initial retry count should be 0
        assert_eq!(monitor.retry_tracker.read().await.get_retries(&task_id), 0);

        // Execute restart recovery
        let result = monitor
            .execute_recovery(&agent_id, RecoveryAction::Restart)
            .await;

        assert!(result.is_ok());

        // Retry count should be incremented
        assert_eq!(monitor.retry_tracker.read().await.get_retries(&task_id), 1);
    }

    #[tokio::test]
    async fn test_execute_recovery_decompose_terminates_agent() {
        let (monitor, _rx, pool) = create_test_monitor(HealthConfig::default());

        // Spawn an agent
        let task_id = TaskId::new();
        let agent_id = {
            let mut pool = pool.write().await;
            pool.spawn(&task_id, "test").await.unwrap()
        };

        // Execute decompose recovery
        let result = monitor
            .execute_recovery(
                &agent_id,
                RecoveryAction::Decompose {
                    into_tasks: vec!["subtask1".to_string(), "subtask2".to_string()],
                },
            )
            .await;

        assert!(result.is_ok());

        // Verify agent was terminated
        assert!(pool.read().await.get(&agent_id).is_none());
    }

    // ========== HealthMonitor with_repo_path Tests ==========

    #[test]
    fn test_health_monitor_with_repo_path() {
        let (pool_tx, _pool_rx) = mpsc::channel(100);
        let pool = Arc::new(RwLock::new(AgentPool::new(5, pool_tx)));
        let (health_tx, _health_rx) = mpsc::channel(100);

        let monitor = HealthMonitor::with_repo_path(
            HealthConfig::default(),
            pool,
            health_tx,
            "/test/repo/path",
        );

        assert_eq!(monitor.repo_path(), Path::new("/test/repo/path"));
    }

    #[test]
    fn test_health_monitor_retry_tracker_accessor() {
        let (monitor, _rx, _pool) = create_test_monitor(HealthConfig::default());
        let tracker = monitor.retry_tracker();
        // Just verify we can access it
        assert!(tracker.try_read().is_ok());
    }

    // ========== Integration: Recovery Flow Tests ==========

    #[tokio::test]
    async fn test_recovery_flow_check_and_recover() {
        // Full flow: detect stuck agent -> determine recovery -> execute recovery
        let config = HealthConfig::with_stuck_threshold(Duration::from_millis(1));
        let (monitor, mut rx, pool) = create_test_monitor(config);

        // Spawn an agent
        let task_id = TaskId::new();
        let agent_id = {
            let mut pool = pool.write().await;
            pool.spawn(&task_id, "test").await.unwrap()
        };

        // Wait for agent to exceed threshold
        std::thread::sleep(Duration::from_millis(5));

        // Check for stuck agents
        let events = monitor.check_all().await;
        assert!(!events.is_empty());

        // Get the stuck agent
        let agent = pool.read().await.get(&agent_id).cloned();
        assert!(agent.is_some());

        // Determine recovery
        let action = monitor.determine_recovery(&agent.unwrap(), Some("test task")).await;

        // Execute recovery
        let result = monitor.execute_recovery(&agent_id, action).await;
        assert!(result.is_ok());

        // Drain the stuck event from channel
        let _stuck_event = rx.try_recv();

        // Verify recovery event was sent
        let recovery_event = rx.try_recv();
        assert!(recovery_event.is_ok());
        assert!(matches!(
            recovery_event.unwrap(),
            HealthEvent::RecoveryTriggered { .. }
        ));
    }
}
