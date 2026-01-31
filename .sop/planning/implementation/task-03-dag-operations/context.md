# Context: DAG Scheduling Operations

## Task Overview
Implement DAG operations needed for parallel task scheduling in Zen v2.

## Source File
- `src/core/dag.rs` - TaskDAG structure (exists, needs extension)

## Requirements
From `.sop/planning/implementation/step08/task-03-dag-operations.code-task.md`:

1. Add scheduling methods to TaskDAG:
   - `ready_tasks(&self, completed: &HashSet<TaskId>) -> Vec<&Task>`
   - `complete_task(&mut self, id: &TaskId) -> Result<()>`
   - `all_complete(&self, completed: &HashSet<TaskId>) -> bool`
   - `topological_order(&self) -> Result<Vec<&Task>>`
   - `pending_count(&self, completed: &HashSet<TaskId>) -> usize`

2. `ready_tasks` returns tasks where all dependencies are in completed set
3. `topological_order` uses petgraph's toposort
4. Note: `task_count()` already exists

## Existing Patterns
- TaskDAG uses petgraph DiGraph<Task, DependencyType>
- task_index HashMap<TaskId, NodeIndex> for fast lookups
- get_dependencies() returns predecessors (incoming edges)
- Cycle detection via petgraph::algo::is_cyclic_directed
- Error handling via crate::error::{Error, Result}

## Dependencies
- petgraph (already in Cargo.toml)
- std::collections::HashSet (needs import)
- petgraph::algo::toposort (needs import)

## Implementation Paths
- File: `src/core/dag.rs`
- Tests: Same file, in `mod tests` block
