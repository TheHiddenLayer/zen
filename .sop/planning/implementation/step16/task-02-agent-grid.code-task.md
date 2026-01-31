# Task: Create Multi-Agent Grid Display

## Description
Create a TUI component that displays multiple agents in a grid layout with status indicators, enabling monitoring of parallel execution.

## Background
During implementation phase, multiple agents run in parallel. The TUI needs to show all active agents with their status, current task, and activity indicators.

## Reference Documentation
**Required:**
- Design: .sop/planning/design/detailed-design.md (Section 4.8 render_agent_grid)

**Note:** You MUST read the detailed design document before beginning implementation.

## Technical Requirements
1. Add to `src/ui.rs`:
   - `render_agent_grid(f: &mut Frame, area: Rect, agents: &[AgentView])`
2. Define AgentView for render state:
   ```rust
   pub struct AgentView {
       pub id: AgentId,
       pub task_name: String,
       pub status: AgentStatus,
       pub elapsed: Duration,
       pub output_preview: String,  // Last few lines
   }
   ```
3. Grid layout: 2x2 or 3x2 depending on count
4. Status indicators: color-coded (green=running, yellow=stuck, red=failed)
5. Activity indicator (spinner or blink)
6. Add `agents: Vec<AgentView>` to RenderState

## Dependencies
- Existing ui.rs patterns
- AgentStatus from Step 4
- ratatui Layout for grid

## Implementation Approach
1. Define AgentView struct
2. Add agents to RenderState
3. Implement grid layout calculation
4. Implement single agent cell rendering
5. Add status color coding
6. Add activity indicators
7. Implement selection highlight (arrow keys to select)
8. Add tests for various agent counts

## Acceptance Criteria

1. **Grid Layout**
   - Given 4 active agents
   - When TUI renders
   - Then 2x2 grid is displayed

2. **Status Colors**
   - Given agent in Running status
   - When rendered
   - Then green indicator is shown

3. **Output Preview**
   - Given agent with recent output
   - When cell renders
   - Then last 3 lines of output are shown

4. **Selection**
   - Given arrow key navigation
   - When user selects agent 2
   - Then agent 2's cell is highlighted

5. **Elapsed Time**
   - Given agent running for 2 minutes
   - When rendered
   - Then "2m 30s" is displayed

## Metadata
- **Complexity**: Medium
- **Labels**: TUI, UI, Agent, Grid
- **Required Skills**: Rust, ratatui, layout
