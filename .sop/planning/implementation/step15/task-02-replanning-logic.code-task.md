# Task: Implement Replanning Logic

## Description
Implement the replanning logic that diffs the updated plan against current tasks and updates the DAG accordingly, without interrupting running agents.

## Background
When plan files change, the system needs to determine what's different: new tasks, removed tasks, or modified tasks. Running tasks continue, but the queue is updated for new work.

## Reference Documentation
**Required:**
- Design: .sop/planning/design/detailed-design.md (Section 4.5 Reactive Planner replan method)

**Note:** You MUST read the detailed design document before beginning implementation.

## Technical Requirements
1. Add to ReactivePlanner:
   - `on_plan_changed(&self, path: &Path) -> Result<()>`
   - `replan(&self) -> Result<()>`
   - `diff_tasks(&self, old: &[Task], new: &[Task]) -> TaskDiff`
2. TaskDiff structure:
   - added: Vec<Task>
   - removed: Vec<TaskId>
   - modified: Vec<Task>
3. Update DAG without interrupting running tasks
4. Notify user of plan changes

## Dependencies
- ReactivePlanner from task-01
- TaskDAG from Step 8
- CodeTask parser from Step 9

## Implementation Approach
1. Define TaskDiff struct
2. Implement diff_tasks() comparing task lists
3. Implement on_plan_changed() to re-parse plan
4. Implement replan() to update DAG:
   - Add new tasks
   - Mark removed tasks as cancelled
   - Keep running tasks unchanged
5. Emit events for TUI notification
6. Add tests for various diff scenarios

## Acceptance Criteria

1. **New Task Detection**
   - Given plan adds 2 new steps
   - When replan() runs
   - Then 2 tasks are added to DAG

2. **Removed Task Handling**
   - Given plan removes a pending task
   - When replan() runs
   - Then task is marked cancelled (not deleted)

3. **Running Task Protection**
   - Given a task is running
   - When plan changes
   - Then running task continues uninterrupted

4. **User Notification**
   - Given plan changes detected
   - When replan completes
   - Then ReplanTriggered event is emitted

5. **Modified Task**
   - Given task description changes in plan
   - When diff_tasks() runs
   - Then task appears in modified list

## Metadata
- **Complexity**: High
- **Labels**: Reactive, Planning, DAG, Diff
- **Required Skills**: Rust, diffing algorithms, state management
