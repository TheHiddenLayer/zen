# Task: Implement Implementation Phase Runner

## Description
Implement the run_implementation_phase() method that executes /code-assist in parallel for each task, using the DAG scheduler to manage dependencies and concurrency.

## Background
Phase 3 is where the actual code gets written. Each task from /code-task-generator is assigned to an agent running /code-assist in auto mode. The scheduler ensures dependencies are respected while maximizing parallelism.

## Reference Documentation
**Required:**
- Design: .sop/planning/design/detailed-design.md (Section 4.2 run_implementation_phase code)

**Note:** You MUST read the detailed design document before beginning implementation.

## Technical Requirements
1. Add to SkillsOrchestrator:
   ```rust
   async fn run_implementation_phase(&self, tasks: &[CodeTask]) -> Result<Vec<ImplResult>> {
       let dag = self.build_task_dag(tasks).await?;
       let scheduler = Scheduler::new(dag, self.agent_pool.clone());
       scheduler.set_task_executor(|task, agent| async move {
           agent.send(&format!(
               "/code-assist\n\ntask_description: {}\nmode: auto",
               task.description
           )).await?;
           self.monitor_agent_output(&agent).await
       });
       scheduler.run().await
   }
   ```
2. Define `ImplResult` struct:
   - task_id, worktree, commit, files_changed
3. Build DAG from CodeTasks with AI-inferred dependencies
4. Wire into execute() as Phase 3

## Dependencies
- Scheduler from Step 10
- CodeTask from Step 9
- monitor_agent_output() from Step 6

## Implementation Approach
1. Define ImplResult struct
2. Implement build_task_dag() to infer dependencies
3. Configure scheduler with code-assist executor
4. Run scheduler and collect results
5. Return ImplResult for each completed task
6. Add tests with mock execution

## Acceptance Criteria

1. **DAG Building**
   - Given list of CodeTasks
   - When build_task_dag() is called
   - Then TaskDAG is created with AI-inferred dependencies

2. **Parallel Execution**
   - Given 4 independent tasks
   - When implementation phase runs
   - Then 4 agents run in parallel

3. **Code Assist Invocation**
   - Given a task to execute
   - When agent is spawned
   - Then /code-assist is invoked with task description

4. **Result Collection**
   - Given all tasks complete
   - When phase returns
   - Then ImplResult for each task includes commit hash

5. **Dependency Respect**
   - Given task B depends on task A
   - When scheduler runs
   - Then B doesn't start until A completes

## Metadata
- **Complexity**: High
- **Labels**: Orchestration, Phase 3, Implementation, Parallel
- **Required Skills**: Rust, async, scheduling, skill integration
