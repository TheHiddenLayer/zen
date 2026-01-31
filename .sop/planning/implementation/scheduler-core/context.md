# Context: Scheduler Core

## Requirements

Create the Scheduler struct that manages parallel task execution, dispatching ready tasks to the agent pool while respecting dependencies and capacity limits.

### Functional Requirements
1. Scheduler struct with TaskDAG, AgentPool, event channel, and completed set
2. SchedulerEvent enum for task lifecycle events
3. Main scheduling loop via `run()` method
4. Respect DAG dependencies when scheduling
5. Respect agent pool capacity limits
6. Emit events on task state changes

### Acceptance Criteria
1. Ready Task Dispatch - Given 3 ready tasks and capacity for 2, only 2 are started
2. Dependency Respect - If A->B and A not complete, B not started
3. Completion Handling - When task completes, completed set updates and new tasks may become ready
4. All Complete Detection - When all done, AllTasksComplete event emitted
5. Event Emission - Task state changes trigger appropriate SchedulerEvents

## Existing Patterns

### TaskDAG (src/core/dag.rs)
- `ready_tasks(&self, completed: &HashSet<TaskId>) -> Vec<&Task>` - returns tasks whose deps are satisfied
- `complete_task(&mut self, id: &TaskId) -> Result<()>` - marks task complete
- `all_complete(&self, completed: &HashSet<TaskId>) -> bool` - checks if all done
- Uses `Arc<RwLock<TaskDAG>>` pattern for shared state

### AgentPool (src/orchestration/pool.rs)
- `spawn(&mut self, task_id: &TaskId, skill: &str) -> Result<AgentId>` - spawns agent
- `has_capacity() -> bool` - checks if more agents can be spawned
- `terminate(&mut self, id: &AgentId) -> Result<()>` - removes agent
- Uses `Arc<RwLock<AgentPool>>` pattern
- Emits AgentEvent via mpsc channel

### AgentEvent enum
```rust
pub enum AgentEvent {
    Started { agent_id: AgentId, task_id: TaskId },
    Completed { agent_id: AgentId, exit_code: i32 },
    Failed { agent_id: AgentId, error: String },
    StuckDetected { agent_id: AgentId, duration: Duration },
    Terminated { agent_id: AgentId },
}
```

### TaskId and AgentId
- Both are UUID-based newtypes with `new()`, `short()`, Display, Hash, Clone, Copy

## Implementation Paths

- New file: `src/orchestration/scheduler.rs`
- Update: `src/orchestration/mod.rs` to export Scheduler types
