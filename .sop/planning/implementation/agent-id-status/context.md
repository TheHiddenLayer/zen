# Context: AgentId and AgentStatus Types

## Task Overview
Add AgentId (UUID-based identifier) and AgentStatus enum to support multiple concurrent agents with lifecycle tracking.

## Requirements
1. **AgentId** - UUID-based newtype (similar to SessionId, WorkflowId)
2. **AgentStatus** enum with variants:
   - `Idle` - Agent not working on any task
   - `Running { task_id: TaskId }` - Agent executing a task
   - `Stuck { since: Instant, reason: String }` - Agent detected as stuck
   - `Failed { error: String }` - Agent failed
   - `Terminated` - Agent was terminated
3. Display implementation for AgentStatus
4. Serialization support for persistence

## Existing Patterns

### SessionId (src/session.rs)
```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SessionId(pub Uuid);

impl SessionId {
    pub fn new() -> Self { Self(Uuid::new_v4()) }
    pub fn short(&self) -> String { self.0.to_string()[..8].to_string() }
}
```

### WorkflowId (src/workflow/types.rs)
Same pattern with additional FromStr and Display implementations.

### TaskId (src/workflow/types.rs)
Placeholder type already exists - will be used for Running variant.

## Implementation Location
- File: `src/agent.rs`
- Keep existing Agent struct unchanged (it's the config)
- Add new types alongside

## Dependencies
- `uuid` crate (already available)
- `serde` for serialization
- `std::time::Instant` for Stuck variant
- `TaskId` from workflow::types

## Notes
- Instant cannot be serialized directly - need custom serialization or store as Duration
- AgentStatus with Stuck variant will need special handling for serde
