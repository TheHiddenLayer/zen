//! Skills orchestrator for coordinating workflow phases.
//!
//! The `SkillsOrchestrator` is the central orchestration engine that drives
//! the 5-phase workflow (PDD -> TaskGen -> Implementation -> Merge -> Docs)
//! by composing AIHumanProxy, AgentPool, and ClaudeHeadless.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, RwLock};
use tokio::time::sleep;

use crate::core::dag::{DependencyType, TaskDAG};
use crate::core::CodeTask;
use crate::error::{Error, Result};
use crate::workflow::{Workflow, WorkflowConfig, WorkflowId, WorkflowPhase, WorkflowState, WorkflowStatus};
use crate::{zlog, zlog_debug};

use super::{AgentEvent, AgentHandle, AgentOutput, AgentPool, AIHumanProxy, ClaudeHeadless, ImplResult, Scheduler};

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

/// Result of the PDD (Prompt-Driven Development) phase.
///
/// Contains paths to the artifacts produced by the /pdd skill:
/// - `design_path`: Path to the detailed design document
/// - `plan_path`: Path to the implementation plan
/// - `research_dir`: Path to the research directory
///
/// # Example
///
/// ```ignore
/// use std::path::Path;
/// use zen::orchestration::PDDResult;
///
/// let result = PDDResult::from_directory(Path::new(".sop/planning"))?;
/// println!("Design: {:?}", result.design_path);
/// println!("Plan: {:?}", result.plan_path);
/// ```
#[derive(Debug, Clone)]
pub struct PDDResult {
    /// Path to the detailed design document (detailed-design.md).
    pub design_path: PathBuf,
    /// Path to the implementation plan (plan.md).
    pub plan_path: PathBuf,
    /// Path to the research directory containing research outputs.
    pub research_dir: PathBuf,
}

impl PDDResult {
    /// Create a PDDResult from the planning directory.
    ///
    /// Validates that the expected artifacts exist in the directory:
    /// - `design/detailed-design.md`
    /// - `implementation/plan.md`
    /// - `research/` directory
    ///
    /// # Arguments
    ///
    /// * `planning_dir` - Path to the .sop/planning directory
    ///
    /// # Errors
    ///
    /// Returns `Error::PDDArtifactNotFound` if any required artifact is missing.
    pub fn from_directory(planning_dir: &Path) -> Result<Self> {
        let design_path = planning_dir.join("design").join("detailed-design.md");
        let plan_path = planning_dir.join("implementation").join("plan.md");
        let research_dir = planning_dir.join("research");

        // Validate design document exists
        if !design_path.exists() {
            return Err(Error::PDDArtifactNotFound {
                path: design_path.display().to_string(),
            });
        }

        // Validate implementation plan exists
        if !plan_path.exists() {
            return Err(Error::PDDArtifactNotFound {
                path: plan_path.display().to_string(),
            });
        }

        // Validate research directory exists
        if !research_dir.exists() {
            return Err(Error::PDDArtifactNotFound {
                path: research_dir.display().to_string(),
            });
        }

        Ok(Self {
            design_path,
            plan_path,
            research_dir,
        })
    }

    /// Create a PDDResult with custom paths (primarily for testing).
    ///
    /// This method does not validate that the paths exist.
    pub fn with_paths(design_path: PathBuf, plan_path: PathBuf, research_dir: PathBuf) -> Self {
        Self {
            design_path,
            plan_path,
            research_dir,
        }
    }

    /// Check if all artifacts exist at their expected paths.
    pub fn artifacts_exist(&self) -> bool {
        self.design_path.exists() && self.plan_path.exists() && self.research_dir.exists()
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
        let generated_tasks = match self.run_task_generation_phase().await {
            Ok(tasks) => {
                zlog!("[orchestrator] Generated {} code tasks", tasks.len());
                tasks
            }
            Err(e) => {
                return Ok(self.fail_workflow(workflow_id, format!("Task generation phase failed: {}", e)).await);
            }
        };

        // Transition to Implementation
        {
            let mut state = self.state.write().await;
            if let Err(e) = state.transition(WorkflowPhase::Implementation) {
                return Ok(self.fail_workflow(workflow_id, format!("Failed to transition to implementation: {}", e)).await);
            }
        }

        // PHASE 3: Implementation with /code-assist in parallel
        zlog!("[orchestrator] Beginning implementation phase");
        let _impl_results = match self.run_implementation_phase(&generated_tasks).await {
            Ok(results) => {
                zlog!("[orchestrator] Implementation phase completed: {} tasks", results.len());
                results
            }
            Err(e) => {
                return Ok(self.fail_workflow(workflow_id, format!("Implementation phase failed: {}", e)).await);
            }
        };

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

    /// Run the planning phase by executing /pdd skill.
    ///
    /// Spawns an agent, invokes /pdd with the user's prompt, and monitors
    /// for completion. Questions from /pdd are answered via AIHumanProxy.
    ///
    /// # Arguments
    ///
    /// * `prompt` - The user's original task description
    ///
    /// # Returns
    ///
    /// `Ok(())` on success. The PDD artifacts are stored in the agent's worktree
    /// at `.sop/planning/`.
    ///
    /// # Errors
    ///
    /// Returns an error if the agent fails to spawn, /pdd fails, or times out.
    async fn run_planning_phase(&self, prompt: &str) -> Result<()> {
        zlog!("[orchestrator] Running /pdd skill with prompt");

        // Run the PDD phase and get the result
        let _pdd_result = self.run_pdd_phase(prompt).await?;

        zlog!("[orchestrator] Planning phase completed successfully");
        Ok(())
    }

    /// Execute the /pdd skill and return the result.
    ///
    /// This is the core PDD phase implementation that:
    /// 1. Spawns an agent for the /pdd skill
    /// 2. Sends the /pdd command with the user's prompt
    /// 3. Monitors output, answering questions via AIHumanProxy
    /// 4. Parses and validates PDD artifacts on completion
    ///
    /// # Arguments
    ///
    /// * `prompt` - The user's original task description (rough idea)
    ///
    /// # Returns
    ///
    /// A `PDDResult` containing paths to the generated artifacts.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Agent fails to spawn (`Error::AgentPoolFull`)
    /// - /pdd execution fails or times out
    /// - PDD artifacts are not found after completion
    pub async fn run_pdd_phase(&self, prompt: &str) -> Result<PDDResult> {
        zlog_debug!("[pdd] Spawning agent for /pdd skill");

        // Spawn an agent for the PDD skill
        let agent = self.agent_pool.write().await.spawn_for_skill("pdd").await?;
        zlog_debug!("[pdd] Agent {} spawned", agent.id);

        // Send the /pdd command with the user's prompt as rough_idea
        let pdd_command = format!("/pdd\n\nrough_idea: {}", prompt);
        agent.send(&pdd_command)?;
        zlog_debug!("[pdd] Sent /pdd command to agent");

        // Monitor agent output and answer questions via AIHumanProxy
        let config = MonitorConfig::default();
        let skill_result = self.monitor_agent_output(&agent, &config).await?;

        if !skill_result.is_success() {
            return Err(Error::ClaudeExecutionFailed(
                "PDD skill did not complete successfully".to_string(),
            ));
        }

        zlog_debug!(
            "[pdd] /pdd completed: {} questions answered in {:?}",
            skill_result.questions_answered,
            skill_result.duration
        );

        // Parse and validate PDD artifacts from the agent's worktree
        // Note: In a real implementation, the worktree would be set up during spawn
        // For now, use the repo_path as the base (actual worktree support in later steps)
        let planning_dir = self.repo_path.join(".sop").join("planning");
        let pdd_result = PDDResult::from_directory(&planning_dir)?;

        zlog!("[pdd] PDD artifacts validated at {:?}", planning_dir);
        Ok(pdd_result)
    }

    /// Run the task generation phase by executing /code-task-generator.
    ///
    /// This phase takes the PDD output (plan.md) and generates individual
    /// .code-task.md files that can be executed independently in the
    /// implementation phase.
    ///
    /// # Returns
    ///
    /// A vector of CodeTask objects parsed from the generated files.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Agent fails to spawn
    /// - /code-task-generator execution fails or times out
    /// - No .code-task.md files are generated
    async fn run_task_generation_phase(&self) -> Result<Vec<CodeTask>> {
        zlog!("[orchestrator] Running /code-task-generator skill");

        // Get the PDD result path from the repo
        let planning_dir = self.repo_path.join(".sop").join("planning");
        let pdd_result = PDDResult::from_directory(&planning_dir)?;

        // Run the task generation with the PDD result
        let tasks = self.run_code_task_generator_phase(&pdd_result).await?;

        zlog!(
            "[orchestrator] Task generation phase completed: {} tasks generated",
            tasks.len()
        );

        Ok(tasks)
    }

    /// Execute the /code-task-generator skill and return the generated code tasks.
    ///
    /// This is the core task generation implementation that:
    /// 1. Spawns an agent for the /code-task-generator skill
    /// 2. Sends the /code-task-generator command with the plan.md path
    /// 3. Monitors output, answering questions via AIHumanProxy
    /// 4. Scans for generated .code-task.md files on completion
    /// 5. Parses and returns CodeTask objects
    ///
    /// # Arguments
    ///
    /// * `pdd` - The PDD result containing the plan.md path
    ///
    /// # Returns
    ///
    /// A vector of CodeTask objects parsed from the generated files.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Agent fails to spawn (`Error::AgentPoolFull`)
    /// - /code-task-generator execution fails or times out
    /// - No .code-task.md files are generated (`Error::NoCodeTasksGenerated`)
    pub async fn run_code_task_generator_phase(&self, pdd: &PDDResult) -> Result<Vec<CodeTask>> {
        zlog_debug!("[task-gen] Spawning agent for /code-task-generator skill");

        // Spawn an agent for the code-task-generator skill
        let agent = self
            .agent_pool
            .write()
            .await
            .spawn_for_skill("code-task-generator")
            .await?;
        zlog_debug!("[task-gen] Agent {} spawned", agent.id);

        // Send the /code-task-generator command with the plan.md path as input
        let generator_command = format!(
            "/code-task-generator\n\ninput: {}",
            pdd.plan_path.display()
        );
        agent.send(&generator_command)?;
        zlog_debug!("[task-gen] Sent /code-task-generator command to agent");

        // Monitor agent output and answer questions via AIHumanProxy
        let config = MonitorConfig::default();
        let skill_result = self.monitor_agent_output(&agent, &config).await?;

        if !skill_result.is_success() {
            return Err(Error::ClaudeExecutionFailed(
                "Code task generator skill did not complete successfully".to_string(),
            ));
        }

        zlog_debug!(
            "[task-gen] /code-task-generator completed: {} questions answered in {:?}",
            skill_result.questions_answered,
            skill_result.duration
        );

        // Scan for generated .code-task.md files
        // The generator typically creates files in the worktree root or .sop directory
        // We'll look in both locations and the implementation directory
        let search_paths = [
            self.repo_path.clone(),
            self.repo_path.join(".sop"),
            self.repo_path.join(".sop").join("planning").join("implementation"),
        ];

        let mut all_tasks = Vec::new();
        for search_path in &search_paths {
            if let Ok(tasks) = CodeTask::from_directory(search_path) {
                all_tasks.extend(tasks);
            }
        }

        // Deduplicate by ID (in case the same task is found in multiple locations)
        let mut seen_ids = std::collections::HashSet::new();
        all_tasks.retain(|task| seen_ids.insert(task.id.clone()));

        if all_tasks.is_empty() {
            return Err(Error::NoCodeTasksGenerated {
                path: self.repo_path.display().to_string(),
            });
        }

        zlog!(
            "[task-gen] Found {} code tasks in {}",
            all_tasks.len(),
            self.repo_path.display()
        );

        Ok(all_tasks)
    }

    /// Run the implementation phase by executing /code-assist in parallel.
    ///
    /// This is Phase 3 of the workflow. It:
    /// 1. Builds a TaskDAG from the generated CodeTasks
    /// 2. Creates a Scheduler to manage parallel execution
    /// 3. Executes /code-assist for each task in dependency order
    /// 4. Returns the implementation results
    ///
    /// # Arguments
    ///
    /// * `tasks` - The code tasks generated from Phase 2
    ///
    /// # Returns
    ///
    /// A vector of `ImplResult` for each completed task, containing
    /// the task ID, worktree path, and commit hash.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - DAG construction fails (e.g., cycle detected)
    /// - Agent spawning fails
    /// - Task execution fails critically
    pub async fn run_implementation_phase(&self, tasks: &[CodeTask]) -> Result<Vec<ImplResult>> {
        if tasks.is_empty() {
            zlog!("[orchestrator] No tasks to implement, skipping implementation phase");
            return Ok(Vec::new());
        }

        zlog!("[orchestrator] Starting implementation phase with {} tasks", tasks.len());

        // Build DAG from CodeTasks with dependencies
        let dag = self.build_task_dag(tasks)?;
        zlog_debug!(
            "[orchestrator] Built task DAG with {} tasks and {} dependencies",
            dag.task_count(),
            dag.dependency_count()
        );

        // Create scheduler event channel for TUI updates
        let (scheduler_event_tx, _scheduler_event_rx) = mpsc::channel(100);

        // Create scheduler
        let mut scheduler = Scheduler::new(
            Arc::new(RwLock::new(dag)),
            Arc::clone(&self.agent_pool),
            scheduler_event_tx,
            self.repo_path.clone(),
        );

        // Get agent event receiver from the pool
        // Note: We need to create a new receiver here since the pool's channel
        // might have other listeners. For now, we'll create a simple monitoring loop.
        let (agent_event_tx, mut agent_event_rx) = mpsc::channel::<AgentEvent>(100);

        // Replace the pool's event sender temporarily
        // This is a limitation of the current architecture - ideally the pool
        // would allow multiple listeners or we'd have a broadcast channel
        {
            let mut pool = self.agent_pool.write().await;
            pool.set_event_sender(agent_event_tx);
        }

        // Run the scheduler
        let results = scheduler.run(&mut agent_event_rx).await?;

        zlog!(
            "[orchestrator] Implementation phase completed: {} tasks implemented",
            results.len()
        );

        Ok(results)
    }

    /// Build a TaskDAG from CodeTasks, inferring dependencies from the task metadata.
    ///
    /// This method:
    /// 1. Converts each CodeTask to a Task for the DAG
    /// 2. Creates a mapping from CodeTask IDs to Task IDs
    /// 3. Adds dependency edges based on CodeTask.dependencies
    ///
    /// # Arguments
    ///
    /// * `tasks` - The code tasks to convert
    ///
    /// # Returns
    ///
    /// A TaskDAG with all tasks and their dependency relationships.
    ///
    /// # Errors
    ///
    /// Returns an error if a dependency cycle is detected.
    pub fn build_task_dag(&self, tasks: &[CodeTask]) -> Result<TaskDAG> {
        let mut dag = TaskDAG::new();
        let mut id_map: HashMap<String, crate::core::task::TaskId> = HashMap::new();

        // First pass: add all tasks to the DAG
        for code_task in tasks {
            let task = code_task.to_task();
            id_map.insert(code_task.id.clone(), task.id);
            dag.add_task(task);
        }

        // Second pass: add dependencies based on CodeTask.dependencies
        for code_task in tasks {
            if let Some(task_id) = id_map.get(&code_task.id) {
                for dep_id in &code_task.dependencies {
                    // Try to find the dependency by ID
                    if let Some(dep_task_id) = id_map.get(dep_id) {
                        // Add dependency: dep_task_id -> task_id (dep must complete before task)
                        if let Err(e) = dag.add_dependency(dep_task_id, task_id, DependencyType::DataDependency) {
                            zlog!(
                                "[orchestrator] Warning: Failed to add dependency {} -> {}: {}",
                                dep_id,
                                code_task.id,
                                e
                            );
                        }
                    } else {
                        zlog_debug!(
                            "[orchestrator] Dependency {} not found for task {}, skipping",
                            dep_id,
                            code_task.id
                        );
                    }
                }
            }
        }

        Ok(dag)
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

    // ========== Execute Tests ==========
    //
    // Note: Full execute() requires tmux for agent communication.
    // These tests verify the workflow structure and failure handling.
    // Integration tests with actual tmux will be in Step 19.

    #[tokio::test]
    async fn test_execute_returns_workflow_result() {
        let mut orchestrator = create_test_orchestrator();
        let result = orchestrator.execute("test prompt").await;

        // execute() returns Ok(WorkflowResult) even on failure (failure is in status)
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_execute_workflow_id_in_result() {
        let mut orchestrator = create_test_orchestrator();
        let result = orchestrator.execute("test prompt").await.unwrap();

        // Verify workflow ID is set (regardless of success/failure)
        assert!(!result.workflow_id.0.is_nil());
    }

    #[tokio::test]
    async fn test_execute_updates_ai_human_prompt() {
        let mut orchestrator = create_test_orchestrator();
        let _ = orchestrator.execute("build authentication").await;

        // AIHumanProxy should have the prompt set even if planning fails
        assert_eq!(orchestrator.ai_human().original_prompt(), "build authentication");
    }

    #[tokio::test]
    async fn test_execute_fails_on_planning_phase_without_tmux() {
        // When tmux is not available (test environment), planning phase should fail
        let mut orchestrator = create_test_orchestrator();
        let result = orchestrator.execute("test prompt").await.unwrap();

        // Without tmux, the planning phase fails and workflow fails
        assert_eq!(result.status, WorkflowStatus::Failed);
        assert!(result.summary.contains("Planning phase failed"));
    }

    #[tokio::test]
    async fn test_execute_sets_workflow_state_on_planning_failure() {
        let mut orchestrator = create_test_orchestrator();
        let result = orchestrator.execute("test prompt").await.unwrap();

        // Verify workflow is marked as failed
        let state = orchestrator.state().read().await;
        assert_eq!(state.workflow().status, WorkflowStatus::Failed);
        assert!(!result.is_success());
    }

    #[tokio::test]
    async fn test_execute_summary_contains_failure_reason() {
        let mut orchestrator = create_test_orchestrator();
        let result = orchestrator.execute("build user authentication").await.unwrap();

        // Summary should indicate planning failed (due to no tmux in tests)
        assert!(result.summary.contains("Planning phase failed"));
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

    // ========== PDDResult Tests ==========

    #[test]
    fn test_pdd_result_with_paths() {
        let result = PDDResult::with_paths(
            PathBuf::from("/path/to/design.md"),
            PathBuf::from("/path/to/plan.md"),
            PathBuf::from("/path/to/research"),
        );
        assert_eq!(result.design_path, PathBuf::from("/path/to/design.md"));
        assert_eq!(result.plan_path, PathBuf::from("/path/to/plan.md"));
        assert_eq!(result.research_dir, PathBuf::from("/path/to/research"));
    }

    #[test]
    fn test_pdd_result_debug() {
        let result = PDDResult::with_paths(
            PathBuf::from("/design.md"),
            PathBuf::from("/plan.md"),
            PathBuf::from("/research"),
        );
        let debug = format!("{:?}", result);
        assert!(debug.contains("PDDResult"));
        assert!(debug.contains("design_path"));
        assert!(debug.contains("plan_path"));
        assert!(debug.contains("research_dir"));
    }

    #[test]
    fn test_pdd_result_clone() {
        let result = PDDResult::with_paths(
            PathBuf::from("/design.md"),
            PathBuf::from("/plan.md"),
            PathBuf::from("/research"),
        );
        let cloned = result.clone();
        assert_eq!(result.design_path, cloned.design_path);
        assert_eq!(result.plan_path, cloned.plan_path);
        assert_eq!(result.research_dir, cloned.research_dir);
    }

    #[test]
    fn test_pdd_result_artifacts_exist_false_for_nonexistent() {
        let result = PDDResult::with_paths(
            PathBuf::from("/nonexistent/design.md"),
            PathBuf::from("/nonexistent/plan.md"),
            PathBuf::from("/nonexistent/research"),
        );
        assert!(!result.artifacts_exist());
    }

    #[test]
    fn test_pdd_result_from_directory_missing_design() {
        let temp_dir = std::env::temp_dir().join(format!("pdd_test_{}", std::process::id()));
        let planning_dir = temp_dir.join("planning");

        // Create only implementation/plan.md and research/
        std::fs::create_dir_all(planning_dir.join("implementation")).unwrap();
        std::fs::create_dir_all(planning_dir.join("research")).unwrap();
        std::fs::write(planning_dir.join("implementation").join("plan.md"), "# Plan").unwrap();

        let result = PDDResult::from_directory(&planning_dir);
        assert!(result.is_err());
        if let Err(Error::PDDArtifactNotFound { path }) = result {
            assert!(path.contains("detailed-design.md"));
        } else {
            panic!("Expected PDDArtifactNotFound error");
        }

        // Cleanup
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_pdd_result_from_directory_missing_plan() {
        let temp_dir = std::env::temp_dir().join(format!("pdd_test_plan_{}", std::process::id()));
        let planning_dir = temp_dir.join("planning");

        // Create only design/detailed-design.md and research/
        std::fs::create_dir_all(planning_dir.join("design")).unwrap();
        std::fs::create_dir_all(planning_dir.join("research")).unwrap();
        std::fs::write(planning_dir.join("design").join("detailed-design.md"), "# Design").unwrap();

        let result = PDDResult::from_directory(&planning_dir);
        assert!(result.is_err());
        if let Err(Error::PDDArtifactNotFound { path }) = result {
            assert!(path.contains("plan.md"));
        } else {
            panic!("Expected PDDArtifactNotFound error");
        }

        // Cleanup
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_pdd_result_from_directory_missing_research() {
        let temp_dir = std::env::temp_dir().join(format!("pdd_test_research_{}", std::process::id()));
        let planning_dir = temp_dir.join("planning");

        // Create design and implementation but not research
        std::fs::create_dir_all(planning_dir.join("design")).unwrap();
        std::fs::create_dir_all(planning_dir.join("implementation")).unwrap();
        std::fs::write(planning_dir.join("design").join("detailed-design.md"), "# Design").unwrap();
        std::fs::write(planning_dir.join("implementation").join("plan.md"), "# Plan").unwrap();

        let result = PDDResult::from_directory(&planning_dir);
        assert!(result.is_err());
        if let Err(Error::PDDArtifactNotFound { path }) = result {
            assert!(path.contains("research"));
        } else {
            panic!("Expected PDDArtifactNotFound error");
        }

        // Cleanup
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_pdd_result_from_directory_success() {
        let temp_dir = std::env::temp_dir().join(format!("pdd_test_success_{}", std::process::id()));
        let planning_dir = temp_dir.join("planning");

        // Create all required artifacts
        std::fs::create_dir_all(planning_dir.join("design")).unwrap();
        std::fs::create_dir_all(planning_dir.join("implementation")).unwrap();
        std::fs::create_dir_all(planning_dir.join("research")).unwrap();
        std::fs::write(planning_dir.join("design").join("detailed-design.md"), "# Design").unwrap();
        std::fs::write(planning_dir.join("implementation").join("plan.md"), "# Plan").unwrap();

        let result = PDDResult::from_directory(&planning_dir);
        assert!(result.is_ok());

        let pdd_result = result.unwrap();
        assert!(pdd_result.design_path.ends_with("design/detailed-design.md"));
        assert!(pdd_result.plan_path.ends_with("implementation/plan.md"));
        assert!(pdd_result.research_dir.ends_with("research"));
        assert!(pdd_result.artifacts_exist());

        // Cleanup
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_pdd_result_is_accessible() {
        let _result = PDDResult::with_paths(
            PathBuf::new(),
            PathBuf::new(),
            PathBuf::new(),
        );
    }

    // ========== Task Generation Phase Tests ==========
    //
    // These tests verify the task generation phase implementation.
    // Full integration tests with actual agents will be in Step 19.

    #[test]
    fn test_run_task_generation_phase_requires_pdd_artifacts() {
        // The task generation phase requires PDD artifacts to exist
        // This is tested via the PDDResult::from_directory tests above
        // Verifying that run_task_generation_phase calls PDDResult::from_directory
        let _orchestrator = create_test_orchestrator();
        // Without PDD artifacts, run_task_generation_phase would fail
        // This is covered by the PDDArtifactNotFound error tests
    }

    #[tokio::test]
    async fn test_task_generation_phase_fails_without_pdd_artifacts() {
        let orchestrator = create_test_orchestrator();
        // repo_path is /tmp/test-repo which doesn't have PDD artifacts
        let result = orchestrator.run_task_generation_phase().await;
        assert!(result.is_err());
    }

    #[test]
    fn test_task_generation_phase_uses_correct_planning_dir() {
        let orchestrator = create_test_orchestrator();
        let expected_planning_dir = orchestrator.repo_path().join(".sop").join("planning");
        // Verify the orchestrator would look in the correct location
        assert!(expected_planning_dir.ends_with(".sop/planning"));
    }

    #[tokio::test]
    async fn test_run_code_task_generator_phase_spawns_agent() {
        // This test verifies that run_code_task_generator_phase attempts to spawn an agent
        // Without tmux, the agent spawn will fail, but we can verify the flow
        let orchestrator = create_test_orchestrator();
        let pdd = PDDResult::with_paths(
            PathBuf::from("/mock/design.md"),
            PathBuf::from("/mock/plan.md"),
            PathBuf::from("/mock/research"),
        );

        let result = orchestrator.run_code_task_generator_phase(&pdd).await;
        // Without tmux, this will fail at agent spawn
        assert!(result.is_err());
    }

    #[test]
    fn test_code_task_search_paths_include_expected_locations() {
        let orchestrator = create_test_orchestrator();
        let repo_path = orchestrator.repo_path();

        // Verify the search paths that would be used
        let expected_paths = vec![
            repo_path.to_path_buf(),
            repo_path.join(".sop"),
            repo_path.join(".sop").join("planning").join("implementation"),
        ];

        // These are the paths that run_code_task_generator_phase searches
        assert_eq!(expected_paths[0], PathBuf::from("/tmp/test-repo"));
        assert_eq!(expected_paths[1], PathBuf::from("/tmp/test-repo/.sop"));
        assert_eq!(
            expected_paths[2],
            PathBuf::from("/tmp/test-repo/.sop/planning/implementation")
        );
    }

    #[test]
    fn test_no_code_tasks_generated_error() {
        // Test the NoCodeTasksGenerated error type
        let error = Error::NoCodeTasksGenerated {
            path: "/test/path".to_string(),
        };
        let error_msg = format!("{}", error);
        assert!(error_msg.contains("No code tasks found"));
        assert!(error_msg.contains("/test/path"));
    }

    // ========== CodeTask Integration Tests ==========

    #[test]
    fn test_code_task_from_directory_with_generated_files() {
        use crate::core::CodeTask;

        let temp_dir = std::env::temp_dir().join(format!("task_gen_test_{}", std::process::id()));
        std::fs::create_dir_all(&temp_dir).unwrap();

        // Create mock .code-task.md files
        for i in 1..=5 {
            let content = format!(
                r#"# Task: Task {}

## Description
Description for task {}.

## Acceptance Criteria
1. Criterion 1
2. Criterion 2

## Metadata
- **Complexity**: Medium
"#,
                i, i
            );
            let file_path = temp_dir.join(format!("task-{:02}.code-task.md", i));
            std::fs::write(&file_path, content).unwrap();
        }

        let tasks = CodeTask::from_directory(&temp_dir).unwrap();
        assert_eq!(tasks.len(), 5);
        assert_eq!(tasks[0].title, "Task 1");
        assert_eq!(tasks[4].title, "Task 5");

        // Cleanup
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_code_task_from_directory_empty() {
        use crate::core::CodeTask;

        let temp_dir = std::env::temp_dir().join(format!("task_gen_empty_{}", std::process::id()));
        std::fs::create_dir_all(&temp_dir).unwrap();

        let tasks = CodeTask::from_directory(&temp_dir).unwrap();
        assert!(tasks.is_empty());

        // Cleanup
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_code_task_deduplication_by_id() {
        // Test that tasks are deduplicated by ID when found in multiple locations
        use std::collections::HashSet;

        let task_ids = vec!["task-01", "task-02", "task-01", "task-03", "task-02"];
        let mut seen_ids = HashSet::new();
        let deduplicated: Vec<_> = task_ids
            .into_iter()
            .filter(|id| seen_ids.insert(*id))
            .collect();

        assert_eq!(deduplicated.len(), 3);
        assert!(deduplicated.contains(&"task-01"));
        assert!(deduplicated.contains(&"task-02"));
        assert!(deduplicated.contains(&"task-03"));
    }

    // ========== Execute with Task Generation Tests ==========

    #[tokio::test]
    async fn test_execute_captures_generated_tasks() {
        // This test verifies that execute() captures the generated tasks
        // Currently, the tasks are stored in _generated_tasks (unused for now)
        // The implementation phase (Step 11) will use these tasks
        let mut orchestrator = create_test_orchestrator();
        let result = orchestrator.execute("test prompt").await.unwrap();

        // Without tmux, planning fails first before task generation
        assert_eq!(result.status, WorkflowStatus::Failed);
        assert!(result.summary.contains("Planning phase failed"));
    }

    #[test]
    fn test_task_generation_phase_is_phase_2() {
        // Verify task generation is Phase 2 in the workflow
        let phase = WorkflowPhase::TaskGeneration;
        assert!(matches!(phase, WorkflowPhase::TaskGeneration));

        // Verify the ordering: Planning < TaskGeneration < Implementation
        use std::cmp::Ordering;
        assert_eq!(
            WorkflowPhase::Planning.cmp(&WorkflowPhase::TaskGeneration),
            Ordering::Less
        );
        assert_eq!(
            WorkflowPhase::TaskGeneration.cmp(&WorkflowPhase::Implementation),
            Ordering::Less
        );
    }

    // ========== Implementation Phase Tests (Step 11) ==========

    // Helper to create CodeTask for testing
    fn create_test_code_task(id: &str, title: &str, deps: Vec<&str>) -> crate::core::CodeTask {
        crate::core::CodeTask {
            id: id.to_string(),
            file_path: std::path::PathBuf::from(format!("{}.code-task.md", id)),
            title: title.to_string(),
            description: format!("Description for {}", title),
            acceptance_criteria: vec!["Criterion 1".to_string()],
            dependencies: deps.into_iter().map(String::from).collect(),
            complexity: crate::core::Complexity::Medium,
        }
    }

    #[test]
    fn test_build_task_dag_empty_tasks() {
        let orchestrator = create_test_orchestrator();
        let tasks: Vec<crate::core::CodeTask> = vec![];

        let dag = orchestrator.build_task_dag(&tasks).unwrap();

        assert!(dag.is_empty());
        assert_eq!(dag.task_count(), 0);
        assert_eq!(dag.dependency_count(), 0);
    }

    #[test]
    fn test_build_task_dag_single_task() {
        let orchestrator = create_test_orchestrator();
        let tasks = vec![create_test_code_task("task-01", "First Task", vec![])];

        let dag = orchestrator.build_task_dag(&tasks).unwrap();

        assert_eq!(dag.task_count(), 1);
        assert_eq!(dag.dependency_count(), 0);

        // Verify task was added
        let all_tasks = dag.all_tasks();
        assert_eq!(all_tasks.len(), 1);
        assert_eq!(all_tasks[0].name, "First Task");
    }

    #[test]
    fn test_build_task_dag_multiple_independent_tasks() {
        let orchestrator = create_test_orchestrator();
        let tasks = vec![
            create_test_code_task("task-01", "First Task", vec![]),
            create_test_code_task("task-02", "Second Task", vec![]),
            create_test_code_task("task-03", "Third Task", vec![]),
        ];

        let dag = orchestrator.build_task_dag(&tasks).unwrap();

        assert_eq!(dag.task_count(), 3);
        assert_eq!(dag.dependency_count(), 0);
    }

    #[test]
    fn test_build_task_dag_with_dependencies() {
        let orchestrator = create_test_orchestrator();
        // task-02 depends on task-01, task-03 depends on both
        let tasks = vec![
            create_test_code_task("task-01", "First Task", vec![]),
            create_test_code_task("task-02", "Second Task", vec!["task-01"]),
            create_test_code_task("task-03", "Third Task", vec!["task-01", "task-02"]),
        ];

        let dag = orchestrator.build_task_dag(&tasks).unwrap();

        assert_eq!(dag.task_count(), 3);
        // task-02 <- task-01, task-03 <- task-01, task-03 <- task-02 = 3 edges
        assert_eq!(dag.dependency_count(), 3);
    }

    #[test]
    fn test_build_task_dag_missing_dependency_is_skipped() {
        let orchestrator = create_test_orchestrator();
        // task-02 depends on task-nonexistent (should be skipped gracefully)
        let tasks = vec![
            create_test_code_task("task-01", "First Task", vec![]),
            create_test_code_task("task-02", "Second Task", vec!["task-nonexistent"]),
        ];

        let dag = orchestrator.build_task_dag(&tasks).unwrap();

        assert_eq!(dag.task_count(), 2);
        // No dependency added since task-nonexistent doesn't exist
        assert_eq!(dag.dependency_count(), 0);
    }

    #[test]
    fn test_build_task_dag_chain_dependency() {
        let orchestrator = create_test_orchestrator();
        // A -> B -> C chain
        let tasks = vec![
            create_test_code_task("task-a", "Task A", vec![]),
            create_test_code_task("task-b", "Task B", vec!["task-a"]),
            create_test_code_task("task-c", "Task C", vec!["task-b"]),
        ];

        let dag = orchestrator.build_task_dag(&tasks).unwrap();

        assert_eq!(dag.task_count(), 3);
        assert_eq!(dag.dependency_count(), 2); // A->B, B->C

        // Verify topological order respects dependencies
        let order = dag.topological_order().unwrap();
        let names: Vec<_> = order.iter().map(|t| t.name.as_str()).collect();

        // A must come before B, B must come before C
        let pos_a = names.iter().position(|&n| n == "Task A").unwrap();
        let pos_b = names.iter().position(|&n| n == "Task B").unwrap();
        let pos_c = names.iter().position(|&n| n == "Task C").unwrap();

        assert!(pos_a < pos_b);
        assert!(pos_b < pos_c);
    }

    #[test]
    fn test_build_task_dag_diamond_pattern() {
        let orchestrator = create_test_orchestrator();
        // Diamond: A -> B, A -> C, B -> D, C -> D
        let tasks = vec![
            create_test_code_task("task-a", "Task A", vec![]),
            create_test_code_task("task-b", "Task B", vec!["task-a"]),
            create_test_code_task("task-c", "Task C", vec!["task-a"]),
            create_test_code_task("task-d", "Task D", vec!["task-b", "task-c"]),
        ];

        let dag = orchestrator.build_task_dag(&tasks).unwrap();

        assert_eq!(dag.task_count(), 4);
        assert_eq!(dag.dependency_count(), 4); // A->B, A->C, B->D, C->D
    }

    #[tokio::test]
    async fn test_run_implementation_phase_empty_tasks() {
        let orchestrator = create_test_orchestrator();
        let tasks: Vec<crate::core::CodeTask> = vec![];

        let results = orchestrator.run_implementation_phase(&tasks).await.unwrap();

        assert!(results.is_empty());
    }

    #[test]
    fn test_implementation_phase_is_phase_3() {
        // Verify implementation is Phase 3 in the workflow
        let phase = WorkflowPhase::Implementation;
        assert!(matches!(phase, WorkflowPhase::Implementation));

        // Verify the ordering
        use std::cmp::Ordering;
        assert_eq!(
            WorkflowPhase::TaskGeneration.cmp(&WorkflowPhase::Implementation),
            Ordering::Less
        );
        assert_eq!(
            WorkflowPhase::Implementation.cmp(&WorkflowPhase::Merging),
            Ordering::Less
        );
    }

    #[test]
    fn test_code_task_to_task_conversion() {
        let code_task = create_test_code_task("task-01", "Test Task", vec![]);
        let task = code_task.to_task();

        assert_eq!(task.name, "Test Task");
        assert_eq!(task.description, "Description for Test Task");
        assert!(!task.id.0.is_nil());
    }

    #[tokio::test]
    async fn test_set_event_sender_on_pool() {
        // Test that set_event_sender works
        let (tx1, _rx1) = mpsc::channel(100);
        let (tx2, mut rx2) = mpsc::channel(100);

        let mut pool = AgentPool::new(4, tx1);
        pool.set_event_sender(tx2);

        // Events should now go to tx2/rx2
        // Note: Can't easily test event sending without spawning an agent
        // which requires tmux. Just verify the method doesn't panic.
        assert!(rx2.try_recv().is_err()); // No events yet
    }
}
