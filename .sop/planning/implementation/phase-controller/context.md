# Context: PhaseController Implementation

## Task Overview
Implement the `PhaseController` that manages workflow phase transitions and emits events for TUI updates. This centralizes phase management logic.

## Requirements

### Technical Requirements
1. Create `PhaseController` struct with:
   - `current_phase: WorkflowPhase`
   - `phase_history: Vec<(WorkflowPhase, Instant)>`
   - `event_tx: mpsc::Sender<PhaseEvent>`

2. Implement methods:
   - `new(event_tx: Sender<PhaseEvent>) -> Self`
   - `transition(&mut self, phase: WorkflowPhase) -> Result<()>`
   - `current(&self) -> WorkflowPhase`
   - `elapsed(&self) -> Duration`

3. Define `PhaseEvent` enum for phase changes

## Existing Patterns

### WorkflowPhase (from `src/workflow/types.rs`)
```rust
pub enum WorkflowPhase {
    Planning,
    TaskGeneration,
    Implementation,
    Merging,
    Documentation,
    Complete,
}
```

### WorkflowState Transition Rules (from `src/workflow/state.rs`)
Valid transitions:
- Planning -> TaskGeneration
- TaskGeneration -> Implementation
- Implementation -> Merging
- Merging -> Documentation OR Complete
- Documentation -> Complete

Uses `Error::InvalidPhaseTransition { from, to }` for invalid transitions.

### Event Pattern (from `src/orchestration/pool.rs`)
```rust
pub enum AgentEvent {
    Started { agent_id: AgentId, task_id: TaskId },
    Completed { agent_id: AgentId, exit_code: i32 },
    Failed { agent_id: AgentId, error: String },
    StuckDetected { agent_id: AgentId },
    Terminated { agent_id: AgentId },
}
```

### Channel Pattern (from AgentPool)
```rust
let (event_tx, event_rx) = mpsc::channel(100);
```

## Implementation Location
File: `src/orchestration/skills.rs` (add to existing file)

## Dependencies
- `WorkflowPhase` from `crate::workflow::types`
- `Error::InvalidPhaseTransition` from `crate::error`
- `tokio::sync::mpsc`
- `std::time::{Duration, Instant}`

## Key Design Decisions
1. Use `Instant` for timing (cannot be serialized, but provides accurate elapsed time)
2. Emit events asynchronously via mpsc channel
3. Reuse transition validation logic from WorkflowState
4. PhaseEvent follows the same pattern as AgentEvent
