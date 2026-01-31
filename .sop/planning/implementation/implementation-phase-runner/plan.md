# Implementation Phase Runner - Plan

## Test Strategy

### Unit Tests

1. **build_task_dag() tests**
   - Given list of CodeTasks without dependencies, creates DAG with all tasks
   - Given CodeTasks with dependencies, creates DAG with correct edges
   - Given empty list, creates empty DAG
   - Handles missing dependency references gracefully

2. **run_implementation_phase() tests**
   - Given CodeTasks, calls build_task_dag and creates scheduler
   - Given empty tasks list, returns empty results
   - Tests integration with scheduler dispatch

3. **Integration tests**
   - Verify execute() now calls run_implementation_phase with generated tasks
   - Verify phase transitions happen correctly

## Implementation Steps

### Step 1: Add build_task_dag() method

```rust
async fn build_task_dag(&self, tasks: &[CodeTask]) -> Result<TaskDAG> {
    let mut dag = TaskDAG::new();
    let mut id_map: HashMap<String, TaskId> = HashMap::new();

    // First pass: add all tasks
    for code_task in tasks {
        let task = code_task.to_task();
        id_map.insert(code_task.id.clone(), task.id);
        dag.add_task(task);
    }

    // Second pass: add dependencies
    for code_task in tasks {
        let task_id = id_map.get(&code_task.id).unwrap();
        for dep_id in &code_task.dependencies {
            if let Some(dep_task_id) = id_map.get(dep_id) {
                dag.add_dependency(dep_task_id, task_id, DependencyType::DataDependency)?;
            }
        }
    }

    Ok(dag)
}
```

### Step 2: Implement run_implementation_phase()

```rust
async fn run_implementation_phase(&self, tasks: &[CodeTask]) -> Result<Vec<ImplResult>> {
    if tasks.is_empty() {
        return Ok(Vec::new());
    }

    // Build DAG from CodeTasks
    let dag = self.build_task_dag(tasks).await?;

    // Create scheduler event channel
    let (event_tx, _event_rx) = mpsc::channel(100);

    // Get agent event receiver from pool
    let pool = self.agent_pool.clone();

    // Create scheduler
    let scheduler = Scheduler::new(
        Arc::new(RwLock::new(dag)),
        pool,
        event_tx,
        self.repo_path.clone(),
    );

    // Run scheduler to completion
    // Note: Need to get agent_rx from somewhere
    scheduler.run(&mut agent_rx).await
}
```

### Step 3: Update execute() to pass generated tasks

The execute() method already captures generated tasks in `_generated_tasks`.
Need to pass them to run_implementation_phase().

## Acceptance Criteria Mapping

| Criterion | Test |
|-----------|------|
| DAG Building | test_build_task_dag_* |
| Parallel Execution | Scheduler tests (already done in Step 10) |
| Code Assist Invocation | Integration test |
| Result Collection | test_run_implementation_phase_returns_results |
| Dependency Respect | Scheduler tests (already done in Step 10) |
