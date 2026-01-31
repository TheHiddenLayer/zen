# Task: Create Workflow UI Components

## Description
Create TUI components that display workflow phase progress and overall status, including a phase indicator and progress bar.

## Background
The TUI needs to show which phase the workflow is in (PDD, TaskGen, Implementation, etc.) and overall progress. This gives users visibility into autonomous execution.

## Reference Documentation
**Required:**
- Design: .sop/planning/design/detailed-design.md (Section 4.8 TUI Components)
- Research: .sop/planning/research/existing-code.md (src/ui.rs section)

**Note:** You MUST read both documents to understand existing TUI patterns.

## Technical Requirements
1. Update `src/ui.rs` with new components:
   - `render_workflow_header(f: &mut Frame, area: Rect, workflow: &WorkflowView)`
   - `render_phase_progress(f: &mut Frame, area: Rect, phase: WorkflowPhase)`
2. Add workflow fields to RenderState:
   - `workflow: Option<WorkflowView>`
   - `phase: Option<WorkflowPhase>`
   - `phase_progress: (usize, usize)` // (completed_phases, total_phases)
3. Design clear phase indicator (5 phases with current highlighted)
4. Add progress bar for overall workflow

## Dependencies
- Existing ui.rs and render.rs
- ratatui widgets (Gauge, List, Block)
- WorkflowPhase from Step 1

## Implementation Approach
1. Define WorkflowView struct for render state
2. Add workflow fields to RenderState
3. Implement render_workflow_header() with title and status
4. Implement render_phase_progress() with 5-phase indicator
5. Use Gauge widget for progress bar
6. Wire up to main render function
7. Add visual tests

## Acceptance Criteria

1. **Phase Display**
   - Given workflow in Implementation phase
   - When TUI renders
   - Then Implementation is highlighted in phase list

2. **Progress Bar**
   - Given 3 of 5 phases complete
   - When TUI renders
   - Then progress bar shows 60%

3. **Workflow Header**
   - Given active workflow "build auth"
   - When TUI renders
   - Then header shows workflow name and status

4. **No Workflow State**
   - Given no active workflow
   - When TUI renders
   - Then workflow section shows "No active workflow"

5. **Phase Transition Update**
   - Given phase changes from TaskGen to Implementation
   - When RenderState updates
   - Then TUI reflects new phase immediately

## Metadata
- **Complexity**: Medium
- **Labels**: TUI, UI, Workflow, Progress
- **Required Skills**: Rust, ratatui, UI design
