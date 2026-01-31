# Task: Create DAG Visualization

## Description
Create an ASCII-based DAG visualization that shows task dependencies and completion status in the TUI.

## Background
Users benefit from seeing the task dependency graph to understand execution order and identify bottlenecks. The visualization shows which tasks are complete, running, or waiting.

## Reference Documentation
**Required:**
- Design: .sop/planning/design/detailed-design.md (Section 4.8 render_task_dag)

**Note:** You MUST read the detailed design document before beginning implementation.

## Technical Requirements
1. Add to `src/ui.rs`:
   - `render_task_dag(f: &mut Frame, area: Rect, dag: &TaskDAGView)`
2. Define TaskDAGView:
   ```rust
   pub struct TaskDAGView {
       pub tasks: Vec<TaskView>,
       pub edges: Vec<(usize, usize)>,  // (from_idx, to_idx)
   }
   pub struct TaskView {
       pub name: String,
       pub status: TaskStatus,
   }
   ```
3. ASCII box layout with arrows:
   ```
   [A:done] ──┐
              ├──> [C:running]
   [B:done] ──┘

   [D:pending] ──> [E:pending]
   ```
4. Status colors for boxes
5. Toggle with 'd' key to show/hide

## Dependencies
- TaskDAG from Step 8
- RenderState updates
- ratatui Paragraph for ASCII art

## Implementation Approach
1. Define TaskDAGView and TaskView structs
2. Implement DAG-to-ASCII conversion
3. Calculate box positions based on topological order
4. Draw edges with ASCII arrows
5. Apply status colors
6. Add toggle mode for DAG view
7. Add tests with sample DAGs

## Acceptance Criteria

1. **Task Boxes**
   - Given 5 tasks in DAG
   - When rendered
   - Then 5 ASCII boxes are displayed with task names

2. **Dependency Arrows**
   - Given A->C dependency
   - When rendered
   - Then arrow connects A box to C box

3. **Status Colors**
   - Given completed task A
   - When rendered
   - Then A's box is green

4. **Toggle Mode**
   - Given user presses 'd'
   - When in dashboard mode
   - Then DAG view is shown/hidden

5. **Complex DAG**
   - Given DAG with 10 tasks and multiple paths
   - When rendered
   - Then all dependencies are visible (may scroll)

## Metadata
- **Complexity**: High
- **Labels**: TUI, DAG, Visualization, ASCII
- **Required Skills**: Rust, ASCII art, graph layout
