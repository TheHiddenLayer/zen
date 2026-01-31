# Task Model Implementation Context

## Requirements

### Functional Requirements
1. Create TaskId newtype with UUID-based identifier
2. Create TaskStatus enum with 6 variants (Pending, Ready, Running, Completed, Failed, Blocked)
3. Create Task struct with execution context, timing, and result fields
4. Implement lifecycle methods (new, start, complete, fail)
5. Implement serialization matching Section 5.3 JSON schema

### Acceptance Criteria
1. Task::new(name, description) creates task with Pending status
2. start() then complete() transitions status correctly with timestamps
3. fail(error) sets status to Failed with error message
4. JSON serialization matches design spec format
5. Project compiles with `pub mod core;` in lib.rs

## Existing Patterns

### Newtype Pattern (from workflow/types.rs and agent.rs)
```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct TypeId(pub Uuid);

impl TypeId {
    pub fn new() -> Self { Self(Uuid::new_v4()) }
    pub fn short(&self) -> String { self.0.to_string()[..8].to_string() }
}

impl Default for TypeId { fn default() -> Self { Self::new() } }
impl std::fmt::Display for TypeId { ... }
impl std::str::FromStr for TypeId { ... }
```

### Existing TaskId Placeholder
There's already a `TaskId` in `src/workflow/types.rs` (lines 119-140). The new core::TaskId will be the full implementation, and we need to decide whether to:
- Replace the placeholder with the full implementation
- Keep both and re-export from core

**Decision:** Create full TaskId in core module, as the design spec puts it under src/core/task.rs. The workflow module can import from core.

### AgentId Reference
AgentId is in `src/agent.rs` - we need to import it for the Task struct's agent_id field.

## JSON Schema (Section 5.3)
```json
{
  "id": "task-001",
  "workflow_id": "wf-001",
  "name": "create-user-model",
  "description": "Create User model with email, password hash, and timestamps",
  "status": "completed",
  "agent_id": "agent-abc123",
  "worktree_path": "/Users/alice/.zen/worktrees/task-001",
  "branch_name": "zen/task/task-001",
  "created_at": "2026-01-30T10:00:05Z",
  "started_at": "2026-01-30T10:00:10Z",
  "completed_at": "2026-01-30T10:05:30Z",
  "commit_hash": "a1b2c3d4e5f6",
  "exit_code": 0,
  "retries": 0
}
```

Note: Task struct in design doesn't include workflow_id, exit_code, or retries. We'll follow the struct definition from Section 4.3, not the JSON schema extras.

## Implementation Path
- Create `src/core/mod.rs`
- Create `src/core/task.rs`
- Add `pub mod core;` to `src/lib.rs`

## Dependencies
- uuid (already in Cargo.toml with v4 and serde features)
- chrono (already in Cargo.toml with serde feature)
- serde (already in Cargo.toml)
- AgentId from crate::agent
- PathBuf from std::path
