# Implementation Plan: PhaseController

## Test Scenarios

### 1. Valid Transition
- **Given:** Controller in Planning phase
- **When:** `transition(TaskGeneration)` is called
- **Then:** Phase changes and PhaseEvent::PhaseChanged is emitted

### 2. Invalid Transition Rejected
- **Given:** Controller in Planning phase
- **When:** `transition(Merging)` is called
- **Then:** Error returned, phase unchanged, no event emitted

### 3. Phase Timing
- **Given:** Controller has been in current phase for some duration
- **When:** `elapsed()` is called
- **Then:** Returns Duration since phase was entered

### 4. Event Emission
- **Given:** Event receiver is listening
- **When:** Transition occurs
- **Then:** PhaseEvent received with correct from/to phases and timestamp

### 5. History Tracking
- **Given:** Multiple phase transitions occur
- **When:** History is examined
- **Then:** All transitions with timestamps are recorded

### 6. Initial State
- **Given:** New PhaseController created
- **When:** Checking current phase
- **Then:** Returns Planning phase

## Implementation Tasks

- [ ] Define `PhaseEvent` enum with Started, Changed, Completed variants
- [ ] Create `PhaseController` struct with required fields
- [ ] Implement `new()` constructor
- [ ] Implement `current()` accessor
- [ ] Implement `elapsed()` duration calculation
- [ ] Implement `transition()` with validation and event emission
- [ ] Add `history()` accessor method
- [ ] Write tests for all acceptance criteria
- [ ] Run `cargo test` to verify

## PhaseEvent Design

```rust
pub enum PhaseEvent {
    /// Phase transition started
    Started {
        phase: WorkflowPhase,
        timestamp: Instant,
    },
    /// Phase transition completed
    Changed {
        from: WorkflowPhase,
        to: WorkflowPhase,
        elapsed: Duration,
    },
    /// Workflow completed
    Completed {
        total_duration: Duration,
    },
}
```

## PhaseController Design

```rust
pub struct PhaseController {
    current_phase: WorkflowPhase,
    phase_history: Vec<(WorkflowPhase, Instant)>,
    event_tx: mpsc::Sender<PhaseEvent>,
}
```
