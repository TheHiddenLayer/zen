# Task: Create Workflow and WorkflowConfig Structs

## Description
Create the main Workflow struct and WorkflowConfig struct that hold all workflow data and configuration. The Workflow struct is the central data model for tracking orchestration runs.

## Background
Each workflow represents a complete orchestration run from user prompt to completion. The Workflow struct must track the original prompt, current phase, status, timestamps, and configuration options. WorkflowConfig controls behavior like documentation updates and parallelism limits.

## Reference Documentation
**Required:**
- Design: .sop/planning/design/detailed-design.md (Section 5.2 Workflow State JSON schema)

**Note:** You MUST read the detailed design document before beginning implementation. The JSON schema in Section 5.2 shows the expected structure.

## Technical Requirements
1. Create `WorkflowConfig` struct in `src/workflow/types.rs`:
   - `update_docs: bool` - whether to run /codebase-summary phase
   - `max_parallel_agents: usize` - limit on concurrent agents
   - `staging_branch_prefix: String` - prefix for staging branches
2. Create `Workflow` struct with fields:
   - `id: WorkflowId`
   - `name: String` - derived from prompt
   - `prompt: String` - original user input
   - `phase: WorkflowPhase`
   - `status: WorkflowStatus`
   - `config: WorkflowConfig`
   - `created_at: DateTime<Utc>`
   - `started_at: Option<DateTime<Utc>>`
   - `completed_at: Option<DateTime<Utc>>`
   - `task_ids: Vec<TaskId>` - placeholder, will be populated in Step 8
3. Implement `Default` for `WorkflowConfig` with sensible defaults
4. Implement builder pattern or constructor for `Workflow`

## Dependencies
- chrono (already in Cargo.toml)
- Types from task-01 (WorkflowId, WorkflowPhase, WorkflowStatus)

## Implementation Approach
1. Define `WorkflowConfig` with `Default` implementation
2. Define `Workflow` struct with all required fields
3. Implement `Workflow::new(prompt: &str, config: WorkflowConfig)` constructor
4. Implement helper methods: `start()`, `complete()`, `fail()`
5. Add serialization tests matching the JSON schema in design doc

## Acceptance Criteria

1. **Workflow Creation**
   - Given a user prompt "build user authentication"
   - When `Workflow::new(prompt, config)` is called
   - Then a workflow is created with generated id, name derived from prompt, and Pending status

2. **Default Configuration**
   - Given no custom configuration
   - When `WorkflowConfig::default()` is called
   - Then update_docs=true, max_parallel_agents=4, staging_branch_prefix="zen/staging/"

3. **Lifecycle Methods**
   - Given a pending workflow
   - When `workflow.start()` is called
   - Then status becomes Running and started_at is set to current time

4. **JSON Schema Compliance**
   - Given a workflow instance
   - When serialized to JSON
   - Then the output matches the schema in design doc Section 5.2

5. **Unit Test Coverage**
   - Given the Workflow implementation
   - When running tests
   - Then workflow creation, lifecycle transitions, and serialization tests pass

## Metadata
- **Complexity**: Low
- **Labels**: Foundation, Types, Workflow, Configuration
- **Required Skills**: Rust, serde, chrono, builder pattern
