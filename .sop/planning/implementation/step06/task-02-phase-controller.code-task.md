# Task: Implement PhaseController

## Description
Create the PhaseController that manages workflow phase transitions and emits events for TUI updates. This centralizes phase management logic.

## Background
The workflow progresses through 5 phases. PhaseController validates transitions, tracks timing, and emits events that the TUI can consume to show real-time progress.

## Reference Documentation
**Required:**
- Design: .sop/planning/design/detailed-design.md (Section 4.2 phase_controller mentions, Section 1.5 phase diagram)

**Note:** You MUST read the detailed design document before beginning implementation.

## Technical Requirements
1. Create `PhaseController` in `src/orchestration/skills.rs`:
   ```rust
   pub struct PhaseController {
       current_phase: WorkflowPhase,
       phase_history: Vec<(WorkflowPhase, Instant)>,
       event_tx: mpsc::Sender<PhaseEvent>,
   }
   ```
2. Implement methods:
   - `new(event_tx: Sender<PhaseEvent>) -> Self`
   - `transition(&mut self, phase: WorkflowPhase) -> Result<()>`
   - `current(&self) -> WorkflowPhase`
   - `elapsed(&self) -> Duration`
3. Define `PhaseEvent` enum for phase changes

## Dependencies
- WorkflowPhase from Step 1
- tokio mpsc channel

## Implementation Approach
1. Define PhaseEvent enum (Started, Completed, Failed)
2. Create PhaseController struct
3. Implement transition() with validation from WorkflowState
4. Track timing for each phase
5. Emit events on transitions
6. Add tests for valid/invalid transitions

## Acceptance Criteria

1. **Valid Transition**
   - Given controller in Planning phase
   - When transition(TaskGeneration) is called
   - Then phase changes and event is emitted

2. **Invalid Transition Rejected**
   - Given controller in Planning phase
   - When transition(Merging) is called
   - Then error is returned and phase unchanged

3. **Phase Timing**
   - Given controller has been in current phase for 30 seconds
   - When elapsed() is called
   - Then ~30 seconds is returned

4. **Event Emission**
   - Given event receiver is listening
   - When transition occurs
   - Then PhaseEvent is received with correct phase

5. **History Tracking**
   - Given multiple phase transitions
   - When history is examined
   - Then all transitions with timestamps are recorded

## Metadata
- **Complexity**: Low
- **Labels**: Orchestration, Phase, Events, State
- **Required Skills**: Rust, state management, channels
