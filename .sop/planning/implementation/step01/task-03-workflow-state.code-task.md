# Task: Implement WorkflowState with Phase Transitions

## Description
Create the WorkflowState struct that manages runtime workflow state and enforces valid phase transitions. This ensures workflows progress through phases in the correct order.

## Background
The Skills-driven workflow has a strict phase order: Planning -> TaskGeneration -> Implementation -> Merging -> Documentation -> Complete. The WorkflowState must enforce this ordering and prevent invalid transitions (e.g., jumping from Planning to Merging).

## Reference Documentation
**Required:**
- Design: .sop/planning/design/detailed-design.md (Section 1.5 Skills-Driven Workflow, Section 4.2 WorkflowPhase enum)

**Note:** You MUST read the detailed design document before beginning implementation. The phase diagram in Section 1.5 shows the required ordering.

## Technical Requirements
1. Create `src/workflow/state.rs` with `WorkflowState` struct
2. Implement phase transition validation:
   - Planning -> TaskGeneration (only valid transition from Planning)
   - TaskGeneration -> Implementation
   - Implementation -> Merging
   - Merging -> Documentation OR Complete (Documentation is optional)
   - Documentation -> Complete
3. Track phase history for debugging/logging
4. Emit phase change notifications (prepare for TEA integration)
5. Wire up module in `src/lib.rs`: add `pub mod workflow;`

## Dependencies
- Types from task-01 and task-02 (WorkflowId, WorkflowPhase, Workflow)

## Implementation Approach
1. Create `WorkflowState` holding current `Workflow` and phase history
2. Implement `transition(&mut self, target: WorkflowPhase) -> Result<()>`
3. Add `can_transition(&self, target: WorkflowPhase) -> bool` helper
4. Implement `current_phase(&self) -> WorkflowPhase`
5. Add `pub mod workflow;` to `src/lib.rs`
6. Add comprehensive tests for valid and invalid transitions

## Acceptance Criteria

1. **Valid Forward Transition**
   - Given a workflow in Planning phase
   - When `state.transition(TaskGeneration)` is called
   - Then the transition succeeds and phase is updated

2. **Invalid Skip Transition**
   - Given a workflow in Planning phase
   - When `state.transition(Merging)` is called
   - Then an error is returned indicating invalid transition

3. **Optional Documentation Phase**
   - Given a workflow in Merging phase
   - When `state.transition(Complete)` is called (skipping Documentation)
   - Then the transition succeeds (Documentation is optional)

4. **Phase History Tracking**
   - Given a workflow that has transitioned through multiple phases
   - When `state.phase_history()` is called
   - Then all previous phases are returned in order

5. **Module Integration**
   - Given the workflow module is complete
   - When `cargo build` is run
   - Then the project compiles with `pub mod workflow;` in lib.rs

6. **Unit Test Coverage**
   - Given the WorkflowState implementation
   - When running `cargo test workflow`
   - Then all transition validation tests pass (valid and invalid cases)

## Metadata
- **Complexity**: Medium
- **Labels**: Foundation, State Management, Workflow, Validation
- **Required Skills**: Rust, state machines, error handling
