# Task Model Implementation Plan

## Test Scenarios

### TaskId Tests
1. `test_task_id_new` - new() creates unique IDs
2. `test_task_id_default` - default() creates non-nil UUID
3. `test_task_id_short` - short() returns 8 characters
4. `test_task_id_display` - Display trait shows full UUID
5. `test_task_id_from_str` - FromStr parses valid UUID
6. `test_task_id_from_str_invalid` - FromStr errors on invalid input
7. `test_task_id_serialization` - JSON round-trip works
8. `test_task_id_equality` - same UUID means equal
9. `test_task_id_hash` - works in HashSet

### TaskStatus Tests
1. `test_task_status_default` - defaults to Pending
2. `test_task_status_display_*` - Display for each variant
3. `test_task_status_serialization` - JSON serialization format
4. `test_task_status_failed_error` - Failed variant stores error
5. `test_task_status_blocked_reason` - Blocked variant stores reason

### Task Tests
1. `test_task_new` - creates with Pending status, generated ID, timestamps
2. `test_task_start` - transitions to Running, sets started_at
3. `test_task_complete` - transitions to Completed, sets completed_at
4. `test_task_fail` - transitions to Failed with error, sets completed_at
5. `test_task_lifecycle_pending_to_running_to_completed` - full happy path
6. `test_task_lifecycle_pending_to_running_to_failed` - failure path
7. `test_task_serialization_json_format` - matches Section 5.3 schema
8. `test_task_with_agent_assignment` - agent_id field works
9. `test_task_with_worktree` - worktree_path and branch_name work
10. `test_task_with_commit` - commit_hash field works

## Implementation Tasks

- [ ] Create src/core/mod.rs with module export
- [ ] Create src/core/task.rs with TaskId newtype
- [ ] Add TaskStatus enum with all variants
- [ ] Add Task struct with all fields
- [ ] Implement Task::new()
- [ ] Implement Task::start()
- [ ] Implement Task::complete()
- [ ] Implement Task::fail()
- [ ] Add Serialize/Deserialize derives
- [ ] Add Display implementations
- [ ] Add tests
- [ ] Add pub mod core to lib.rs
- [ ] Run cargo test and verify all pass
- [ ] Run cargo build and verify compilation
