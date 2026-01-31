# Context: Workflow UI Components

## Project Structure
- TUI rendering: `src/ui.rs` - main render functions
- Render state: `src/render.rs` - RenderState struct and SessionView
- Workflow types: `src/workflow/types.rs` - WorkflowPhase, WorkflowStatus, Workflow

## Requirements
1. Create WorkflowView struct for render state
2. Add workflow fields to RenderState (workflow, phase, phase_progress)
3. Implement render_workflow_header() with title and status
4. Implement render_phase_progress() with 5-phase indicator
5. Use Gauge widget for progress bar

## Existing Patterns
- RenderState is an immutable snapshot for rendering
- SessionView provides a view struct for sessions
- Color tokens: COLOR_TEXT_DIMMED (Gray), COLOR_TEXT_MUTED (DarkGray)
- Typography: bold headers, dimmed secondary info
- render_* functions take Frame, RenderState, and Rect

## Implementation Paths
- src/render.rs: Add WorkflowView struct and extend RenderState
- src/ui.rs: Add render_workflow_header() and render_phase_progress()

## Dependencies
- WorkflowPhase from src/workflow/types.rs
- WorkflowStatus from src/workflow/types.rs
- ratatui widgets (Gauge, Paragraph, Line, Span)
