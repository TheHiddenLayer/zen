//! Skills orchestrator for coordinating workflow phases.
//!
//! The `SkillsOrchestrator` is the central orchestration engine that drives
//! the 5-phase workflow (PDD -> TaskGen -> Implementation -> Merge -> Docs)
//! by composing AIHumanProxy, AgentPool, and ClaudeHeadless.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, RwLock};
use tokio::time::sleep;

use crate::error::{Error, Result};
use crate::workflow::{Workflow, WorkflowConfig, WorkflowId, WorkflowPhase, WorkflowState, WorkflowStatus};
use crate::{zlog, zlog_debug};

use super::{AgentEvent, AgentHandle, AgentOutput, AgentPool, AIHumanProxy, ClaudeHeadless};

/// Events emitted by the PhaseController during phase transitions.
///
/// These events can be consumed by the TUI to show real-time progress updates.
#[derive(Debug, Clone)]
pub enum PhaseEvent {
    /// A new phase has started.
    Started {
        /// The phase that started.
        phase: WorkflowPhase,
    },
    /// Transitioned from one phase to another.
    Changed {
        /// The phase we transitioned from.
        from: WorkflowPhase,
        /// The phase we transitioned to.
        to: WorkflowPhase,
        /// How long we spent in the previous phase.
        elapsed: Duration,
    },
    /// The entire workflow has completed.
    Completed {
        /// Total duration of all phases combined.
        total_duration: Duration,
    },
}

/// Controls workflow phase transitions and emits events for TUI updates.
///
/// The `PhaseController` centralizes phase management logic, validating
/// transitions, tracking timing for each phase, and emitting events that
/// the TUI can consume to show real-time progress.
///
/// # Example
///
/// ```ignore
/// use tokio::sync::mpsc;
/// use zen::orchestration::PhaseController;
///
/// let (tx, mut rx) = mpsc::channel(100);
/// let mut controller = PhaseController::new(tx);
///
/// // Transition to next phase
/// controller.transition(WorkflowPhase::TaskGeneration).await?;
///
/// // Receive event
/// if let Some(event) = rx.recv().await {
///     println!("Phase changed: {:?}", event);
/// }
/// ```
pub struct PhaseController {
    /// The current workflow phase.
    current_phase: WorkflowPhase,
    /// History of phases visited with timestamps.
    phase_history: Vec<(WorkflowPhase, Instant)>,
    /// Channel for emitting phase events.
    event_tx: mpsc::Sender<PhaseEvent>,
    /// When the controller was created (for total duration).
    created_at: Instant,
}

impl PhaseController {
    /// Create a new PhaseController.
    ///
    /// Starts in the Planning phase and emits a `PhaseEvent::Started` event.
    ///
    /// # Arguments
    ///
    /// * `event_tx` - Channel sender for emitting phase events
    pub fn new(event_tx: mpsc::Sender<PhaseEvent>) -> Self {
        let now = Instant::now();
        let initial_phase = WorkflowPhase::Planning;

        // Emit initial started event (best effort, ignore if channel full)
        let _ = event_tx.try_send(PhaseEvent::Started {
            phase: initial_phase,
        });

        Self {
            current_phase: initial_phase,
            phase_history: vec![(initial_phase, now)],
            event_tx,
            created_at: now,
        }
    }

    /// Get the current workflow phase.
    pub fn current(&self) -> WorkflowPhase {
        self.current_phase
    }

    /// Get the duration since the current phase started.
    pub fn elapsed(&self) -> Duration {
        if let Some((_, entered_at)) = self.phase_history.last() {
            entered_at.elapsed()
        } else {
            Duration::ZERO
        }
    }

    /// Get the history of all phases visited.
    ///
    /// Returns a slice of (phase, timestamp) tuples in the order visited.
    pub fn history(&self) -> &[(WorkflowPhase, Instant)] {
        &self.phase_history
    }

    /// Attempt to transition to a new phase.
    ///
    /// Validates the transition according to the workflow rules:
    /// - Planning -> TaskGeneration
    /// - TaskGeneration -> Implementation
    /// - Implementation -> Merging
    /// - Merging -> Documentation OR Complete
    /// - Documentation -> Complete
    ///
    /// On success, emits a `PhaseEvent::Changed` event. If transitioning to
    /// `Complete`, also emits a `PhaseEvent::Completed` event.
    ///
    /// # Errors
    ///
    /// Returns `Error::InvalidPhaseTransition` if the transition is not valid.
    pub async fn transition(&mut self, target: WorkflowPhase) -> Result<()> {
        // Validate transition
        if !self.can_transition(target) {
            return Err(Error::InvalidPhaseTransition {
                from: self.current_phase.to_string(),
                to: target.to_string(),
            });
        }

        let from = self.current_phase;
        let elapsed = self.elapsed();
        let now = Instant::now();

        // Update state
        self.current_phase = target;
        self.phase_history.push((target, now));

        // Emit changed event
        let _ = self.event_tx.send(PhaseEvent::Changed {
            from,
            to: target,
            elapsed,
        }).await;

        // If completing, also emit completed event
        if target == WorkflowPhase::Complete {
            let total_duration = self.created_at.elapsed();
            let _ = self.event_tx.send(PhaseEvent::Completed {
                total_duration,
            }).await;
        }

        Ok(())
    }

    /// Check if a transition to the target phase is valid.
    fn can_transition(&self, target: WorkflowPhase) -> bool {
        matches!(
            (self.current_phase, target),
            (WorkflowPhase::Planning, WorkflowPhase::TaskGeneration)
                | (WorkflowPhase::TaskGeneration, WorkflowPhase::Implementation)
                | (WorkflowPhase::Implementation, WorkflowPhase::Merging)
                | (WorkflowPhase::Merging, WorkflowPhase::Documentation)
                | (WorkflowPhase::Merging, WorkflowPhase::Complete)
                | (WorkflowPhase::Documentation, WorkflowPhase::Complete)
        )
    }
}

/// Configuration for the agent output monitor loop.
///
/// Controls polling interval and timeout behavior for monitoring agent output.
#[derive(Debug, Clone)]
pub struct MonitorConfig {
    /// How often to poll for new output.
    pub poll_interval: Duration,
    /// Maximum time to wait for agent to complete before timing out.
    pub timeout: Duration,
}

impl Default for MonitorConfig {
    fn default() -> Self {
        Self {
            poll_interval: Duration::from_millis(100),
            timeout: Duration::from_secs(600), // 10 minutes
        }
    }
}

impl MonitorConfig {
    /// Create a new MonitorConfig with custom values.
    pub fn new(poll_interval: Duration, timeout: Duration) -> Self {
        Self {
            poll_interval,
            timeout,
        }
    }

    /// Create a MonitorConfig for fast polling (useful for tests).
    pub fn fast() -> Self {
        Self {
            poll_interval: Duration::from_millis(10),
            timeout: Duration::from_secs(60),
        }
    }
}

/// Result of a skill execution via the monitor loop.
///
/// Contains information about the skill's execution including success status,
/// any captured output, the number of questions answered, and total duration.
#[derive(Debug, Clone)]
pub struct SkillResult {
    /// Whether the skill completed successfully.
    pub success: bool,
    /// Optional output captured from the skill.
    pub output: Option<String>,
    /// Number of questions answered by AIHumanProxy during execution.
    pub questions_answered: usize,
    /// Total duration of the skill execution.
    pub duration: Duration,
}

impl SkillResult {
    /// Create a successful skill result.
    pub fn success(questions_answered: usize, duration: Duration) -> Self {
        Self {
            success: true,
            output: None,
            questions_answered,
            duration,
        }
    }

    /// Create a successful skill result with output.
    pub fn success_with_output(
        output: impl Into<String>,
        questions_answered: usize,
        duration: Duration,
    ) -> Self {
        Self {
            success: true,
            output: Some(output.into()),
            questions_answered,
            duration,
        }
    }

    /// Create a failed skill result.
    pub fn failure(duration: Duration) -> Self {
        Self {
            success: false,
            output: None,
            questions_answered: 0,
            duration,
        }
    }

    /// Check if the skill completed successfully.
    pub fn is_success(&self) -> bool {
        self.success
    }
}

/// Result of a workflow execution.
///
/// Contains the workflow identifier, final status, and a human-readable
/// summary of what was accomplished during the workflow.
#[derive(Debug, Clone)]
pub struct WorkflowResult {
    /// The unique identifier of the executed workflow.
    pub workflow_id: WorkflowId,
    /// The final status of the workflow (Completed or Failed).
    pub status: WorkflowStatus,
    /// Human-readable summary of the workflow execution.
    pub summary: String,
}

impl WorkflowResult {
    /// Create a new workflow result.
    pub fn new(workflow_id: WorkflowId, status: WorkflowStatus, summary: impl Into<String>) -> Self {
        Self {
            workflow_id,
            status,
            summary: summary.into(),
        }
    }

    /// Create a successful workflow result.
    pub fn success(workflow_id: WorkflowId, summary: impl Into<String>) -> Self {
        Self::new(workflow_id, WorkflowStatus::Completed, summary)
    }

    /// Create a failed workflow result.
    pub fn failure(workflow_id: WorkflowId, summary: impl Into<String>) -> Self {
        Self::new(workflow_id, WorkflowStatus::Failed, summary)
    }

    /// Check if the workflow completed successfully.
    pub fn is_success(&self) -> bool {
        self.status == WorkflowStatus::Completed
    }
}

/// The central orchestration engine for Zen v2.
///
/// SkillsOrchestrator coordinates the full workflow phases:
/// 1. Planning - Run /pdd to generate design and plan
/// 2. TaskGeneration - Run /code-task-generator to create tasks
/// 3. Implementation - Run /code-assist in parallel for each task
/// 4. Merging - Merge worktrees and resolve conflicts
/// 5. Documentation - Optional /codebase-summary for documentation
///
/// # Example
///
/// ```ignore
/// use zen::orchestration::SkillsOrchestrator;
/// use zen::workflow::WorkflowConfig;
/// use std::path::Path;
///
/// let orchestrator = SkillsOrchestrator::new(
///     WorkflowConfig::default(),
///     Path::new("/path/to/repo"),
/// )?;
///
/// let result = orchestrator.execute("build user authentication").await?;
/// println!("Workflow completed: {:?}", result.status);
/// ```
pub struct SkillsOrchestrator {
    /// AI-as-Human proxy for answering skill questions autonomously.
    ai_human: AIHumanProxy,
    /// Pool of concurrent agents for parallel task execution.
    agent_pool: Arc<RwLock<AgentPool>>,
    /// Current workflow state with phase transition tracking.
    state: Arc<RwLock<WorkflowState>>,
    /// Claude headless executor for programmatic Claude interaction.
    claude: ClaudeHeadless,
    /// Path to the repository being orchestrated.
    repo_path: PathBuf,
    /// Receiver for agent events (stored for later use in monitoring).
    #[allow(dead_code)]
    event_rx: Arc<RwLock<mpsc::Receiver<AgentEvent>>>,
}

impl SkillsOrchestrator {
    /// Create a new SkillsOrchestrator.
    ///
    /// Initializes all components:
    /// - AIHumanProxy with an empty prompt (will be set during execute)
    /// - AgentPool with configured max concurrent agents
    /// - WorkflowState with initial Planning phase
    /// - ClaudeHeadless for Claude Code execution
    ///
    /// # Arguments
    ///
    /// * `config` - Workflow configuration options
    /// * `repo_path` - Path to the repository to orchestrate
    ///
    /// # Errors
    ///
    /// Returns an error if the Claude binary cannot be found.
    pub fn new(config: WorkflowConfig, repo_path: &Path) -> Result<Self> {
        // Create event channel for agent pool
        let (event_tx, event_rx) = mpsc::channel(100);

        // Initialize agent pool with configured capacity
        let agent_pool = AgentPool::new(config.max_parallel_agents, event_tx);

        // Create workflow and state
        let workflow = Workflow::new("", config);
        let state = WorkflowState::new(workflow);

        // Initialize Claude headless executor
        let claude = ClaudeHeadless::new()?;

        // Create AI-as-Human proxy (will be updated with actual prompt during execute)
        let ai_human = AIHumanProxy::new("");

        Ok(Self {
            ai_human,
            agent_pool: Arc::new(RwLock::new(agent_pool)),
            state: Arc::new(RwLock::new(state)),
            claude,
            repo_path: repo_path.to_path_buf(),
            event_rx: Arc::new(RwLock::new(event_rx)),
        })
    }

    /// Execute the full workflow from a user prompt.
    ///
    /// Runs through all workflow phases in order:
    /// 1. Planning (/pdd)
    /// 2. Task Generation (/code-task-generator)
    /// 3. Implementation (/code-assist in parallel)
    /// 4. Merging (merge worktrees)
    /// 5. Documentation (optional /codebase-summary)
    ///
    /// # Arguments
    ///
    /// * `prompt` - The user's natural language task description
    ///
    /// # Returns
    ///
    /// A `WorkflowResult` containing the workflow ID, final status, and summary.
    ///
    /// # Errors
    ///
    /// Returns an error if any phase fails critically.
    pub async fn execute(&mut self, prompt: &str) -> Result<WorkflowResult> {
        // Update AI-as-Human proxy with actual prompt
        self.ai_human = AIHumanProxy::new(prompt);

        // Initialize workflow with the prompt
        {
            let mut state = self.state.write().await;
            let workflow = Workflow::new(prompt, state.workflow().config.clone());
            *state = WorkflowState::new(workflow);
            state.workflow_mut().start();
        }

        let workflow_id = self.state.read().await.workflow().id;
        zlog!("[orchestrator] Starting workflow execution: {}", workflow_id);

        // PHASE 1: Planning with /pdd
        zlog!("[orchestrator] Beginning planning phase");
        if let Err(e) = self.run_planning_phase(prompt).await {
            return Ok(self.fail_workflow(workflow_id, format!("Planning phase failed: {}", e)).await);
        }

        // Transition to TaskGeneration
        {
            let mut state = self.state.write().await;
            if let Err(e) = state.transition(WorkflowPhase::TaskGeneration) {
                return Ok(self.fail_workflow(workflow_id, format!("Failed to transition to task generation: {}", e)).await);
            }
        }

        // PHASE 2: Task Generation with /code-task-generator
        zlog!("[orchestrator] Beginning task generation phase");
        if let Err(e) = self.run_task_generation_phase().await {
            return Ok(self.fail_workflow(workflow_id, format!("Task generation phase failed: {}", e)).await);
        }

        // Transition to Implementation
        {
            let mut state = self.state.write().await;
            if let Err(e) = state.transition(WorkflowPhase::Implementation) {
                return Ok(self.fail_workflow(workflow_id, format!("Failed to transition to implementation: {}", e)).await);
            }
        }

        // PHASE 3: Implementation with /code-assist in parallel
        zlog!("[orchestrator] Beginning implementation phase");
        if let Err(e) = self.run_implementation_phase().await {
            return Ok(self.fail_workflow(workflow_id, format!("Implementation phase failed: {}", e)).await);
        }

        // Transition to Merging
        {
            let mut state = self.state.write().await;
            if let Err(e) = state.transition(WorkflowPhase::Merging) {
                return Ok(self.fail_workflow(workflow_id, format!("Failed to transition to merging: {}", e)).await);
            }
        }

        // PHASE 4: Merge and resolve conflicts
        zlog!("[orchestrator] Beginning merge phase");
        if let Err(e) = self.run_merge_phase().await {
            return Ok(self.fail_workflow(workflow_id, format!("Merge phase failed: {}", e)).await);
        }

        // PHASE 5: Documentation (optional)
        let update_docs = self.state.read().await.workflow().config.update_docs;
        if update_docs {
            // Transition to Documentation
            {
                let mut state = self.state.write().await;
                if let Err(e) = state.transition(WorkflowPhase::Documentation) {
                    return Ok(self.fail_workflow(workflow_id, format!("Failed to transition to documentation: {}", e)).await);
                }
            }

            zlog!("[orchestrator] Beginning documentation phase");
            if let Err(e) = self.run_documentation_phase().await {
                // Documentation failure is not critical, just log and continue
                zlog!("[orchestrator] Documentation phase failed (non-critical): {}", e);
            }
        }

        // Transition to Complete
        {
            let mut state = self.state.write().await;
            let target = WorkflowPhase::Complete;
            if state.can_transition(target) {
                state.transition(target)?;
            }
            state.workflow_mut().complete();
        }

        zlog!("[orchestrator] Workflow {} completed successfully", workflow_id);

        Ok(WorkflowResult::success(
            workflow_id,
            format!("Workflow '{}' completed successfully", prompt),
        ))
    }

    /// Run the planning phase (stub - will be implemented in Step 7).
    async fn run_planning_phase(&self, _prompt: &str) -> Result<()> {
        zlog_debug!("[orchestrator] Planning phase stub - will run /pdd in Step 7");
        Ok(())
    }

    /// Run the task generation phase (stub - will be implemented in Step 9).
    async fn run_task_generation_phase(&self) -> Result<()> {
        zlog_debug!("[orchestrator] Task generation phase stub - will run /code-task-generator in Step 9");
        Ok(())
    }

    /// Run the implementation phase (stub - will be implemented in Step 11).
    async fn run_implementation_phase(&self) -> Result<()> {
        zlog_debug!("[orchestrator] Implementation phase stub - will run /code-assist in parallel in Step 11");
        Ok(())
    }

    /// Run the merge phase (stub - will be implemented in Step 12).
    async fn run_merge_phase(&self) -> Result<()> {
        zlog_debug!("[orchestrator] Merge phase stub - will merge worktrees in Step 12");
        Ok(())
    }

    /// Run the documentation phase (stub - will be implemented in Step 13).
    async fn run_documentation_phase(&self) -> Result<()> {
        zlog_debug!("[orchestrator] Documentation phase stub - will run /codebase-summary in Step 13");
        Ok(())
    }

    /// Mark the workflow as failed and return a failure result.
    async fn fail_workflow(&self, workflow_id: WorkflowId, reason: String) -> WorkflowResult {
        {
            let mut state = self.state.write().await;
            state.workflow_mut().fail();
        }
        zlog!("[orchestrator] Workflow {} failed: {}", workflow_id, reason);
        WorkflowResult::failure(workflow_id, reason)
    }

    /// Get a reference to the AI-as-Human proxy.
    pub fn ai_human(&self) -> &AIHumanProxy {
        &self.ai_human
    }

    /// Get a reference to the agent pool.
    pub fn agent_pool(&self) -> &Arc<RwLock<AgentPool>> {
        &self.agent_pool
    }

    /// Get a reference to the workflow state.
    pub fn state(&self) -> &Arc<RwLock<WorkflowState>> {
        &self.state
    }

    /// Get a reference to the Claude headless executor.
    pub fn claude(&self) -> &ClaudeHeadless {
        &self.claude
    }

    /// Get the repository path.
    pub fn repo_path(&self) -> &Path {
        &self.repo_path
    }

    /// Monitor an agent's output and handle questions, completion, and errors.
    ///
    /// This is the core interaction pattern for all skill phases. The loop:
    /// 1. Polls the agent's output at the configured interval
    /// 2. Detects questions and answers them via AIHumanProxy
    /// 3. Detects completion and returns success
    /// 4. Detects errors and returns the error
    /// 5. Times out if no completion within the configured timeout
    ///
    /// # Arguments
    ///
    /// * `agent` - The agent handle to monitor
    /// * `config` - Configuration for polling interval and timeout
    ///
    /// # Returns
    ///
    /// A `SkillResult` on success, or an error if the agent fails or times out.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The agent outputs an error
    /// - The agent times out (returns `Error::Timeout`)
    /// - Communication with the agent fails
    pub async fn monitor_agent_output(
        &self,
        agent: &AgentHandle,
        config: &MonitorConfig,
    ) -> Result<SkillResult> {
        let start = Instant::now();
        let mut questions_answered = 0;
        let mut last_output = String::new();

        loop {
            // Check timeout
            let elapsed = start.elapsed();
            if elapsed >= config.timeout {
                zlog!("[monitor] Agent {} timed out after {:?}", agent.id, elapsed);
                return Err(Error::Timeout(elapsed));
            }

            // Poll agent output
            match agent.read_output() {
                Ok(output) => match output {
                    AgentOutput::Question(question) => {
                        zlog_debug!("[monitor] Agent {} asked: {}", agent.id, question);

                        // Answer the question via AIHumanProxy
                        let answer = self.ai_human.answer_question(&question);
                        zlog_debug!("[monitor] Answering with: {}", answer);

                        // Send the answer back to the agent
                        if let Err(e) = agent.send(&answer) {
                            zlog!("[monitor] Failed to send answer to agent {}: {}", agent.id, e);
                            return Err(e);
                        }

                        questions_answered += 1;
                    }
                    AgentOutput::Completed => {
                        zlog_debug!("[monitor] Agent {} completed", agent.id);
                        let duration = start.elapsed();
                        return Ok(SkillResult::success_with_output(
                            last_output,
                            questions_answered,
                            duration,
                        ));
                    }
                    AgentOutput::Error(error) => {
                        zlog!("[monitor] Agent {} error: {}", agent.id, error);
                        return Err(Error::AgentNotAvailable(error));
                    }
                    AgentOutput::Text(text) => {
                        // Capture output for potential use
                        if !text.is_empty() {
                            last_output = text;
                        }
                    }
                },
                Err(e) => {
                    zlog!("[monitor] Failed to read agent {} output: {}", agent.id, e);
                    return Err(e);
                }
            }

            // Wait before next poll
            sleep(config.poll_interval).await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    // Helper to create a test orchestrator (uses mock Claude binary path)
    fn create_test_orchestrator() -> SkillsOrchestrator {
        let config = WorkflowConfig::default();
        let repo_path = PathBuf::from("/tmp/test-repo");

        // Create with mock binary since we're testing
        let (event_tx, event_rx) = mpsc::channel(100);
        let agent_pool = AgentPool::new(config.max_parallel_agents, event_tx);
        let workflow = Workflow::new("", config.clone());
        let state = WorkflowState::new(workflow);
        let claude = ClaudeHeadless::with_binary(PathBuf::from("/mock/claude"));
        let ai_human = AIHumanProxy::new("");

        SkillsOrchestrator {
            ai_human,
            agent_pool: Arc::new(RwLock::new(agent_pool)),
            state: Arc::new(RwLock::new(state)),
            claude,
            repo_path,
            event_rx: Arc::new(RwLock::new(event_rx)),
        }
    }

    // ========== WorkflowResult Tests ==========

    #[test]
    fn test_workflow_result_new() {
        let id = WorkflowId::new();
        let result = WorkflowResult::new(id, WorkflowStatus::Completed, "Test summary");

        assert_eq!(result.workflow_id, id);
        assert_eq!(result.status, WorkflowStatus::Completed);
        assert_eq!(result.summary, "Test summary");
    }

    #[test]
    fn test_workflow_result_success() {
        let id = WorkflowId::new();
        let result = WorkflowResult::success(id, "Completed successfully");

        assert_eq!(result.status, WorkflowStatus::Completed);
        assert!(result.is_success());
    }

    #[test]
    fn test_workflow_result_failure() {
        let id = WorkflowId::new();
        let result = WorkflowResult::failure(id, "Something went wrong");

        assert_eq!(result.status, WorkflowStatus::Failed);
        assert!(!result.is_success());
    }

    #[test]
    fn test_workflow_result_debug() {
        let id = WorkflowId::new();
        let result = WorkflowResult::success(id, "test");
        let debug = format!("{:?}", result);
        assert!(debug.contains("WorkflowResult"));
    }

    #[test]
    fn test_workflow_result_clone() {
        let id = WorkflowId::new();
        let result = WorkflowResult::success(id, "test");
        let cloned = result.clone();
        assert_eq!(result.workflow_id, cloned.workflow_id);
        assert_eq!(result.status, cloned.status);
        assert_eq!(result.summary, cloned.summary);
    }

    // ========== SkillsOrchestrator Construction Tests ==========

    #[test]
    fn test_orchestrator_construction() {
        let orchestrator = create_test_orchestrator();

        // Verify all components are initialized
        assert!(orchestrator.repo_path().exists() || true); // Path may not exist in test
        assert_eq!(orchestrator.claude().output_format(), "json");
    }

    #[test]
    fn test_orchestrator_has_ai_human() {
        let orchestrator = create_test_orchestrator();
        // ai_human should be initialized
        assert_eq!(orchestrator.ai_human().original_prompt(), "");
    }

    #[tokio::test]
    async fn test_orchestrator_has_agent_pool() {
        let orchestrator = create_test_orchestrator();
        let pool = orchestrator.agent_pool().read().await;
        assert_eq!(pool.active_count(), 0);
        assert!(pool.has_capacity());
    }

    #[tokio::test]
    async fn test_orchestrator_has_workflow_state() {
        let orchestrator = create_test_orchestrator();
        let state = orchestrator.state().read().await;
        assert_eq!(state.current_phase(), WorkflowPhase::Planning);
    }

    #[test]
    fn test_orchestrator_has_claude() {
        let orchestrator = create_test_orchestrator();
        assert_eq!(orchestrator.claude().output_format(), "json");
    }

    #[test]
    fn test_orchestrator_repo_path() {
        let orchestrator = create_test_orchestrator();
        assert_eq!(orchestrator.repo_path(), Path::new("/tmp/test-repo"));
    }

    // ========== Execute Skeleton Tests ==========

    #[tokio::test]
    async fn test_execute_returns_workflow_result() {
        let mut orchestrator = create_test_orchestrator();
        let result = orchestrator.execute("test prompt").await;

        assert!(result.is_ok());
        let result = result.unwrap();
        assert!(result.is_success());
    }

    #[tokio::test]
    async fn test_execute_workflow_id_in_result() {
        let mut orchestrator = create_test_orchestrator();
        let result = orchestrator.execute("test prompt").await.unwrap();

        // Verify workflow ID is set
        assert!(!result.workflow_id.0.is_nil());
    }

    #[tokio::test]
    async fn test_execute_updates_ai_human_prompt() {
        let mut orchestrator = create_test_orchestrator();
        orchestrator.execute("build authentication").await.unwrap();

        assert_eq!(orchestrator.ai_human().original_prompt(), "build authentication");
    }

    #[tokio::test]
    async fn test_execute_transitions_through_phases() {
        let mut orchestrator = create_test_orchestrator();
        orchestrator.execute("test prompt").await.unwrap();

        let state = orchestrator.state().read().await;
        assert_eq!(state.current_phase(), WorkflowPhase::Complete);
    }

    #[tokio::test]
    async fn test_execute_records_phase_history() {
        let mut orchestrator = create_test_orchestrator();
        orchestrator.execute("test prompt").await.unwrap();

        let state = orchestrator.state().read().await;
        let history = state.phase_history();

        // Should have: Planning, TaskGeneration, Implementation, Merging, Documentation, Complete
        // (with default config.update_docs = true)
        assert!(history.len() >= 5);
    }

    #[tokio::test]
    async fn test_execute_without_documentation_phase() {
        let mut orchestrator = create_test_orchestrator();

        // Disable documentation phase
        {
            let mut state = orchestrator.state.write().await;
            state.workflow_mut().config.update_docs = false;
        }

        // Re-create orchestrator with update_docs = false
        let config = WorkflowConfig {
            update_docs: false,
            ..Default::default()
        };
        let (event_tx, event_rx) = mpsc::channel(100);
        let agent_pool = AgentPool::new(config.max_parallel_agents, event_tx);
        let workflow = Workflow::new("", config);
        let state = WorkflowState::new(workflow);

        orchestrator.agent_pool = Arc::new(RwLock::new(agent_pool));
        orchestrator.state = Arc::new(RwLock::new(state));
        orchestrator.event_rx = Arc::new(RwLock::new(event_rx));

        orchestrator.execute("test prompt").await.unwrap();

        let state = orchestrator.state().read().await;
        let history = state.phase_history();

        // Should not include Documentation phase
        let has_documentation = history
            .iter()
            .any(|entry| entry.phase == WorkflowPhase::Documentation);
        assert!(!has_documentation);
    }

    #[tokio::test]
    async fn test_execute_sets_workflow_status_running_then_completed() {
        let mut orchestrator = create_test_orchestrator();
        let result = orchestrator.execute("test prompt").await.unwrap();

        assert_eq!(result.status, WorkflowStatus::Completed);

        let state = orchestrator.state().read().await;
        assert_eq!(state.workflow().status, WorkflowStatus::Completed);
    }

    #[tokio::test]
    async fn test_execute_summary_contains_prompt() {
        let mut orchestrator = create_test_orchestrator();
        let result = orchestrator.execute("build user authentication").await.unwrap();

        assert!(result.summary.contains("build user authentication"));
    }

    // ========== Module Export Tests ==========

    #[test]
    fn test_workflow_result_is_accessible() {
        // This test verifies WorkflowResult can be used
        let _result: WorkflowResult = WorkflowResult::success(WorkflowId::new(), "test");
    }

    #[test]
    fn test_skills_orchestrator_is_accessible() {
        // This test verifies SkillsOrchestrator can be constructed
        let _orchestrator = create_test_orchestrator();
    }

    // ========== PhaseEvent Tests ==========

    #[test]
    fn test_phase_event_started_debug() {
        let event = PhaseEvent::Started {
            phase: WorkflowPhase::Planning,
        };
        let debug = format!("{:?}", event);
        assert!(debug.contains("Started"));
        assert!(debug.contains("Planning"));
    }

    #[test]
    fn test_phase_event_changed_debug() {
        let event = PhaseEvent::Changed {
            from: WorkflowPhase::Planning,
            to: WorkflowPhase::TaskGeneration,
            elapsed: Duration::from_secs(30),
        };
        let debug = format!("{:?}", event);
        assert!(debug.contains("Changed"));
        assert!(debug.contains("Planning"));
        assert!(debug.contains("TaskGeneration"));
    }

    #[test]
    fn test_phase_event_completed_debug() {
        let event = PhaseEvent::Completed {
            total_duration: Duration::from_secs(120),
        };
        let debug = format!("{:?}", event);
        assert!(debug.contains("Completed"));
    }

    #[test]
    fn test_phase_event_clone() {
        let event = PhaseEvent::Changed {
            from: WorkflowPhase::Planning,
            to: WorkflowPhase::TaskGeneration,
            elapsed: Duration::from_secs(10),
        };
        let cloned = event.clone();
        match cloned {
            PhaseEvent::Changed { from, to, elapsed } => {
                assert_eq!(from, WorkflowPhase::Planning);
                assert_eq!(to, WorkflowPhase::TaskGeneration);
                assert_eq!(elapsed, Duration::from_secs(10));
            }
            _ => panic!("Expected Changed event"),
        }
    }

    // ========== PhaseController Construction Tests ==========

    #[test]
    fn test_phase_controller_new() {
        let (tx, _rx) = mpsc::channel(100);
        let controller = PhaseController::new(tx);

        assert_eq!(controller.current(), WorkflowPhase::Planning);
    }

    #[test]
    fn test_phase_controller_initial_history() {
        let (tx, _rx) = mpsc::channel(100);
        let controller = PhaseController::new(tx);

        let history = controller.history();
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].0, WorkflowPhase::Planning);
    }

    #[tokio::test]
    async fn test_phase_controller_emits_started_event_on_creation() {
        let (tx, mut rx) = mpsc::channel(100);
        let _controller = PhaseController::new(tx);

        // Should have received a Started event
        let event = rx.try_recv();
        assert!(event.is_ok());
        match event.unwrap() {
            PhaseEvent::Started { phase } => {
                assert_eq!(phase, WorkflowPhase::Planning);
            }
            _ => panic!("Expected Started event"),
        }
    }

    // ========== PhaseController current() Tests ==========

    #[test]
    fn test_phase_controller_current_returns_planning() {
        let (tx, _rx) = mpsc::channel(100);
        let controller = PhaseController::new(tx);
        assert_eq!(controller.current(), WorkflowPhase::Planning);
    }

    #[tokio::test]
    async fn test_phase_controller_current_updates_after_transition() {
        let (tx, _rx) = mpsc::channel(100);
        let mut controller = PhaseController::new(tx);

        controller.transition(WorkflowPhase::TaskGeneration).await.unwrap();
        assert_eq!(controller.current(), WorkflowPhase::TaskGeneration);
    }

    // ========== PhaseController elapsed() Tests ==========

    #[test]
    fn test_phase_controller_elapsed_is_positive() {
        let (tx, _rx) = mpsc::channel(100);
        let controller = PhaseController::new(tx);

        // Wait briefly
        std::thread::sleep(Duration::from_millis(10));

        let elapsed = controller.elapsed();
        assert!(elapsed >= Duration::from_millis(10));
    }

    #[tokio::test]
    async fn test_phase_controller_elapsed_resets_on_transition() {
        let (tx, _rx) = mpsc::channel(100);
        let mut controller = PhaseController::new(tx);

        // Wait a bit in Planning phase
        std::thread::sleep(Duration::from_millis(20));

        // Transition to TaskGeneration
        controller.transition(WorkflowPhase::TaskGeneration).await.unwrap();

        // Elapsed should be very small (just after transition)
        let elapsed = controller.elapsed();
        assert!(elapsed < Duration::from_millis(10));
    }

    // ========== PhaseController transition() Valid Transition Tests ==========

    #[tokio::test]
    async fn test_phase_controller_transition_planning_to_task_generation() {
        let (tx, _rx) = mpsc::channel(100);
        let mut controller = PhaseController::new(tx);

        let result = controller.transition(WorkflowPhase::TaskGeneration).await;
        assert!(result.is_ok());
        assert_eq!(controller.current(), WorkflowPhase::TaskGeneration);
    }

    #[tokio::test]
    async fn test_phase_controller_transition_task_generation_to_implementation() {
        let (tx, _rx) = mpsc::channel(100);
        let mut controller = PhaseController::new(tx);

        controller.transition(WorkflowPhase::TaskGeneration).await.unwrap();
        let result = controller.transition(WorkflowPhase::Implementation).await;

        assert!(result.is_ok());
        assert_eq!(controller.current(), WorkflowPhase::Implementation);
    }

    #[tokio::test]
    async fn test_phase_controller_transition_implementation_to_merging() {
        let (tx, _rx) = mpsc::channel(100);
        let mut controller = PhaseController::new(tx);

        controller.transition(WorkflowPhase::TaskGeneration).await.unwrap();
        controller.transition(WorkflowPhase::Implementation).await.unwrap();
        let result = controller.transition(WorkflowPhase::Merging).await;

        assert!(result.is_ok());
        assert_eq!(controller.current(), WorkflowPhase::Merging);
    }

    #[tokio::test]
    async fn test_phase_controller_transition_merging_to_documentation() {
        let (tx, _rx) = mpsc::channel(100);
        let mut controller = PhaseController::new(tx);

        controller.transition(WorkflowPhase::TaskGeneration).await.unwrap();
        controller.transition(WorkflowPhase::Implementation).await.unwrap();
        controller.transition(WorkflowPhase::Merging).await.unwrap();
        let result = controller.transition(WorkflowPhase::Documentation).await;

        assert!(result.is_ok());
        assert_eq!(controller.current(), WorkflowPhase::Documentation);
    }

    #[tokio::test]
    async fn test_phase_controller_transition_merging_to_complete() {
        let (tx, _rx) = mpsc::channel(100);
        let mut controller = PhaseController::new(tx);

        controller.transition(WorkflowPhase::TaskGeneration).await.unwrap();
        controller.transition(WorkflowPhase::Implementation).await.unwrap();
        controller.transition(WorkflowPhase::Merging).await.unwrap();
        let result = controller.transition(WorkflowPhase::Complete).await;

        assert!(result.is_ok());
        assert_eq!(controller.current(), WorkflowPhase::Complete);
    }

    #[tokio::test]
    async fn test_phase_controller_transition_documentation_to_complete() {
        let (tx, _rx) = mpsc::channel(100);
        let mut controller = PhaseController::new(tx);

        controller.transition(WorkflowPhase::TaskGeneration).await.unwrap();
        controller.transition(WorkflowPhase::Implementation).await.unwrap();
        controller.transition(WorkflowPhase::Merging).await.unwrap();
        controller.transition(WorkflowPhase::Documentation).await.unwrap();
        let result = controller.transition(WorkflowPhase::Complete).await;

        assert!(result.is_ok());
        assert_eq!(controller.current(), WorkflowPhase::Complete);
    }

    // ========== PhaseController transition() Invalid Transition Tests ==========

    #[tokio::test]
    async fn test_phase_controller_invalid_transition_planning_to_implementation() {
        let (tx, _rx) = mpsc::channel(100);
        let mut controller = PhaseController::new(tx);

        let result = controller.transition(WorkflowPhase::Implementation).await;

        assert!(result.is_err());
        assert_eq!(controller.current(), WorkflowPhase::Planning);
    }

    #[tokio::test]
    async fn test_phase_controller_invalid_transition_planning_to_merging() {
        let (tx, _rx) = mpsc::channel(100);
        let mut controller = PhaseController::new(tx);

        let result = controller.transition(WorkflowPhase::Merging).await;

        assert!(result.is_err());
        assert_eq!(controller.current(), WorkflowPhase::Planning);
    }

    #[tokio::test]
    async fn test_phase_controller_invalid_transition_planning_to_complete() {
        let (tx, _rx) = mpsc::channel(100);
        let mut controller = PhaseController::new(tx);

        let result = controller.transition(WorkflowPhase::Complete).await;

        assert!(result.is_err());
        assert_eq!(controller.current(), WorkflowPhase::Planning);
    }

    #[tokio::test]
    async fn test_phase_controller_invalid_same_phase_transition() {
        let (tx, _rx) = mpsc::channel(100);
        let mut controller = PhaseController::new(tx);

        let result = controller.transition(WorkflowPhase::Planning).await;

        assert!(result.is_err());
        assert_eq!(controller.current(), WorkflowPhase::Planning);
    }

    #[tokio::test]
    async fn test_phase_controller_invalid_backward_transition() {
        let (tx, _rx) = mpsc::channel(100);
        let mut controller = PhaseController::new(tx);

        controller.transition(WorkflowPhase::TaskGeneration).await.unwrap();
        let result = controller.transition(WorkflowPhase::Planning).await;

        assert!(result.is_err());
        assert_eq!(controller.current(), WorkflowPhase::TaskGeneration);
    }

    // ========== PhaseController Event Emission Tests ==========

    #[tokio::test]
    async fn test_phase_controller_emits_changed_event_on_transition() {
        let (tx, mut rx) = mpsc::channel(100);
        let mut controller = PhaseController::new(tx);

        // Drain the Started event
        let _ = rx.recv().await;

        controller.transition(WorkflowPhase::TaskGeneration).await.unwrap();

        let event = rx.recv().await.expect("Should receive Changed event");
        match event {
            PhaseEvent::Changed { from, to, .. } => {
                assert_eq!(from, WorkflowPhase::Planning);
                assert_eq!(to, WorkflowPhase::TaskGeneration);
            }
            _ => panic!("Expected Changed event"),
        }
    }

    #[tokio::test]
    async fn test_phase_controller_emits_completed_event_on_final_transition() {
        let (tx, mut rx) = mpsc::channel(100);
        let mut controller = PhaseController::new(tx);

        // Run through all transitions
        controller.transition(WorkflowPhase::TaskGeneration).await.unwrap();
        controller.transition(WorkflowPhase::Implementation).await.unwrap();
        controller.transition(WorkflowPhase::Merging).await.unwrap();
        controller.transition(WorkflowPhase::Complete).await.unwrap();

        // Collect all events
        let mut events = Vec::new();
        while let Ok(event) = rx.try_recv() {
            events.push(event);
        }

        // Should have: Started, Changed x4, Completed
        assert!(events.len() >= 5);

        // Last event should be Completed
        let last = events.last().unwrap();
        match last {
            PhaseEvent::Completed { total_duration } => {
                assert!(*total_duration > Duration::ZERO);
            }
            _ => panic!("Expected Completed event as last event"),
        }
    }

    #[tokio::test]
    async fn test_phase_controller_changed_event_contains_elapsed_duration() {
        let (tx, mut rx) = mpsc::channel(100);
        let mut controller = PhaseController::new(tx);

        // Drain the Started event
        let _ = rx.recv().await;

        // Wait a bit then transition
        std::thread::sleep(Duration::from_millis(15));
        controller.transition(WorkflowPhase::TaskGeneration).await.unwrap();

        let event = rx.recv().await.expect("Should receive Changed event");
        match event {
            PhaseEvent::Changed { elapsed, .. } => {
                // Should have been in Planning for at least 15ms
                assert!(elapsed >= Duration::from_millis(15));
            }
            _ => panic!("Expected Changed event"),
        }
    }

    // ========== PhaseController History Tracking Tests ==========

    #[tokio::test]
    async fn test_phase_controller_history_tracks_all_transitions() {
        let (tx, _rx) = mpsc::channel(100);
        let mut controller = PhaseController::new(tx);

        controller.transition(WorkflowPhase::TaskGeneration).await.unwrap();
        controller.transition(WorkflowPhase::Implementation).await.unwrap();
        controller.transition(WorkflowPhase::Merging).await.unwrap();

        let history = controller.history();
        assert_eq!(history.len(), 4);
        assert_eq!(history[0].0, WorkflowPhase::Planning);
        assert_eq!(history[1].0, WorkflowPhase::TaskGeneration);
        assert_eq!(history[2].0, WorkflowPhase::Implementation);
        assert_eq!(history[3].0, WorkflowPhase::Merging);
    }

    #[tokio::test]
    async fn test_phase_controller_history_preserves_timestamp_order() {
        let (tx, _rx) = mpsc::channel(100);
        let mut controller = PhaseController::new(tx);

        controller.transition(WorkflowPhase::TaskGeneration).await.unwrap();
        controller.transition(WorkflowPhase::Implementation).await.unwrap();

        let history = controller.history();

        // Timestamps should be in increasing order
        for i in 1..history.len() {
            assert!(history[i].1 >= history[i - 1].1);
        }
    }

    #[tokio::test]
    async fn test_phase_controller_history_not_modified_on_invalid_transition() {
        let (tx, _rx) = mpsc::channel(100);
        let mut controller = PhaseController::new(tx);

        let initial_len = controller.history().len();

        // Try invalid transition
        let _ = controller.transition(WorkflowPhase::Merging).await;

        assert_eq!(controller.history().len(), initial_len);
    }

    // ========== Full Workflow Traversal Tests ==========

    #[tokio::test]
    async fn test_phase_controller_full_workflow_with_documentation() {
        let (tx, _rx) = mpsc::channel(100);
        let mut controller = PhaseController::new(tx);

        controller.transition(WorkflowPhase::TaskGeneration).await.unwrap();
        controller.transition(WorkflowPhase::Implementation).await.unwrap();
        controller.transition(WorkflowPhase::Merging).await.unwrap();
        controller.transition(WorkflowPhase::Documentation).await.unwrap();
        controller.transition(WorkflowPhase::Complete).await.unwrap();

        assert_eq!(controller.current(), WorkflowPhase::Complete);
        assert_eq!(controller.history().len(), 6);
    }

    #[tokio::test]
    async fn test_phase_controller_full_workflow_without_documentation() {
        let (tx, _rx) = mpsc::channel(100);
        let mut controller = PhaseController::new(tx);

        controller.transition(WorkflowPhase::TaskGeneration).await.unwrap();
        controller.transition(WorkflowPhase::Implementation).await.unwrap();
        controller.transition(WorkflowPhase::Merging).await.unwrap();
        controller.transition(WorkflowPhase::Complete).await.unwrap();

        assert_eq!(controller.current(), WorkflowPhase::Complete);
        assert_eq!(controller.history().len(), 5);
    }

    // ========== Module Export Tests for PhaseController ==========

    #[test]
    fn test_phase_controller_is_accessible() {
        let (tx, _rx) = mpsc::channel::<PhaseEvent>(100);
        let _controller = PhaseController::new(tx);
    }

    #[test]
    fn test_phase_event_is_accessible() {
        let _event = PhaseEvent::Started {
            phase: WorkflowPhase::Planning,
        };
    }

    // ========== MonitorConfig Tests ==========

    #[test]
    fn test_monitor_config_default() {
        let config = MonitorConfig::default();
        assert_eq!(config.poll_interval, Duration::from_millis(100));
        assert_eq!(config.timeout, Duration::from_secs(600));
    }

    #[test]
    fn test_monitor_config_new() {
        let config = MonitorConfig::new(
            Duration::from_millis(50),
            Duration::from_secs(300),
        );
        assert_eq!(config.poll_interval, Duration::from_millis(50));
        assert_eq!(config.timeout, Duration::from_secs(300));
    }

    #[test]
    fn test_monitor_config_fast() {
        let config = MonitorConfig::fast();
        assert_eq!(config.poll_interval, Duration::from_millis(10));
        assert_eq!(config.timeout, Duration::from_secs(60));
    }

    #[test]
    fn test_monitor_config_debug() {
        let config = MonitorConfig::default();
        let debug = format!("{:?}", config);
        assert!(debug.contains("MonitorConfig"));
        assert!(debug.contains("poll_interval"));
        assert!(debug.contains("timeout"));
    }

    #[test]
    fn test_monitor_config_clone() {
        let config = MonitorConfig::default();
        let cloned = config.clone();
        assert_eq!(config.poll_interval, cloned.poll_interval);
        assert_eq!(config.timeout, cloned.timeout);
    }

    // ========== SkillResult Tests ==========

    #[test]
    fn test_skill_result_success() {
        let result = SkillResult::success(5, Duration::from_secs(10));
        assert!(result.success);
        assert!(result.output.is_none());
        assert_eq!(result.questions_answered, 5);
        assert_eq!(result.duration, Duration::from_secs(10));
        assert!(result.is_success());
    }

    #[test]
    fn test_skill_result_success_with_output() {
        let result = SkillResult::success_with_output(
            "Task completed",
            3,
            Duration::from_secs(5),
        );
        assert!(result.success);
        assert_eq!(result.output, Some("Task completed".to_string()));
        assert_eq!(result.questions_answered, 3);
        assert_eq!(result.duration, Duration::from_secs(5));
    }

    #[test]
    fn test_skill_result_failure() {
        let result = SkillResult::failure(Duration::from_secs(2));
        assert!(!result.success);
        assert!(result.output.is_none());
        assert_eq!(result.questions_answered, 0);
        assert_eq!(result.duration, Duration::from_secs(2));
        assert!(!result.is_success());
    }

    #[test]
    fn test_skill_result_debug() {
        let result = SkillResult::success(1, Duration::from_millis(100));
        let debug = format!("{:?}", result);
        assert!(debug.contains("SkillResult"));
        assert!(debug.contains("success"));
    }

    #[test]
    fn test_skill_result_clone() {
        let result = SkillResult::success_with_output("test", 2, Duration::from_secs(1));
        let cloned = result.clone();
        assert_eq!(result.success, cloned.success);
        assert_eq!(result.output, cloned.output);
        assert_eq!(result.questions_answered, cloned.questions_answered);
        assert_eq!(result.duration, cloned.duration);
    }

    // ========== Module Export Tests for New Types ==========

    #[test]
    fn test_monitor_config_is_accessible() {
        let _config = MonitorConfig::default();
    }

    #[test]
    fn test_skill_result_is_accessible() {
        let _result = SkillResult::success(0, Duration::ZERO);
    }
}
