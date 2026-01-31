# Plan: Workflow Core Types Implementation

## Test Strategy

### Test Scenarios

#### WorkflowId Tests

| Test Case | Input | Expected Output |
|-----------|-------|-----------------|
| test_workflow_id_new | Call `WorkflowId::new()` twice | Two different UUIDs |
| test_workflow_id_default | Call `WorkflowId::default()` | Non-nil UUID |
| test_workflow_id_short | Any WorkflowId | 8-character string |
| test_workflow_id_display | Any WorkflowId | Full UUID string |
| test_workflow_id_from_str | Valid UUID string | Parsed WorkflowId |
| test_workflow_id_from_str_invalid | "invalid" | Error |
| test_workflow_id_serialization | Any WorkflowId | JSON round-trip preserves value |
| test_workflow_id_equality | Two WorkflowIds with same UUID | Equal |
| test_workflow_id_hash | Two equal WorkflowIds | Same hash |

#### WorkflowPhase Tests

| Test Case | Input | Expected Output |
|-----------|-------|-----------------|
| test_workflow_phase_ordering | All phases | Planning < TaskGeneration < Implementation < Merging < Documentation < Complete |
| test_workflow_phase_display_planning | Planning | "planning" |
| test_workflow_phase_display_task_generation | TaskGeneration | "task_generation" |
| test_workflow_phase_display_implementation | Implementation | "implementation" |
| test_workflow_phase_display_merging | Merging | "merging" |
| test_workflow_phase_display_documentation | Documentation | "documentation" |
| test_workflow_phase_display_complete | Complete | "complete" |
| test_workflow_phase_serialization | All phases | JSON round-trip preserves values |
| test_workflow_phase_equality | Two same phases | Equal |

#### WorkflowStatus Tests

| Test Case | Input | Expected Output |
|-----------|-------|-----------------|
| test_workflow_status_display_pending | Pending | "pending" |
| test_workflow_status_display_running | Running | "running" |
| test_workflow_status_display_paused | Paused | "paused" |
| test_workflow_status_display_completed | Completed | "completed" |
| test_workflow_status_display_failed | Failed | "failed" |
| test_workflow_status_serialization | All statuses | JSON round-trip preserves values |
| test_workflow_status_default | Default | Pending |

## Implementation Plan

### Phase 1: Create Module Structure

1. Create `src/workflow/` directory
2. Create `src/workflow/mod.rs` with module exports
3. Create `src/workflow/types.rs` empty file
4. Update `src/lib.rs` to include workflow module

### Phase 2: Implement WorkflowId (TDD)

1. Write tests for WorkflowId
2. Implement WorkflowId struct with:
   - `#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]`
   - `#[serde(transparent)]`
   - `new()`, `short()` methods
   - `Default`, `Display`, `FromStr` trait implementations

### Phase 3: Implement WorkflowPhase (TDD)

1. Write tests for WorkflowPhase
2. Implement WorkflowPhase enum with:
   - `#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]`
   - `#[serde(rename_all = "snake_case")]`
   - Variants: Planning, TaskGeneration, Implementation, Merging, Documentation, Complete
   - `Display` trait implementation

### Phase 4: Implement WorkflowStatus (TDD)

1. Write tests for WorkflowStatus
2. Implement WorkflowStatus enum with:
   - `#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]`
   - `#[serde(rename_all = "snake_case")]`
   - Variants: Pending (default), Running, Paused, Completed, Failed
   - `Display` trait implementation

### Phase 5: Validation

1. Run `cargo test workflow` to verify all tests pass
2. Run `cargo build` to verify compilation
3. Run `cargo clippy` to check for lints

## Implementation Checklist

- [ ] Create src/workflow/mod.rs
- [ ] Create src/workflow/types.rs
- [ ] Update src/lib.rs
- [ ] Implement WorkflowId with tests
- [ ] Implement WorkflowPhase with tests
- [ ] Implement WorkflowStatus with tests
- [ ] All tests pass
- [ ] Build succeeds
- [ ] Changes committed
