# Zen v2 Implementation Plan

**Version:** 1.0
**Date:** 2026-01-30
**Based on:** detailed-design.md

---

## Implementation Checklist

- [ ] **Step 1:** Core Workflow Models and Types
- [ ] **Step 2:** Git State Manager Migration
- [ ] **Step 3:** AI-as-Human Proxy Foundation
- [ ] **Step 4:** Agent Pool Enhancements
- [ ] **Step 5:** Claude Code Headless Integration
- [ ] **Step 6:** Skills Orchestrator Skeleton
- [ ] **Step 7:** Phase 1 - PDD Skill Integration
- [ ] **Step 8:** Task and DAG Data Models
- [ ] **Step 9:** Phase 2 - Code Task Generator Integration
- [ ] **Step 10:** DAG Scheduler with Parallel Execution
- [ ] **Step 11:** Phase 3 - Parallel Code Assist Execution
- [ ] **Step 12:** Phase 4 - Merge and Conflict Resolution
- [ ] **Step 13:** Phase 5 - Codebase Summary Integration
- [ ] **Step 14:** Health Monitor and Stuck Detection
- [ ] **Step 15:** Reactive Planner (Plan Change Detection)
- [ ] **Step 16:** TUI Dashboard Enhancements
- [ ] **Step 17:** CLI Commands (run, review, accept, reject)
- [ ] **Step 18:** Worktree Auto-Cleanup
- [ ] **Step 19:** Integration Testing and Polish
- [ ] **Step 20:** Documentation and User Guide

---

## Step 1: Core Workflow Models and Types

**Objective:** Define the foundational data types for workflows, phases, and configuration that the entire orchestration system will use.

**Implementation Guidance:**

1. Create `src/workflow/mod.rs` with module exports
2. Create `src/workflow/types.rs` with:
   - `WorkflowId` - UUID-based identifier (similar to existing `SessionId`)
   - `WorkflowPhase` enum (Planning, TaskGeneration, Implementation, Merging, Documentation, Complete)
   - `WorkflowStatus` enum (Pending, Running, Paused, Completed, Failed)
   - `WorkflowConfig` struct (update_docs flag, max_parallel_agents, etc.)
   - `Workflow` struct (id, name, prompt, phase, status, timestamps, config)
3. Create `src/workflow/state.rs` with:
   - `WorkflowState` struct for tracking runtime state
   - Methods for phase transitions with validation
4. Implement `Serialize`/`Deserialize` for all types (git notes storage)
5. Add comprehensive unit tests for state transitions

**Test Requirements:**
- Workflow creation with default config
- Phase transition validation (can't skip phases)
- Serialization round-trip tests

**Integration:**
- Add `pub mod workflow;` to `src/lib.rs`
- Types will be consumed by Skills Orchestrator in Step 6

**Demo:** Run `cargo test workflow` to verify all workflow types serialize correctly and phase transitions are enforced.

---

## Step 2: Git State Manager Migration

**Objective:** Migrate from JSON state file to git-native storage using the existing GitRefs and GitNotes modules.

**Implementation Guidance:**

1. Create `src/state/mod.rs` with module exports
2. Create `src/state/manager.rs` with `GitStateManager`:
   ```rust
   pub struct GitStateManager {
       refs: GitRefs,
       notes: GitNotes,
       ops: GitOps,
   }
   ```
3. Implement workflow persistence:
   - `save_workflow(&self, workflow: &Workflow)` → stores in `refs/notes/zen/workflows`
   - `load_workflow(&self, id: &WorkflowId)` → retrieves from notes
   - `list_workflows(&self)` → lists all active workflows
4. Implement task persistence (placeholder for Step 8):
   - `save_task(&self, task: &Task)`
   - `load_task(&self, id: &TaskId)`
5. Add migration tool for existing JSON state → git-native
6. Update `State` in `session.rs` to use `GitStateManager` for new data

**Test Requirements:**
- Workflow CRUD operations via git notes
- Concurrent workflow storage (multiple workflows)
- Migration from JSON preserves data integrity

**Integration:**
- Existing sessions continue to work (backward compatible)
- New workflow data uses git-native storage

**Demo:** Create a workflow, verify it appears in `git notes --ref=refs/notes/zen/workflows list`, restart Zen, workflow persists.

---

## Step 3: AI-as-Human Proxy Foundation

**Objective:** Build the core AI-as-Human proxy that autonomously answers skill clarification questions.

**Implementation Guidance:**

1. Create `src/orchestration/mod.rs` with module exports
2. Create `src/orchestration/ai_human.rs` with:
   ```rust
   pub struct AIHumanProxy {
       original_prompt: String,
       context: Arc<RwLock<ConversationContext>>,
       model: String,  // "haiku" for fast responses
   }

   pub struct ConversationContext {
       qa_history: Vec<(String, String)>,
       decisions: HashMap<String, String>,
   }
   ```
3. Implement `answer_question(&self, question: &str) -> String`:
   - Format prompt with original user intent + question + context
   - Call Claude via headless mode (Step 5 will provide this)
   - For now, use mock responses for testing
4. Implement `needs_escalation(&self, question: &str) -> bool`:
   - Detect ambiguous questions requiring human input
5. Implement context tracking:
   - `record(&mut self, question: &str, answer: &str)`
   - `summary(&self) -> String`

**Test Requirements:**
- Context accumulation across multiple Q&A
- Decision extraction (naming conventions, etc.)
- Escalation detection for ambiguous patterns

**Integration:**
- Will be used by Skills Orchestrator (Step 6)
- Will integrate with Claude headless mode (Step 5)

**Demo:** Unit test shows mock AI-as-Human answering a series of questions with consistent context tracking.

---

## Step 4: Agent Pool Enhancements

**Objective:** Enhance the Agent module to support multiple concurrent agents with unique identifiers and lifecycle management.

**Implementation Guidance:**

1. Update `src/agent.rs`:
   - Add `AgentId` newtype (UUID-based)
   - Add `AgentStatus` enum (Idle, Running, Stuck, Failed, Terminated)
   - Add task association: `current_task: Option<TaskId>`
2. Create `src/orchestration/pool.rs` with `AgentPool`:
   ```rust
   pub struct AgentPool {
       agents: HashMap<AgentId, AgentHandle>,
       max_concurrent: usize,
       event_tx: mpsc::Sender<AgentEvent>,
   }
   ```
3. Implement agent lifecycle:
   - `spawn(&mut self, task: &Task, skill: &str) -> Result<AgentId>`
   - `terminate(&mut self, id: &AgentId) -> Result<()>`
   - `get(&self, id: &AgentId) -> Option<&AgentHandle>`
   - `active_count(&self) -> usize`
   - `has_capacity(&self) -> bool`
4. Implement `AgentHandle`:
   - Holds tmux session name, worktree path, cancel token
   - `send(&self, input: &str) -> Result<()>` - send to tmux pane
   - `read_output(&self) -> Result<AgentOutput>` - capture pane
5. Define `AgentEvent` enum for pool events

**Test Requirements:**
- Pool capacity enforcement
- Agent spawn creates worktree + tmux session
- Agent terminate cleans up resources
- Event emission on state changes

**Integration:**
- Builds on existing `Agent` and `Tmux` modules
- Will be used by Skills Orchestrator (Step 6)

**Demo:** Spawn 3 agents in pool (up to max capacity), verify each has isolated worktree and tmux session, terminate one, verify cleanup.

---

## Step 5: Claude Code Headless Integration

**Objective:** Implement headless Claude Code execution with JSON output parsing and session management.

**Implementation Guidance:**

1. Create `src/orchestration/claude.rs` with `ClaudeHeadless`:
   ```rust
   pub struct ClaudeHeadless {
       binary: PathBuf,
       output_format: String,  // "json"
   }
   ```
2. Implement headless execution:
   - `execute(&self, prompt: &str, cwd: &Path) -> Result<ClaudeResponse>`
   - Uses `claude -p "prompt" --output-format json`
   - Parses JSON response structure
3. Define response types:
   ```rust
   pub struct ClaudeResponse {
       pub session_id: Option<String>,
       pub result: ResultType,
       pub cost_usd: Option<f64>,
   }

   pub enum ResultType {
       Success { output: String },
       Error { message: String },
   }
   ```
4. Implement session continuation:
   - `continue_session(&self, session_id: &str, prompt: &str) -> Result<ClaudeResponse>`
   - Uses `--session-id` flag for multi-turn
5. Integrate with AIHumanProxy:
   - Use haiku model for fast Q&A responses
   - Update mock to use real Claude calls

**Test Requirements:**
- Headless execution returns parsed JSON
- Session ID captured for continuation
- Error handling for non-zero exit codes
- Timeout handling for long operations

**Integration:**
- AIHumanProxy (Step 3) uses this for real responses
- AgentPool (Step 4) uses this for agent execution

**Demo:** Execute `claude -p "Hello" --output-format json` programmatically, parse response, continue session with follow-up.

---

## Step 6: Skills Orchestrator Skeleton

**Objective:** Build the skeleton of the Skills Orchestrator that will coordinate the full workflow phases.

**Implementation Guidance:**

1. Create `src/orchestration/skills.rs` with `SkillsOrchestrator`:
   ```rust
   pub struct SkillsOrchestrator {
       ai_human: AIHumanProxy,
       agent_pool: Arc<RwLock<AgentPool>>,
       state: Arc<RwLock<WorkflowState>>,
       claude: ClaudeHeadless,
   }
   ```
2. Implement main execution flow (skeleton):
   ```rust
   pub async fn execute(&self, prompt: &str) -> Result<WorkflowResult> {
       // Phase 1: Planning (/pdd) - Step 7
       // Phase 2: Task Generation (/code-task-generator) - Step 9
       // Phase 3: Implementation (/code-assist parallel) - Step 11
       // Phase 4: Merge and Resolve - Step 12
       // Phase 5: Documentation (/codebase-summary) - Step 13
   }
   ```
3. Implement phase controller:
   - `PhaseController` tracks current phase
   - `transition(&mut self, phase: WorkflowPhase)` with validation
   - Emits phase change events for TUI
4. Implement output monitoring loop (shared pattern):
   ```rust
   async fn monitor_agent_output(&self, agent: &AgentHandle) -> Result<SkillResult> {
       loop {
           match agent.read_output().await? {
               AgentOutput::Question(q) => {
                   let answer = self.ai_human.answer_question(&q).await;
                   agent.send(&answer).await?;
               }
               AgentOutput::Completed => break,
               AgentOutput::Error(e) => return Err(e.into()),
               _ => continue,
           }
       }
   }
   ```
5. Wire up TEA integration:
   - Add `Message::WorkflowPhaseChanged(WorkflowPhase)`
   - Add `Message::WorkflowCompleted(WorkflowId)`
   - Add `Command::StartWorkflow(String)` for prompt

**Test Requirements:**
- Phase transitions emit events
- Monitor loop handles Q&A correctly
- Workflow state persisted on phase changes

**Integration:**
- Consumes AIHumanProxy, AgentPool, ClaudeHeadless
- Integrates with TEA pattern for UI updates

**Demo:** Start workflow skeleton (no actual skills yet), verify phase transitions logged and state persisted.

---

## Step 7: Phase 1 - PDD Skill Integration

**Objective:** Implement Phase 1 of the workflow - running /pdd skill with AI-as-Human answering questions.

**Implementation Guidance:**

1. Add `run_pdd_phase()` to SkillsOrchestrator:
   ```rust
   async fn run_pdd_phase(&self, prompt: &str) -> Result<PDDResult> {
       let agent = self.agent_pool.write().await
           .spawn_for_skill("pdd").await?;

       // Send /pdd invocation
       agent.send(&format!("/pdd\n\nrough_idea: {}", prompt)).await?;

       // Monitor and answer questions via AI-as-Human
       self.monitor_agent_output(&agent).await?;

       // Parse PDD artifacts
       PDDResult::from_directory(&agent.worktree_path().join(".sop/planning"))
   }
   ```
2. Define `PDDResult`:
   ```rust
   pub struct PDDResult {
       pub design_path: PathBuf,      // detailed-design.md
       pub plan_path: PathBuf,        // plan.md
       pub research_dir: PathBuf,     // research/
   }
   ```
3. Implement question detection in output:
   - Parse tmux capture for question patterns
   - Detect waiting-for-input state (cursor position, patterns)
4. Implement artifact validation:
   - Verify required files exist after /pdd completes
   - Parse plan.md to extract steps count

**Test Requirements:**
- PDD invocation sends correct command
- Questions detected and answered via AI-as-Human
- Artifacts parsed correctly after completion
- Error handling if /pdd fails

**Integration:**
- Called by SkillsOrchestrator.execute() in Phase 1
- Output feeds into Phase 2 (code-task-generator)

**Demo:** Run `zen "build user authentication"`, watch PDD skill execute with AI answering clarification questions, see `.sop/planning/` artifacts created.

---

## Step 8: Task and DAG Data Models

**Objective:** Define task data models and the DAG structure for parallel execution scheduling.

**Implementation Guidance:**

1. Create `src/core/mod.rs` with module exports
2. Create `src/core/task.rs`:
   ```rust
   pub struct TaskId(Uuid);

   pub enum TaskStatus {
       Pending, Ready, Running, Completed, Failed { error: String }, Blocked { reason: String }
   }

   pub struct Task {
       pub id: TaskId,
       pub name: String,
       pub description: String,
       pub status: TaskStatus,
       pub worktree_path: Option<PathBuf>,
       pub branch_name: Option<String>,
       pub agent_id: Option<AgentId>,
       pub created_at: DateTime<Utc>,
       pub started_at: Option<DateTime<Utc>>,
       pub completed_at: Option<DateTime<Utc>>,
       pub commit_hash: Option<String>,
   }
   ```
3. Create `src/core/dag.rs` with `TaskDAG`:
   ```rust
   use petgraph::graph::{DiGraph, NodeIndex};

   pub struct TaskDAG {
       graph: DiGraph<Task, DependencyType>,
       task_index: HashMap<TaskId, NodeIndex>,
   }

   pub enum DependencyType {
       DataDependency,
       FileDependency { files: Vec<PathBuf> },
       SemanticDependency { reason: String },
   }
   ```
4. Implement DAG operations:
   - `add_task(&mut self, task: Task, deps: Vec<TaskId>)`
   - `ready_tasks(&self, completed: &HashSet<TaskId>) -> Vec<&Task>`
   - `complete_task(&mut self, id: &TaskId)`
   - `all_complete(&self, completed: &HashSet<TaskId>) -> bool`
   - `topological_order(&self) -> Vec<&Task>`
5. Add cycle detection (no circular dependencies)
6. Add petgraph to Cargo.toml dependencies

**Test Requirements:**
- Task creation with defaults
- DAG edge creation with dependency types
- Ready tasks returns only those with satisfied dependencies
- Cycle detection throws error
- Topological ordering correct

**Integration:**
- Task persisted via GitStateManager (Step 2)
- DAG used by scheduler (Step 10)

**Demo:** Create DAG with 5 tasks (A→C, B→C, C→D, E independent), verify ready_tasks returns [A, B, E], complete A, verify ready_tasks returns [B, E].

---

## Step 9: Phase 2 - Code Task Generator Integration

**Objective:** Implement Phase 2 - running /code-task-generator to create .code-task.md files from the PDD plan.

**Implementation Guidance:**

1. Create `src/core/code_task.rs`:
   ```rust
   pub struct CodeTask {
       pub id: String,
       pub file_path: PathBuf,      // .code-task-001.md
       pub description: String,
       pub acceptance_criteria: Vec<String>,
       pub dependencies: Vec<String>,
   }

   impl CodeTask {
       pub fn from_directory(dir: &Path) -> Result<Vec<Self>>
       pub fn to_task(&self) -> Task
   }
   ```
2. Add `run_task_generation_phase()` to SkillsOrchestrator:
   ```rust
   async fn run_task_generation_phase(&self, pdd: &PDDResult) -> Result<Vec<CodeTask>> {
       let agent = self.agent_pool.write().await
           .spawn_for_skill("code-task-generator").await?;

       // Feed PDD plan to code-task-generator
       agent.send(&format!(
           "/code-task-generator\n\ninput: {}",
           pdd.plan_path.display()
       )).await?;

       // AI-as-Human approves task breakdown
       self.monitor_agent_output(&agent).await?;

       // Parse generated code tasks
       CodeTask::from_directory(&agent.worktree_path())
   }
   ```
3. Implement code task file parsing:
   - Scan for `.code-task-*.md` files
   - Parse markdown structure (YAML front matter + body)
   - Extract dependencies from task content
4. Implement AI-inferred dependency detection:
   - Analyze task descriptions for implicit dependencies
   - Use AI to suggest dependency ordering

**Test Requirements:**
- Code task parsing extracts all fields
- Multiple .code-task files discovered
- Dependencies parsed from task content
- AI dependency inference returns reasonable ordering

**Integration:**
- Called by SkillsOrchestrator.execute() in Phase 2
- Output converted to Task + DAG for Phase 3

**Demo:** After PDD completes, see /code-task-generator create 5-10 .code-task.md files, verify they parse correctly into Tasks.

---

## Step 10: DAG Scheduler with Parallel Execution

**Objective:** Build the parallel execution scheduler that dispatches ready tasks to the agent pool.

**Implementation Guidance:**

1. Create `src/orchestration/scheduler.rs`:
   ```rust
   pub struct Scheduler {
       dag: Arc<RwLock<TaskDAG>>,
       agent_pool: Arc<RwLock<AgentPool>>,
       event_tx: mpsc::Sender<SchedulerEvent>,
   }

   pub enum SchedulerEvent {
       TaskStarted { task_id: TaskId, agent_id: AgentId },
       TaskCompleted { task_id: TaskId, commit: String },
       TaskFailed { task_id: TaskId, error: String },
       AllTasksComplete,
   }
   ```
2. Implement main scheduling loop:
   ```rust
   pub async fn run(&self) -> Result<()> {
       let mut completed = HashSet::new();

       while !self.dag.read().await.all_complete(&completed) {
           // Get tasks ready to run
           let ready = self.dag.read().await.ready_tasks(&completed);

           // Spawn agents for ready tasks (up to pool capacity)
           for task in ready {
               if self.agent_pool.read().await.has_capacity() {
                   self.spawn_task(task).await?;
               }
           }

           // Wait for any agent to complete
           let event = self.wait_for_completion().await?;
           // Handle completion, update completed set
       }
   }
   ```
3. Implement task spawning with worktree isolation:
   - Create worktree for task: `~/.zen/worktrees/{task-id}`
   - Create branch: `zen/task/{task-id}`
   - Spawn agent in worktree
4. Wire up TEA messages:
   - `Message::TaskStarted { task_id, agent_id }`
   - `Message::TaskCompleted { task_id }`
   - `Message::TaskFailed { task_id, error }`

**Test Requirements:**
- Parallel execution up to pool capacity
- Dependency ordering respected
- Task completion updates DAG state
- Events emitted for TUI updates

**Integration:**
- Used by SkillsOrchestrator in Phase 3
- Integrates with AgentPool, TaskDAG

**Demo:** Schedule 5 tasks with dependencies, verify 2-3 run in parallel initially, dependencies block correctly, all complete in correct order.

---

## Step 11: Phase 3 - Parallel Code Assist Execution

**Objective:** Execute /code-assist in parallel for each task, with AI-as-Human mode for minimal interaction.

**Implementation Guidance:**

1. Add `run_implementation_phase()` to SkillsOrchestrator:
   ```rust
   async fn run_implementation_phase(&self, tasks: &[CodeTask]) -> Result<Vec<ImplResult>> {
       // Build DAG from code tasks (AI-inferred dependencies)
       let dag = self.build_task_dag(tasks).await?;

       // Create scheduler
       let scheduler = Scheduler::new(dag, self.agent_pool.clone());

       // Set up task execution with /code-assist
       scheduler.set_task_executor(|task, agent| async move {
           agent.send(&format!(
               "/code-assist\n\ntask_description: {}\nmode: auto",
               task.description
           )).await?;

           // Minimal interaction in auto mode
           self.monitor_agent_output(&agent).await
       });

       // Run scheduler
       scheduler.run().await
   }
   ```
2. Define `ImplResult`:
   ```rust
   pub struct ImplResult {
       pub task_id: TaskId,
       pub worktree: PathBuf,
       pub commit: String,
       pub files_changed: Vec<PathBuf>,
   }
   ```
3. Implement code-assist auto mode:
   - Pass task description as input
   - AI-as-Human answers minimal questions
   - Capture final commit hash
4. Track implementation progress:
   - Update task status in GitStateManager
   - Emit progress events for TUI

**Test Requirements:**
- Multiple /code-assist agents run in parallel
- Each agent in isolated worktree
- Commits captured for each completed task
- Progress tracked in state

**Integration:**
- Central Phase 3 of the workflow
- Output feeds into Phase 4 (merge)

**Demo:** Start workflow with 4 independent tasks, see 4 parallel agents working, each completes with a commit, progress shown in TUI.

---

## Step 12: Phase 4 - Merge and Conflict Resolution

**Objective:** Merge completed task worktrees to staging branch with AI-assisted conflict resolution.

**Implementation Guidance:**

1. Create `src/orchestration/resolver.rs`:
   ```rust
   pub struct ConflictResolver {
       git_ops: GitOps,
       agent_pool: Arc<RwLock<AgentPool>>,
   }

   pub enum MergeResult {
       Success { commit: String },
       Conflicts { files: Vec<ConflictFile> },
       Failed { error: String },
   }

   pub struct ConflictFile {
       pub path: PathBuf,
       pub ours: String,
       pub theirs: String,
       pub base: Option<String>,
   }
   ```
2. Add `run_merge_phase()` to SkillsOrchestrator:
   ```rust
   async fn run_merge_phase(&self, results: &[ImplResult]) -> Result<()> {
       let resolver = ConflictResolver::new(...);

       // Create staging branch
       let staging = format!("zen/staging/{}", workflow_id);

       // Merge each task worktree
       for result in results {
           match resolver.merge(&result.worktree, &staging).await? {
               MergeResult::Success { .. } => continue,
               MergeResult::Conflicts { files } => {
                   // Spawn resolver agent
                   resolver.resolve_conflicts(files).await?;
               }
               MergeResult::Failed { error } => return Err(...),
           }
       }
   }
   ```
3. Implement conflict resolution agent:
   - Spawn dedicated agent for conflict resolution
   - Provide conflict context in prompt
   - Agent uses Edit tool to fix each file
   - Verify resolution before committing
4. Add conflict detection using git2:
   - Check for conflict markers in merged files
   - Extract ours/theirs/base content

**Test Requirements:**
- Clean merges succeed without resolver
- Conflicts detected correctly
- Resolver agent called for conflicts
- Resolution verified before commit

**Integration:**
- Phase 4 of workflow
- Staging branch ready for user review

**Demo:** Simulate merge conflict between two tasks modifying same file, watch resolver agent fix it, verify staging branch has combined changes.

---

## Step 13: Phase 5 - Codebase Summary Integration

**Objective:** Optionally run /codebase-summary to update documentation with new components.

**Implementation Guidance:**

1. Add `run_documentation_phase()` to SkillsOrchestrator:
   ```rust
   async fn run_documentation_phase(&self) -> Result<()> {
       if !self.state.read().await.config.update_docs {
           return Ok(());
       }

       let agent = self.agent_pool.write().await
           .spawn_for_skill("codebase-summary").await?;

       // Run codebase-summary on staging branch
       agent.send("/codebase-summary").await?;

       // AI-as-Human handles any questions
       self.monitor_agent_output(&agent).await?;
   }
   ```
2. Configure documentation targets:
   - AGENTS.md update (for AI context)
   - README.md update (if significant features)
   - CONTRIBUTING.md update (if new patterns)
3. Make phase optional via WorkflowConfig:
   - Default: enabled
   - User can disable via config or flag

**Test Requirements:**
- Phase skipped if disabled
- Documentation files updated
- Changes committed to staging

**Integration:**
- Final phase before workflow complete
- Optional based on config

**Demo:** Complete full workflow, see AGENTS.md updated with new component documentation, commit message references workflow.

---

## Step 14: Health Monitor and Stuck Detection

**Objective:** Detect stuck or failing agents and implement AI-driven recovery.

**Implementation Guidance:**

1. Create `src/orchestration/health.rs`:
   ```rust
   pub struct HealthMonitor {
       config: HealthConfig,
       agent_pool: Arc<RwLock<AgentPool>>,
       event_tx: mpsc::Sender<HealthEvent>,
   }

   pub struct HealthConfig {
       pub stuck_threshold: Duration,      // 5 minutes
       pub max_retries: u32,               // 3
       pub stuck_patterns: Vec<String>,    // "rate limit", "error"
   }

   pub enum RecoveryAction {
       Restart,
       Reassign { to_agent: AgentId },
       Decompose { into_tasks: Vec<Task> },
       Escalate { message: String },
       Abort,
   }
   ```
2. Implement monitoring loop:
   - Check agent activity timestamps
   - Detect stuck patterns in output
   - Emit HealthEvent for issues
3. Implement AI-driven recovery:
   ```rust
   async fn determine_recovery(&self, agent: &Agent) -> RecoveryAction {
       // Use AI to analyze situation
       let prompt = format!(
           "Agent working on '{}' appears stuck. Output: {}.
            What should we do: restart, decompose into smaller tasks,
            or escalate to user?",
           task.description,
           recent_output
       );
       // Parse AI response into RecoveryAction
   }
   ```
4. Integrate with Scheduler:
   - Scheduler subscribes to health events
   - Executes recovery actions

**Test Requirements:**
- Stuck detection after timeout
- Pattern matching for error states
- Recovery action execution
- Retry limit enforcement

**Integration:**
- Runs alongside Scheduler
- Integrates with AgentPool for recovery

**Demo:** Simulate stuck agent (no output for 5 min), watch health monitor detect and restart, task completes after recovery.

---

## Step 15: Reactive Planner (Plan Change Detection)

**Objective:** Watch for plan/design file changes and trigger replanning automatically.

**Implementation Guidance:**

1. Create `src/orchestration/planner.rs`:
   ```rust
   pub struct ReactivePlanner {
       dag: Arc<RwLock<TaskDAG>>,
       watch_paths: Vec<PathBuf>,
       watcher: notify::RecommendedWatcher,
   }
   ```
2. Implement file watching:
   - Watch `.sop/planning/` for changes
   - Debounce rapid changes (1s window)
   - Trigger replan on significant changes
3. Implement replanning:
   - Parse updated plan/design
   - Diff against current task list
   - Add new tasks, mark removed as cancelled
   - Recompute dependencies
4. Handle in-flight work:
   - Don't interrupt running agents
   - Queue new tasks for after current batch
   - Notify user of plan changes

**Test Requirements:**
- File changes detected
- Plan diff computed correctly
- Running tasks not interrupted
- New tasks added to DAG

**Integration:**
- Runs alongside workflow execution
- Updates DAG and notifies Scheduler

**Demo:** While workflow runs, edit plan.md to add a new step, see Zen detect change and add new task to queue.

---

## Step 16: TUI Dashboard Enhancements

**Objective:** Enhance the TUI to display workflow progress, multiple agents, and task status.

**Implementation Guidance:**

1. Update `src/ui.rs` for new layouts:
   - Workflow overview panel (phase, progress)
   - Multi-agent list with status indicators
   - Task DAG visualization (ASCII)
   - Log/notification area
2. Add new UI components:
   ```rust
   fn render_workflow_header(f: &mut Frame, area: Rect, workflow: &Workflow)
   fn render_agent_grid(f: &mut Frame, area: Rect, agents: &[AgentView])
   fn render_task_dag(f: &mut Frame, area: Rect, dag: &TaskDAG)
   fn render_phase_progress(f: &mut Frame, area: Rect, phase: WorkflowPhase)
   ```
3. Update `RenderState`:
   - Add `workflow: Option<WorkflowView>`
   - Add `tasks: Vec<TaskView>`
   - Add `phase: WorkflowPhase`
4. Implement keyboard navigation:
   - Arrow keys to select agent
   - Enter to attach to agent pane
   - Tab to switch panels
   - `p` to show phase details
   - `d` to show DAG view
5. Handle TEA messages for real-time updates:
   - `Message::WorkflowPhaseChanged` → update phase display
   - `Message::TaskStarted/Completed` → update task list
   - `Message::AgentOutputLine` → update preview

**Test Requirements:**
- Layout renders without panic
- All panels display correct data
- Keyboard navigation works
- Real-time updates reflected

**Integration:**
- Consumes WorkflowState, TaskDAG, AgentPool
- Driven by TEA message updates

**Demo:** Start workflow, see TUI show 5 phases, 4 parallel agents with progress bars, task DAG with dependencies, live output.

---

## Step 17: CLI Commands (run, review, accept, reject)

**Objective:** Implement the CLI interface for workflow operations.

**Implementation Guidance:**

1. Update `src/main.rs` with clap commands:
   ```rust
   #[derive(Parser)]
   enum Command {
       /// Run a workflow with natural language prompt
       Run {
           #[arg(short, long)]
           prompt: String,
           #[arg(short, long)]
           headless: bool,
       },
       /// Review completed workflow
       Review {
           workflow_id: Option<WorkflowId>,
       },
       /// Accept and merge to main
       Accept {
           workflow_id: Option<WorkflowId>,
           #[arg(long)]
           task_id: Option<TaskId>,  // Accept specific task
       },
       /// Reject and rollback
       Reject {
           workflow_id: WorkflowId,
           #[arg(long)]
           task_id: Option<TaskId>,  // Reject specific task
       },
       /// Show workflow status
       Status,
       /// Attach to agent pane
       Attach {
           agent_id: AgentId,
       },
       /// Reset all state
       Reset {
           #[arg(long)]
           force: bool,
       },
   }
   ```
2. Implement `zen run`:
   - Parse prompt, create workflow
   - Start SkillsOrchestrator.execute()
   - Show TUI with progress
3. Implement `zen review`:
   - Load completed workflow
   - Show summary of changes
   - List tasks with status
   - Show diff of staging vs main
4. Implement `zen accept`:
   - Merge staging to main
   - Clean up worktrees
   - Mark workflow complete
5. Implement `zen reject`:
   - Delete staging branch
   - Clean up worktrees
   - Mark workflow rejected
6. Implement headless mode:
   - `--headless` flag for CI/scripting
   - JSON output for status
   - Exit code indicates success/failure

**Test Requirements:**
- CLI parsing for all commands
- Run creates and executes workflow
- Review shows correct summary
- Accept merges correctly
- Reject cleans up

**Integration:**
- Entry point for all user interactions
- Drives SkillsOrchestrator

**Demo:** Run `zen run -p "build auth"`, wait for completion, run `zen review`, run `zen accept`, verify changes on main.

---

## Step 18: Worktree Auto-Cleanup

**Objective:** Automatically clean up merged worktrees and orphaned resources.

**Implementation Guidance:**

1. Create `src/cleanup.rs`:
   ```rust
   pub struct CleanupManager {
       git_ops: GitOps,
       config: CleanupConfig,
   }

   pub struct CleanupConfig {
       pub auto_cleanup: bool,
       pub cleanup_delay: Duration,  // Wait before cleanup
       pub keep_failed: bool,        // Keep failed task worktrees
   }
   ```
2. Implement automatic cleanup:
   - After task merge: remove worktree, keep branch
   - After workflow accept: remove all worktrees, optionally delete branches
   - After workflow reject: remove all worktrees and branches
3. Implement orphan detection:
   - Find worktrees not associated with any workflow
   - Find tmux sessions not associated with any agent
   - Offer cleanup via `zen cleanup` command
4. Add background cleanup actor:
   - Runs periodically (every 5 min)
   - Cleans up completed workflows after delay
5. Integrate with session reconciliation:
   - On startup, detect orphaned resources
   - Log warnings, optionally auto-clean

**Test Requirements:**
- Merged worktrees removed
- Branches preserved for history
- Orphans detected correctly
- No cleanup during active workflow

**Integration:**
- Called after Phase 4 merge
- Called on accept/reject
- Background actor for periodic cleanup

**Demo:** Complete workflow, see worktrees cleaned up automatically, run `zen cleanup` to remove orphans.

---

## Step 19: Integration Testing and Polish

**Objective:** Comprehensive integration testing and final polish of the system.

**Implementation Guidance:**

1. Create `tests/integration/` directory:
   - `workflow_e2e.rs` - Full workflow execution
   - `parallel_agents.rs` - Parallel execution correctness
   - `conflict_resolution.rs` - Merge conflict handling
   - `recovery.rs` - Health monitor and recovery
2. Implement test fixtures:
   - Temporary git repos with realistic structure
   - Mock Claude responses for deterministic tests
   - Predefined task sets with known dependencies
3. Test scenarios:
   - Happy path: 5 tasks, 2 parallel, all succeed
   - Conflict: 2 tasks modify same file, resolution works
   - Failure: Task fails, recovery restarts, succeeds
   - Replan: Plan changes mid-execution
   - Headless: Full workflow via CLI without TUI
4. Performance testing:
   - Measure scheduling overhead
   - Verify 60 FPS render during execution
   - Check memory usage with many agents
5. Polish:
   - Review error messages for clarity
   - Add progress percentages
   - Improve notification messages
   - Handle edge cases (no git repo, no tmux, etc.)

**Test Requirements:**
- All integration tests pass
- E2E workflow completes correctly
- Performance within bounds
- Error messages clear and actionable

**Integration:**
- Validates all previous steps
- Exercises full system

**Demo:** Run full integration test suite, all pass, run `zen "build user auth"` on real codebase, complete successfully.

---

## Step 20: Documentation and User Guide

**Objective:** Create comprehensive documentation for users and contributors.

**Implementation Guidance:**

1. Update `docs/user-guide.md`:
   - Installation instructions
   - Quick start with `zen run`
   - Workflow phases explained
   - CLI reference
   - Configuration options
   - Troubleshooting
2. Update `docs/architecture.md`:
   - System overview diagram
   - Component descriptions
   - Data flow
   - Thread model
3. Create `docs/skills-integration.md`:
   - How Zen uses Skills
   - AI-as-Human pattern explained
   - Customizing skill behavior
4. Update `README.md`:
   - Project description
   - Key features
   - Installation
   - Basic usage
   - Links to detailed docs
5. Create `CONTRIBUTING.md`:
   - Development setup
   - Code style
   - Testing requirements
   - PR process

**Test Requirements:**
- Docs render correctly
- Examples work as written
- All CLI commands documented

**Integration:**
- Final deliverable
- Enables user adoption

**Demo:** New user reads README, runs `zen "build hello world"`, successfully completes first workflow.

---

## Summary

This implementation plan transforms Zen from a single-session manager into a parallel multi-agent orchestrator in 20 incremental steps. Key milestones:

- **Steps 1-5:** Foundation (models, state, AI-as-Human, agent pool, Claude integration)
- **Steps 6-7:** Skills Orchestrator + PDD Phase
- **Steps 8-11:** Task DAG + Parallel Execution
- **Steps 12-13:** Merge/Conflict Resolution + Documentation
- **Steps 14-15:** Health Monitoring + Reactive Planning
- **Steps 16-18:** TUI + CLI + Cleanup
- **Steps 19-20:** Testing + Documentation

Each step produces working, demoable functionality and builds incrementally on previous work. The existing decoupled game loop architecture is preserved and extended.

**Total estimated components:** ~15 new modules, ~8,000-12,000 lines of Rust code.
