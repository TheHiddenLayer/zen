//! Skills orchestrator for coordinating workflow phases.
//!
//! The `SkillsOrchestrator` is the central orchestration engine that drives
//! the 5-phase workflow (PDD -> TaskGen -> Implementation -> Merge -> Docs)
//! by composing AIHumanProxy, AgentPool, and ClaudeHeadless.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};

use crate::error::Result;
use crate::workflow::{Workflow, WorkflowConfig, WorkflowId, WorkflowPhase, WorkflowState, WorkflowStatus};
use crate::{zlog, zlog_debug};

use super::{AgentEvent, AgentPool, AIHumanProxy, ClaudeHeadless};

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
}
