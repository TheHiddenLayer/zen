# Task: Implement Task Generation Phase

## Description
Implement the run_task_generation_phase() method that executes /code-task-generator as Phase 2 of the workflow, creating .code-task.md files from the PDD plan.

## Background
After PDD completes with a plan.md, /code-task-generator breaks down the plan into individual code tasks. Each task becomes a .code-task.md file that can be executed independently.

## Reference Documentation
**Required:**
- Design: .sop/planning/design/detailed-design.md (Section 4.2 run_task_generation_phase code)

**Note:** You MUST read the detailed design document before beginning implementation.

## Technical Requirements
1. Add to SkillsOrchestrator:
   ```rust
   async fn run_task_generation_phase(&self, pdd: &PDDResult) -> Result<Vec<CodeTask>> {
       let agent = self.agent_pool.write().await
           .spawn_for_skill("code-task-generator").await?;
       agent.send(&format!(
           "/code-task-generator\n\ninput: {}",
           pdd.plan_path.display()
       )).await?;
       self.monitor_agent_output(&agent).await?;
       CodeTask::from_directory(&agent.worktree_path())
   }
   ```
2. Pass PDD plan.md path as input
3. Use AIHumanProxy to approve task breakdown
4. Parse generated .code-task.md files
5. Wire into execute() as Phase 2

## Dependencies
- SkillsOrchestrator from Step 6
- PDDResult from Step 7
- CodeTask from task-01
- monitor_agent_output() from Step 6

## Implementation Approach
1. Implement run_task_generation_phase()
2. Format /code-task-generator invocation with plan path
3. Monitor for questions and auto-answer
4. After completion, scan for generated files
5. Convert to CodeTask objects
6. Add tests with mock generator output

## Acceptance Criteria

1. **Generator Invocation**
   - Given PDDResult with plan_path
   - When run_task_generation_phase() is called
   - Then /code-task-generator is invoked with plan.md path

2. **Task Breakdown Approval**
   - Given generator asks for approval
   - When AIHumanProxy responds
   - Then generation continues

3. **File Discovery**
   - Given generator creates 5 .code-task.md files
   - When phase completes
   - Then 5 CodeTask objects are returned

4. **Error Handling**
   - Given generator fails
   - When error occurs
   - Then appropriate error is returned with context

5. **Phase Integration**
   - Given execute() runs Phase 2
   - When PDD phase completes
   - Then task generation phase runs with PDD output

## Metadata
- **Complexity**: Medium
- **Labels**: Skills, Phase 2, CodeTask, Orchestration
- **Required Skills**: Rust, async, skill integration
