# Task: Implement Task Spawning with Worktree Isolation

## Description
Implement task spawning that creates an isolated git worktree for each task and launches an agent in that worktree.

## Background
Each task runs in its own git worktree to prevent conflicts during parallel execution. The worktree is created from the base branch, and the agent's changes are isolated until merge phase.

## Reference Documentation
**Required:**
- Design: .sop/planning/design/detailed-design.md (Section 4.7 GitStateManager worktree operations)
- Research: .sop/planning/research/existing-code.md (GitOps section)

**Note:** You MUST read both documents to understand worktree management.

## Technical Requirements
1. Add to Scheduler:
   - `spawn_task(&mut self, task: &Task) -> Result<AgentId>`
2. Worktree creation:
   - Path: `~/.zen/worktrees/{task-id}`
   - Branch: `zen/task/{task-id}`
3. Agent spawning:
   - Create tmux session in worktree
   - Launch Claude Code agent
   - Return AgentId for tracking
4. Update task with worktree_path and agent_id

## Dependencies
- Scheduler from task-01
- GitOps for worktree creation
- AgentPool for agent spawning

## Implementation Approach
1. Implement spawn_task() method
2. Generate unique worktree path and branch name
3. Use GitOps to create worktree
4. Use AgentPool to spawn agent in worktree
5. Update Task with assignment info
6. Emit TaskStarted event
7. Add tests for worktree creation

## Acceptance Criteria

1. **Worktree Creation**
   - Given a task to spawn
   - When spawn_task() is called
   - Then worktree is created at ~/.zen/worktrees/{task-id}

2. **Branch Creation**
   - Given worktree is created
   - When git status is checked
   - Then branch zen/task/{task-id} exists

3. **Agent Launch**
   - Given worktree exists
   - When agent is spawned
   - Then agent runs in the worktree directory

4. **Task Update**
   - Given spawn completes
   - When task is examined
   - Then worktree_path and agent_id are set

5. **Event Emission**
   - Given spawn completes
   - When spawn_task() returns
   - Then TaskStarted event is emitted

## Metadata
- **Complexity**: Medium
- **Labels**: Scheduler, Worktree, Git, Isolation
- **Required Skills**: Rust, git worktrees, file system
