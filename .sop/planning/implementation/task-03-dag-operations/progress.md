# Progress: DAG Scheduling Operations

## Script Execution
- [x] Setup documentation structure
- [x] Explore requirements and patterns
- [x] Plan test strategy
- [x] Implement tests (TDD RED phase)
- [x] Implement operations (TDD GREEN phase)
- [x] Refactor and validate
- [x] Commit changes

## Setup Notes
- Documentation directory: `.sop/planning/implementation/task-03-dag-operations/`
- Implementation file: `src/core/dag.rs`
- Task file read and requirements understood

## Implementation Summary

### Methods Added to TaskDAG

1. **ready_tasks(&self, completed: &HashSet<TaskId>) -> Vec<&Task>**
   - Returns tasks where all dependencies are in the completed set
   - Excludes already-completed tasks from the result

2. **complete_task(&mut self, id: &TaskId) -> Result<()>**
   - Marks a task as completed using Task::complete()
   - Returns error if task not found

3. **all_complete(&self, completed: &HashSet<TaskId>) -> bool**
   - Checks if every task ID in the DAG is in the completed set

4. **topological_order(&self) -> Result<Vec<&Task>>**
   - Uses petgraph::algo::toposort for dependency-respecting order
   - Returns error on cycle (though cycles prevented by add_dependency)

5. **pending_count(&self, completed: &HashSet<TaskId>) -> usize**
   - Counts tasks not in the completed set

### Test Coverage
- 25 new tests for scheduling operations
- 64 total DAG tests passing
- 690 total tests in project (687 pass, 3 ignored)
- All acceptance criteria from task file verified

## Commit
- Hash: (pending)
- Message: feat(core): implement DAG scheduling operations for parallel execution
