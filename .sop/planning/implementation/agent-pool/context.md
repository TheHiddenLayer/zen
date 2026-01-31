# AgentPool Implementation Context

## Task Summary
Create AgentPool for multi-agent management in the orchestration layer.

## Requirements
1. Create `src/orchestration/pool.rs` with `AgentPool` struct
2. Implement pool operations: new, spawn (stub), terminate, get, active_count, has_capacity
3. Define `AgentEvent` enum for pool events

## Dependencies
- `AgentId`, `AgentStatus` from `src/agent.rs` (Task 4.1 - completed)
- `TaskId` from `src/workflow/types.rs`
- tokio mpsc channel
- `AgentHandle` from Task 4.3 (implement placeholder)

## Existing Patterns

### Module Organization
- Each feature gets its own file in the appropriate module directory
- Public exports via `mod.rs`
- Tests in `#[cfg(test)] mod tests` at the bottom of each file

### Type Patterns
- ID types use UUID newtype pattern: `pub struct AgentId(pub Uuid)`
- Status enums use variants with associated data
- Arc<RwLock<T>> for shared mutable state

### Testing Patterns
- Comprehensive unit tests for all public methods
- Test names follow `test_<function>_<scenario>` pattern
- Use assertions for all expected behaviors

## Implementation Details

### AgentHandle (Placeholder)
Task 4.3 will implement the full AgentHandle. For now, create a minimal placeholder:
```rust
pub struct AgentHandle {
    pub id: AgentId,
    pub status: AgentStatus,
}
```

### AgentPool Structure
```rust
pub struct AgentPool {
    agents: HashMap<AgentId, AgentHandle>,
    max_concurrent: usize,
    event_tx: mpsc::Sender<AgentEvent>,
}
```

### AgentEvent Enum
From design doc section 4.3:
```rust
pub enum AgentEvent {
    Started { agent_id: AgentId, task_id: TaskId },
    Completed { agent_id: AgentId, exit_code: i32 },
    Failed { agent_id: AgentId, error: String },
    StuckDetected { agent_id: AgentId, duration: Duration },
}
```

## Acceptance Criteria
1. Capacity Enforcement - has_capacity() returns false when max_concurrent reached
2. Agent Tracking - get(id) returns correct AgentHandle
3. Active Count - active_count() returns number of agents
4. Terminate Cleanup - terminate removes agent and decreases count
5. Event Emission - state changes send AgentEvents via event_tx
