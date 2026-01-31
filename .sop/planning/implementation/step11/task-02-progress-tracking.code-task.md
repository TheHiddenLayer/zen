# Task: Implement Progress Tracking for Implementation Phase

## Description
Add progress tracking to the implementation phase that updates task status in GitStateManager and emits events for TUI display.

## Background
Users need to see real-time progress during the implementation phase. This includes which tasks are running, completed, or failed, plus overall progress percentage.

## Reference Documentation
**Required:**
- Design: .sop/planning/design/detailed-design.md (Section 4.7 GitStateManager task operations)

**Note:** You MUST read the detailed design document before beginning implementation.

## Technical Requirements
1. Track progress in Scheduler:
   - Task started -> update status, emit event
   - Task completed -> capture commit, update status, emit event
   - Task failed -> record error, emit event
2. Persist status changes via GitStateManager
3. Calculate progress: completed / total tasks
4. Wire up TEA messages for TUI updates:
   - `Message::TaskStarted { task_id, agent_id }`
   - `Message::TaskProgress { completed, total }`
   - `Message::TaskCompleted { task_id, commit }`
   - `Message::TaskFailed { task_id, error }`

## Dependencies
- Scheduler from Step 10
- GitStateManager from Step 2
- TEA message system

## Implementation Approach
1. Add progress tracking to Scheduler
2. Implement status persistence via GitStateManager.save_task()
3. Calculate and emit progress percentage
4. Add TEA Message variants for task updates
5. Wire events to message channel
6. Add tests for progress tracking

## Acceptance Criteria

1. **Status Persistence**
   - Given a task starts
   - When Scheduler processes start
   - Then GitStateManager.save_task() is called with Running status

2. **Progress Calculation**
   - Given 3 of 5 tasks complete
   - When progress is calculated
   - Then 60% is reported

3. **Completion Recording**
   - Given a task completes with commit abc123
   - When completion is processed
   - Then task.commit_hash is set and persisted

4. **TEA Integration**
   - Given task state changes
   - When events are processed
   - Then appropriate Message variants are sent

5. **Real-Time Updates**
   - Given implementation phase is running
   - When tasks complete
   - Then progress updates are emitted promptly

## Metadata
- **Complexity**: Medium
- **Labels**: Progress, Tracking, Events, TEA
- **Required Skills**: Rust, state management, event-driven
