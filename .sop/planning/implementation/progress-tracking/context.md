# Progress Tracking Implementation Context

## Task Overview
Implement progress tracking for the implementation phase that:
1. Updates task status in GitStateManager
2. Emits events for TUI display
3. Calculates progress percentage
4. Adds TEA Message variants for task updates

## Existing Components

### Scheduler (src/orchestration/scheduler.rs)
- Already has `SchedulerEvent` enum with TaskStarted, TaskCompleted, TaskFailed, AllTasksComplete
- Has `handle_completion()` and `handle_failure()` methods
- Tracks `completed: HashSet<TaskId>` for completion status
- Has `event_tx: mpsc::Sender<SchedulerEvent>` for emitting events
- Missing: GitStateManager integration, progress percentage calculation

### GitStateManager (src/state/manager.rs)
- Has `save_workflow()` and `load_workflow()` methods
- Missing: `save_task()` and `load_task()` methods per design doc Section 4.7

### TEA Message System (src/tea/message.rs)
- Has existing Message enum variants for Session operations
- Missing: Task-related message variants as specified

### Task Data Model (src/core/task.rs)
- Complete Task struct with status, timing, worktree_path, commit_hash
- TaskStatus enum with Pending, Ready, Running, Completed, Failed, Blocked
- TaskId with UUID and serialization support

## Implementation Requirements

1. **Add save_task/load_task to GitStateManager** (Section 4.7)
   - save_task(&self, task: &Task) -> Result<()>
   - load_task(&self, id: &TaskId) -> Result<Option<Task>>
   - list_tasks(&self) -> Result<Vec<Task>>

2. **Add progress tracking to Scheduler**
   - Add state_manager: Option<GitStateManager> field
   - Call save_task() on status changes
   - Add progress_percentage() method: completed / total

3. **Add TEA Message variants**
   - Message::TaskStarted { task_id, agent_id }
   - Message::TaskProgress { completed, total }
   - Message::TaskCompleted { task_id, commit }
   - Message::TaskFailed { task_id, error }

4. **Wire SchedulerEvents to TEA Messages**
   - Bridge between SchedulerEvent and Message types

## Key Files to Modify
- `src/state/manager.rs` - Add save_task/load_task/list_tasks
- `src/orchestration/scheduler.rs` - Add progress tracking with GitStateManager
- `src/tea/message.rs` - Add task message variants

## Patterns to Follow
- Use git notes for task persistence (same pattern as workflows)
- UUID-based TaskId for namespace isolation
- Event-driven updates via mpsc channels
