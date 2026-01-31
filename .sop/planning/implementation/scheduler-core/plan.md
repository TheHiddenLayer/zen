# Plan: Scheduler Core Implementation

## Test Strategy

### Test Scenarios

1. **SchedulerEvent enum tests**
   - TaskStarted variant has task_id and agent_id
   - TaskCompleted variant has task_id and commit
   - TaskFailed variant has task_id and error
   - AllTasksComplete variant
   - Debug, Clone traits

2. **ImplResult struct tests**
   - Contains task_id, worktree, commit
   - Serialization/deserialization

3. **Scheduler creation tests**
   - new() creates scheduler with components
   - completed set starts empty

4. **Ready task dispatch tests**
   - Dispatches ready tasks up to capacity
   - Does not exceed pool capacity
   - Sends TaskStarted events

5. **Dependency respect tests**
   - Task with unmet dependency not started
   - Task with met dependency becomes ready

6. **Completion handling tests**
   - Completed task added to completed set
   - TaskCompleted event sent
   - Dependents become ready after completion

7. **All complete detection tests**
   - AllTasksComplete event sent when done
   - run() returns results

8. **Integration tests**
   - Full DAG execution with dependencies
   - Parallel execution respects capacity

## Implementation Plan

### 1. Define SchedulerEvent enum
```
SchedulerEvent {
    TaskStarted { task_id, agent_id },
    TaskCompleted { task_id, commit },
    TaskFailed { task_id, error },
    AllTasksComplete,
}
```

### 2. Define ImplResult struct
```
ImplResult {
    task_id: TaskId,
    worktree: PathBuf,
    commit: String,
}
```

### 3. Define Scheduler struct
```
Scheduler {
    dag: Arc<RwLock<TaskDAG>>,
    agent_pool: Arc<RwLock<AgentPool>>,
    event_tx: mpsc::Sender<SchedulerEvent>,
    completed: HashSet<TaskId>,
}
```

### 4. Implement run() method
- Loop while not all complete
- Get ready tasks from DAG
- Spawn agents for ready tasks (up to capacity)
- Wait for agent events
- Process completions/failures
- Update completed set
- Emit events
- Return Vec<ImplResult>

## Implementation Checklist

- [ ] Create scheduler.rs file
- [ ] Define SchedulerEvent enum
- [ ] Define ImplResult struct
- [ ] Define Scheduler struct
- [ ] Implement Scheduler::new()
- [ ] Implement Scheduler::run()
- [ ] Add unit tests for SchedulerEvent
- [ ] Add unit tests for ImplResult
- [ ] Add unit tests for Scheduler::new()
- [ ] Add unit tests for dispatch logic
- [ ] Add unit tests for dependency respect
- [ ] Add unit tests for completion handling
- [ ] Add unit tests for all complete detection
- [ ] Update mod.rs exports
- [ ] Run cargo test scheduler
- [ ] Run cargo build
