# Task: Create Scheduler Core

## Description
Create the Scheduler struct that manages parallel task execution, dispatching ready tasks to the agent pool while respecting dependencies and capacity limits.

## Background
The Scheduler is the execution engine that drives Phase 3. It monitors the DAG for ready tasks, spawns agents for them (up to capacity), and handles completions/failures. This enables true parallel execution.

## Reference Documentation
**Required:**
- Design: .sop/planning/design/detailed-design.md (Section 4.2 run_implementation_phase, Section 6.2 Scheduler pseudocode)

**Note:** You MUST read the detailed design document before beginning implementation.

## Technical Requirements
1. Create `src/orchestration/scheduler.rs` with:
   ```rust
   pub struct Scheduler {
       dag: Arc<RwLock<TaskDAG>>,
       agent_pool: Arc<RwLock<AgentPool>>,
       event_tx: mpsc::Sender<SchedulerEvent>,
       completed: HashSet<TaskId>,
   }

   pub enum SchedulerEvent {
       TaskStarted { task_id: TaskId, agent_id: AgentId },
       TaskCompleted { task_id: TaskId, commit: String },
       TaskFailed { task_id: TaskId, error: String },
       AllTasksComplete,
   }
   ```
2. Implement main scheduling loop:
   - `run(&mut self) -> Result<Vec<ImplResult>>`
3. Check ready tasks, spawn agents, wait for completions

## Dependencies
- TaskDAG from Step 8
- AgentPool from Step 4
- tokio for async

## Implementation Approach
1. Define Scheduler struct and SchedulerEvent enum
2. Implement run() with main loop
3. Get ready tasks from DAG
4. Spawn agents for ready tasks (up to capacity)
5. Wait for any completion using tokio::select!
6. Update completed set and repeat
7. Add tests with mock DAG and pool

## Acceptance Criteria

1. **Ready Task Dispatch**
   - Given 3 ready tasks and capacity for 2
   - When scheduler runs
   - Then 2 tasks are started (up to capacity)

2. **Dependency Respect**
   - Given A->B dependency
   - When A is not complete
   - Then B is not started

3. **Completion Handling**
   - Given a task completes
   - When scheduler processes completion
   - Then completed set is updated and new tasks may become ready

4. **All Complete Detection**
   - Given all tasks complete
   - When scheduler checks
   - Then AllTasksComplete event is emitted

5. **Event Emission**
   - Given task state changes
   - When changes occur
   - Then appropriate SchedulerEvents are sent

## Metadata
- **Complexity**: High
- **Labels**: Orchestration, Scheduler, Parallel, Core
- **Required Skills**: Rust, async, concurrency, tokio
