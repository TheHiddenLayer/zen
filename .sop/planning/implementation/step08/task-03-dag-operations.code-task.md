# Task: Implement DAG Scheduling Operations

## Description
Implement the DAG operations needed for parallel scheduling: finding ready tasks, marking completion, and checking overall completion status.

## Background
The scheduler needs to know which tasks can run (dependencies satisfied), track completions, and know when all tasks are done. These operations must be efficient as they're called frequently during execution.

## Reference Documentation
**Required:**
- Design: .sop/planning/design/detailed-design.md (Section 4.3 TaskDAG operations, Section 6.2 Scheduler pseudocode)

**Note:** You MUST read the detailed design document before beginning implementation.

## Technical Requirements
1. Add to TaskDAG:
   - `ready_tasks(&self, completed: &HashSet<TaskId>) -> Vec<&Task>`
   - `complete_task(&mut self, id: &TaskId) -> Result<()>`
   - `all_complete(&self, completed: &HashSet<TaskId>) -> bool`
   - `topological_order(&self) -> Result<Vec<&Task>>`
   - `task_count(&self) -> usize`
   - `pending_count(&self, completed: &HashSet<TaskId>) -> usize`
2. ready_tasks returns tasks where all dependencies are in completed set
3. topological_order uses petgraph's toposort
4. Optimize for frequent calls (cache if needed)

## Dependencies
- TaskDAG from task-02
- HashSet for completed tracking

## Implementation Approach
1. Implement ready_tasks() checking incoming edges
2. Implement complete_task() updating task status
3. Implement all_complete() checking all nodes
4. Implement topological_order() using petgraph::algo::toposort
5. Add helper methods for counts
6. Add comprehensive tests with complex DAGs

## Acceptance Criteria

1. **Ready Tasks Identification**
   - Given DAG: A->C, B->C, D (independent)
   - When ready_tasks({}) is called (nothing complete)
   - Then [A, B, D] are returned (no dependencies)

2. **Ready After Completion**
   - Given DAG: A->B->C
   - When ready_tasks({A}) is called (A complete)
   - Then [B] is returned

3. **All Complete Check**
   - Given DAG with 5 tasks
   - When all_complete({all 5 ids}) is called
   - Then true is returned

4. **Topological Order**
   - Given DAG: A->C, B->C
   - When topological_order() is called
   - Then A and B come before C

5. **Cycle Prevention in toposort**
   - Given DAG has no cycles (enforced by add_dependency)
   - When topological_order() is called
   - Then valid order is always returned

## Metadata
- **Complexity**: Medium
- **Labels**: Core, DAG, Scheduling, Algorithm
- **Required Skills**: Rust, graph algorithms, petgraph
