# Progress Tracking Implementation Plan

## Test Scenarios

### GitStateManager Task Persistence Tests
1. save_task/load_task roundtrip - save and reload a task
2. save_task overwrites existing - update status and verify
3. load_task nonexistent returns None
4. list_tasks with multiple tasks
5. list_tasks empty returns empty vector

### Scheduler Progress Tracking Tests
1. Progress percentage calculation - 3/5 = 60%
2. Progress percentage with zero tasks
3. Emit TaskProgress event on completion
4. State manager called on task start
5. State manager called on task completion with commit

### TEA Message Tests
1. TaskStarted message structure
2. TaskProgress message structure
3. TaskCompleted message structure
4. TaskFailed message structure

## Implementation Steps

### Step 1: Add Task Persistence to GitStateManager
- Add save_task() method using git notes
- Add load_task() method
- Add list_tasks() method
- Use tasks/{id} namespace

### Step 2: Add Progress Tracking to Scheduler
- Add optional GitStateManager to Scheduler
- Add with_state_manager() builder method
- Add progress_percentage() method
- Update dispatch/handle methods to persist state
- Add TaskProgress event emission

### Step 3: Add TEA Message Variants
- Add Message::TaskStarted
- Add Message::TaskProgress
- Add Message::TaskCompleted
- Add Message::TaskFailed

## Acceptance Criteria Mapping

| Criteria | Test | Implementation |
|----------|------|----------------|
| Status Persistence | save_task on start | dispatch_ready_tasks |
| Progress Calculation | 3/5 = 60% | progress_percentage() |
| Completion Recording | commit_hash set | handle_completion |
| TEA Integration | Message variants | message.rs |
| Real-Time Updates | prompt emission | event_tx.send() |
