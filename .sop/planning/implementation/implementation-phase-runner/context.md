# Implementation Phase Runner - Context

## Requirements Analysis

This task implements the implementation phase runner that executes /code-assist in parallel for each task, using the DAG scheduler to manage dependencies and concurrency.

### Core Requirements

1. **build_task_dag()** - Convert CodeTask list to TaskDAG with AI-inferred dependencies
2. **run_implementation_phase()** - Execute /code-assist in parallel using the scheduler
3. **Wire into execute()** - Replace stub with actual implementation

### Key Components

- `SkillsOrchestrator` - Orchestration engine (src/orchestration/skills.rs)
- `Scheduler` - DAG-based parallel execution (src/orchestration/scheduler.rs)
- `CodeTask` - Parsed .code-task.md files (src/core/code_task.rs)
- `TaskDAG` - Dependency graph (src/core/dag.rs)
- `ImplResult` - Task execution result (already defined in scheduler.rs)

### Dependencies

- Step 10: Scheduler with spawn_task(), run(), handle_completion()
- Step 9: CodeTask with dependencies field
- Step 6: monitor_agent_output() for question handling

## Implementation Patterns

### Existing Code Patterns

1. **Scheduler already has ImplResult** - Reuse from scheduler.rs:
   ```rust
   pub struct ImplResult {
       pub task_id: TaskId,
       pub worktree: PathBuf,
       pub commit: String,
   }
   ```

2. **CodeTask has dependencies** - Already parsed:
   ```rust
   pub dependencies: Vec<String>,
   ```

3. **Scheduler.run()** - Main execution loop:
   ```rust
   pub async fn run(&mut self, agent_rx: &mut mpsc::Receiver<AgentEvent>) -> Result<Vec<ImplResult>>
   ```

### Integration Points

1. `build_task_dag()` should:
   - Convert CodeTask to Task via `code_task.to_task()`
   - Add dependencies based on CodeTask.dependencies
   - Match dependencies by ID

2. `run_implementation_phase()` should:
   - Build DAG from CodeTasks
   - Create Scheduler
   - Configure task executor for /code-assist
   - Call scheduler.run()
   - Return Vec<ImplResult>

## File Paths

- Main: `src/orchestration/skills.rs`
- Scheduler: `src/orchestration/scheduler.rs`
- Core types: `src/core/code_task.rs`, `src/core/dag.rs`
