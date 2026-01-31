# Plan: WorkflowState with Phase Transitions

## Test Strategy

### Test Scenarios

#### 1. Valid Forward Transitions
- Planning -> TaskGeneration: should succeed
- TaskGeneration -> Implementation: should succeed
- Implementation -> Merging: should succeed
- Merging -> Documentation: should succeed
- Documentation -> Complete: should succeed

#### 2. Optional Documentation Phase
- Merging -> Complete: should succeed (skip Documentation)

#### 3. Invalid Skip Transitions
- Planning -> Implementation: should fail
- Planning -> Merging: should fail
- Planning -> Documentation: should fail
- Planning -> Complete: should fail
- TaskGeneration -> Merging: should fail
- TaskGeneration -> Documentation: should fail
- TaskGeneration -> Complete: should fail
- Implementation -> Documentation: should fail
- Implementation -> Complete: should fail
- Documentation -> Planning: should fail (backward)

#### 4. Backward Transitions (all invalid)
- TaskGeneration -> Planning: should fail
- Implementation -> TaskGeneration: should fail
- Merging -> Implementation: should fail
- Complete -> any: should fail

#### 5. Same Phase Transition
- Planning -> Planning: should fail (no-op is invalid)

#### 6. Phase History Tracking
- Track all phases visited in order
- Include timestamps for each entry
- Provide ordered access to history

#### 7. can_transition Helper
- Returns true for valid transitions
- Returns false for invalid transitions

#### 8. current_phase Access
- Returns current workflow phase
- Reflects state after transitions

#### 9. WorkflowState Construction
- From Workflow instance
- Initializes with workflow's current phase in history

## Implementation Plan

### Step 1: Add Error Variant
Add `InvalidPhaseTransition` to `src/error.rs`

### Step 2: Create state.rs Module
File: `src/workflow/state.rs`

Structs:
- `PhaseHistoryEntry`: phase, timestamp
- `WorkflowState`: workflow, phase_history

Methods:
- `new(workflow: Workflow) -> Self`
- `can_transition(&self, target: WorkflowPhase) -> bool`
- `transition(&mut self, target: WorkflowPhase) -> Result<()>`
- `current_phase(&self) -> WorkflowPhase`
- `phase_history(&self) -> &[PhaseHistoryEntry]`

### Step 3: Wire Up Module
Update `src/workflow/mod.rs` to include `mod state;` and re-exports

### Step 4: Tests
Comprehensive tests covering all scenarios above
