# Plan: DAG Scheduling Operations

## Test Scenarios

### 1. ready_tasks(&self, completed: &HashSet<TaskId>) -> Vec<&Task>

**Scenario 1.1: Empty DAG returns empty**
- Input: Empty DAG, empty completed set
- Expected: Empty vec

**Scenario 1.2: Independent tasks with nothing completed**
- Input: DAG with A, B, D (no dependencies), completed={}
- Expected: [A, B, D] (all ready)

**Scenario 1.3: Chain with nothing completed**
- Input: DAG A->B->C, completed={}
- Expected: [A] (only A has no dependencies)

**Scenario 1.4: Chain with partial completion**
- Input: DAG A->B->C, completed={A}
- Expected: [B]

**Scenario 1.5: Diamond pattern**
- Input: DAG A->C, B->C, D, completed={}
- Expected: [A, B, D]

**Scenario 1.6: Diamond with partial completion**
- Input: DAG A->C, B->C, completed={A}
- Expected: [B] (C needs both A and B)

**Scenario 1.7: Diamond fully ready**
- Input: DAG A->C, B->C, completed={A, B}
- Expected: [C]

### 2. complete_task(&mut self, id: &TaskId) -> Result<()>

**Scenario 2.1: Complete existing task**
- Input: DAG with task A, call complete_task(A)
- Expected: Ok(()), task status becomes Completed

**Scenario 2.2: Complete non-existent task**
- Input: Empty DAG, call complete_task(random_id)
- Expected: Error (task not found)

**Scenario 2.3: Complete task sets completed_at**
- Input: Task A, complete_task(A)
- Expected: completed_at is set

### 3. all_complete(&self, completed: &HashSet<TaskId>) -> bool

**Scenario 3.1: Empty DAG is complete**
- Input: Empty DAG, empty set
- Expected: true

**Scenario 3.2: Non-empty DAG, nothing completed**
- Input: DAG with 3 tasks, completed={}
- Expected: false

**Scenario 3.3: All tasks completed**
- Input: DAG with 5 tasks, completed contains all 5
- Expected: true

**Scenario 3.4: Some tasks completed**
- Input: DAG with 3 tasks, completed contains 2
- Expected: false

### 4. topological_order(&self) -> Result<Vec<&Task>>

**Scenario 4.1: Empty DAG**
- Input: Empty DAG
- Expected: Ok(empty vec)

**Scenario 4.2: Linear chain**
- Input: DAG A->B->C
- Expected: [A, B, C] in that order

**Scenario 4.3: Diamond pattern**
- Input: DAG A->C, B->C
- Expected: A and B before C

**Scenario 4.4: Multiple independent subgraphs**
- Input: DAG A->B, C->D
- Expected: A before B, C before D

### 5. pending_count(&self, completed: &HashSet<TaskId>) -> usize

**Scenario 5.1: Empty DAG**
- Input: Empty DAG, empty set
- Expected: 0

**Scenario 5.2: All pending**
- Input: DAG with 5 tasks, completed={}
- Expected: 5

**Scenario 5.3: Some completed**
- Input: DAG with 5 tasks, completed has 2
- Expected: 3

**Scenario 5.4: All completed**
- Input: DAG with 3 tasks, completed has all 3
- Expected: 0

## Implementation Plan

### Step 1: Add imports
- Add `use std::collections::HashSet;`
- Add `use petgraph::algo::toposort;`

### Step 2: Implement ready_tasks
- Iterate all tasks
- For each task, get dependencies (incoming edges)
- Return task if all dependencies are in completed set

### Step 3: Implement complete_task
- Get mutable task by ID
- Call task.complete() method
- Return error if task not found

### Step 4: Implement all_complete
- Compare completed set size with task count
- All task IDs must be in completed set

### Step 5: Implement topological_order
- Use petgraph::algo::toposort
- Map NodeIndex results back to Task references

### Step 6: Implement pending_count (already exists, verify)
- Note: task_count already exists
- pending_count = task_count - completed.len() that are in DAG

## Implementation Checklist
- [ ] Add required imports
- [ ] Implement ready_tasks
- [ ] Implement complete_task
- [ ] Implement all_complete
- [ ] Implement topological_order
- [ ] Implement pending_count
- [ ] All tests pass
- [ ] Build succeeds
