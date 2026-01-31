# Context: Workflow and WorkflowConfig Structs

## Project Structure
- Rust project using Cargo
- Existing workflow module at `src/workflow/` with `mod.rs` and `types.rs`
- Uses `chrono` for DateTime handling with serde support
- Uses `uuid` for unique identifiers
- Uses `serde` for serialization

## Existing Types (from task-01)
- `WorkflowId` - UUID-based identifier with Display, FromStr, serde support
- `WorkflowPhase` - 6-variant enum (Planning, TaskGeneration, Implementation, Merging, Documentation, Complete)
- `WorkflowStatus` - 5-variant enum with Default (Pending, Running, Paused, Completed, Failed)

## Requirements

### WorkflowConfig
- `update_docs: bool` - whether to run /codebase-summary phase
- `max_parallel_agents: usize` - limit on concurrent agents
- `staging_branch_prefix: String` - prefix for staging branches
- Default: update_docs=true, max_parallel_agents=4, staging_branch_prefix="zen/staging/"

### Workflow
- `id: WorkflowId`
- `name: String` - derived from prompt
- `prompt: String` - original user input
- `phase: WorkflowPhase`
- `status: WorkflowStatus`
- `config: WorkflowConfig`
- `created_at: DateTime<Utc>`
- `started_at: Option<DateTime<Utc>>`
- `completed_at: Option<DateTime<Utc>>`
- `task_ids: Vec<TaskId>` - placeholder type

### Lifecycle Methods
- `Workflow::new(prompt, config)` - creates workflow with Pending status
- `workflow.start()` - sets status to Running, sets started_at
- `workflow.complete()` - sets status to Completed, sets completed_at
- `workflow.fail()` - sets status to Failed, sets completed_at

## JSON Schema Compliance (Section 5.2)
The serialized JSON must match:
```json
{
  "id": "wf-001",
  "name": "build-user-auth",
  "status": "running",
  "prompt": "Build user authentication...",
  "created_at": "2026-01-30T10:00:00Z",
  "started_at": "2026-01-30T10:00:05Z",
  "tasks": ["task-001", "task-002", "task-003"],
  ...
}
```

## Dependencies
- chrono (already in Cargo.toml with serde feature)
- uuid (already in Cargo.toml)
- serde (already in Cargo.toml)

## Implementation Paths
- Add types to `src/workflow/types.rs`
- Export from `src/workflow/mod.rs`
