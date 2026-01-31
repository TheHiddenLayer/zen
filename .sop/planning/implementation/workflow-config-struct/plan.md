# Implementation Plan: Workflow and WorkflowConfig Structs

## Test Strategy

### WorkflowConfig Tests
1. `test_workflow_config_default` - verify default values (update_docs=true, max_parallel_agents=4, staging_branch_prefix="zen/staging/")
2. `test_workflow_config_serialization` - verify JSON round-trip
3. `test_workflow_config_custom_values` - verify custom configuration works

### TaskId Tests (Placeholder type)
1. `test_task_id_new` - verify unique ID generation
2. `test_task_id_serialization` - verify JSON round-trip

### Workflow Tests
1. `test_workflow_new` - verify creation with Pending status, generated id, name from prompt
2. `test_workflow_name_derivation` - verify name is derived from prompt (first few words)
3. `test_workflow_start` - verify status becomes Running, started_at is set
4. `test_workflow_complete` - verify status becomes Completed, completed_at is set
5. `test_workflow_fail` - verify status becomes Failed, completed_at is set
6. `test_workflow_serialization` - verify JSON matches schema from design doc
7. `test_workflow_with_custom_config` - verify workflow uses provided config

## Implementation Steps

### Step 1: Add TaskId placeholder type
- Simple UUID-based type like WorkflowId
- Needed for task_ids field in Workflow

### Step 2: Implement WorkflowConfig
- Define struct with 3 fields
- Implement Default trait
- Derive Serialize, Deserialize, Clone, Debug

### Step 3: Implement Workflow struct
- Define struct with all required fields
- Implement constructor `new(prompt, config)`
- Implement lifecycle methods: start(), complete(), fail()
- Helper to derive name from prompt

### Step 4: Update exports in mod.rs
- Export TaskId, WorkflowConfig, Workflow

## Acceptance Criteria Mapping
- AC1 (Workflow Creation) -> test_workflow_new
- AC2 (Default Configuration) -> test_workflow_config_default
- AC3 (Lifecycle Methods) -> test_workflow_start, test_workflow_complete, test_workflow_fail
- AC4 (JSON Schema Compliance) -> test_workflow_serialization
- AC5 (Unit Test Coverage) -> all tests pass
