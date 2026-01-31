# Task: Create TaskDAG Structure with petgraph

## Description
Create the TaskDAG struct using petgraph that represents task dependencies as a directed acyclic graph, enabling parallel execution of independent tasks.

## Background
Tasks have dependencies (e.g., "create user model" before "create user API"). The DAG structure captures these relationships and allows the scheduler to identify which tasks can run in parallel.

## Reference Documentation
**Required:**
- Design: .sop/planning/design/detailed-design.md (Section 4.3 TaskDAG code)
- Research: .sop/planning/research/rust-ecosystem.md (petgraph section)

**Note:** You MUST read both documents before beginning implementation.

## Technical Requirements
1. Add petgraph to Cargo.toml: `petgraph = "0.6"`
2. Create `src/core/dag.rs` with:
   ```rust
   use petgraph::graph::{DiGraph, NodeIndex};

   pub struct TaskDAG {
       graph: DiGraph<Task, DependencyType>,
       task_index: HashMap<TaskId, NodeIndex>,
   }

   pub enum DependencyType {
       DataDependency,
       FileDependency { files: Vec<PathBuf> },
       SemanticDependency { reason: String },
   }
   ```
3. Implement basic operations:
   - `new() -> Self`
   - `add_task(&mut self, task: Task) -> NodeIndex`
   - `add_dependency(&mut self, from: &TaskId, to: &TaskId, dep_type: DependencyType)`
   - `get_task(&self, id: &TaskId) -> Option<&Task>`
4. Implement cycle detection on add_dependency()

## Dependencies
- Task, TaskId from task-01
- petgraph crate

## Implementation Approach
1. Add petgraph dependency
2. Define DependencyType enum
3. Create TaskDAG with DiGraph and index HashMap
4. Implement add_task() storing in both graph and index
5. Implement add_dependency() with cycle check
6. Add tests for graph construction

## Acceptance Criteria

1. **Task Addition**
   - Given an empty DAG
   - When add_task() is called
   - Then task is stored and retrievable via get_task()

2. **Dependency Addition**
   - Given tasks A and B in the DAG
   - When add_dependency(A, B, DataDependency) is called
   - Then edge exists from A to B

3. **Cycle Detection**
   - Given tasks A->B->C
   - When add_dependency(C, A) is attempted
   - Then error is returned (would create cycle)

4. **Dependency Types**
   - Given different dependency types
   - When edges are created
   - Then correct DependencyType is stored on edge

5. **Index Consistency**
   - Given tasks added to DAG
   - When looking up by TaskId
   - Then correct NodeIndex is returned

## Metadata
- **Complexity**: Medium
- **Labels**: Core, DAG, Graph, petgraph
- **Required Skills**: Rust, petgraph, graph algorithms
