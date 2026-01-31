# Context: WorkflowState with Phase Transitions

## Project Structure

- **Language**: Rust
- **Build System**: Cargo
- **Crate Root**: `src/lib.rs`
- **Workflow Module**: `src/workflow/`

## Requirements

### Functional Requirements

1. Create `WorkflowState` struct in `src/workflow/state.rs`
2. Hold current `Workflow` and track phase history
3. Implement phase transition validation with these rules:
   - Planning -> TaskGeneration (only valid from Planning)
   - TaskGeneration -> Implementation
   - Implementation -> Merging
   - Merging -> Documentation OR Complete (Documentation is optional)
   - Documentation -> Complete
4. Provide phase history for debugging/logging
5. Prepare for TEA integration (phase change notifications)

### Acceptance Criteria

1. Valid forward transitions succeed
2. Invalid skip transitions return error
3. Optional Documentation phase (can skip from Merging to Complete)
4. Phase history tracking works
5. Module integration compiles
6. All tests pass

## Existing Patterns

### Error Handling

From `src/error.rs`:
- Uses `thiserror::Error` derive macro
- Enum variants with `#[error("...")]` attributes
- `Result<T>` type alias for `std::result::Result<T, Error>`

### Workflow Types (from `src/workflow/types.rs`)

- `WorkflowId`: UUID-based identifier
- `WorkflowPhase`: 6 variants (Planning, TaskGeneration, Implementation, Merging, Documentation, Complete)
- `WorkflowStatus`: 5 variants (Pending, Running, Paused, Completed, Failed)
- `Workflow`: Main struct with id, name, prompt, phase, status, config, timestamps, task_ids
- Uses `serde::{Serialize, Deserialize}` for serialization
- Uses `chrono::{DateTime, Utc}` for timestamps

### Module Organization

From `src/workflow/mod.rs`:
- Module doc comment with `//!`
- `mod types;` for submodule
- `pub use types::{...}` for re-exports

## Dependencies

- Existing types: `Workflow`, `WorkflowId`, `WorkflowPhase` from `types.rs`
- Error type: Will add new error variant for invalid transitions
- Chrono: For timestamps in phase history

## Implementation Path

1. Add `InvalidPhaseTransition` error variant to `src/error.rs`
2. Create `src/workflow/state.rs` with:
   - `PhaseHistoryEntry` struct for tracking
   - `WorkflowState` struct
   - `can_transition()` helper
   - `transition()` method
   - `current_phase()` method
   - `phase_history()` method
3. Update `src/workflow/mod.rs` to include state module
4. Tests in `src/workflow/state.rs` following existing patterns
