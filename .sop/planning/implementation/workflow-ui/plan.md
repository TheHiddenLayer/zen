# Plan: Workflow UI Components

## Test Scenarios

### WorkflowView Tests
1. WorkflowView::new creates view from Workflow
2. WorkflowView with None workflow returns appropriate defaults
3. WorkflowView phase_progress calculates correctly (3/5 = 60%)

### RenderState Workflow Fields Tests
1. RenderState default has workflow: None
2. RenderState with workflow displays correctly

### render_workflow_header Tests
1. Active workflow shows name and status
2. No active workflow shows "No active workflow"
3. Different statuses display with correct styling

### render_phase_progress Tests
1. Phase indicator shows all 5 phases with current highlighted
2. Progress bar shows correct percentage
3. Phase transitions update display immediately

## Implementation Tasks

- [ ] Create WorkflowView struct in src/render.rs
- [ ] Add workflow fields to RenderState
- [ ] Write tests for WorkflowView
- [ ] Implement render_workflow_header() in src/ui.rs
- [ ] Implement render_phase_progress() in src/ui.rs
- [ ] Write tests for render functions
- [ ] Validate all tests pass
