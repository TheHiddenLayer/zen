# Task: Create CleanupManager

## Description
Create the CleanupManager struct that handles automatic cleanup of worktrees and resources after workflow completion.

## Background
Worktrees accumulate during workflow execution. After tasks merge or workflows complete, these need cleanup to prevent disk space issues and maintain a clean state.

## Reference Documentation
**Required:**
- Design: .sop/planning/design/detailed-design.md (Section 4.7 GitStateManager worktree operations)

**Note:** You MUST read the detailed design document before beginning implementation.

## Technical Requirements
1. Create `src/cleanup.rs`:
   ```rust
   pub struct CleanupManager {
       git_ops: GitOps,
       config: CleanupConfig,
   }

   pub struct CleanupConfig {
       pub auto_cleanup: bool,
       pub cleanup_delay: Duration,
       pub keep_failed: bool,
   }
   ```
2. Implement cleanup methods:
   - `cleanup_task(&self, task: &Task) -> Result<()>`
   - `cleanup_workflow(&self, workflow: &Workflow) -> Result<()>`
   - `cleanup_orphaned(&self) -> Result<CleanupReport>`
3. Add to lib.rs

## Dependencies
- GitOps from existing code
- Task and Workflow types

## Implementation Approach
1. Define CleanupConfig with defaults
2. Create CleanupManager struct
3. Implement cleanup_task() removing worktree
4. Implement cleanup_workflow() removing all task worktrees
5. Implement cleanup_orphaned() detecting untracked worktrees
6. Add CleanupReport struct for results
7. Add tests for cleanup operations

## Acceptance Criteria

1. **Task Cleanup**
   - Given a completed task
   - When cleanup_task() is called
   - Then worktree is removed, branch is kept

2. **Workflow Cleanup**
   - Given completed workflow with 5 tasks
   - When cleanup_workflow() is called
   - Then all 5 worktrees are removed

3. **Keep Failed Option**
   - Given config.keep_failed = true and failed task
   - When cleanup is attempted
   - Then failed task worktree is preserved

4. **Orphan Detection**
   - Given worktree not linked to any workflow
   - When cleanup_orphaned() runs
   - Then orphan is reported (not auto-deleted)

5. **Cleanup Report**
   - Given cleanup operations
   - When complete
   - Then report shows what was cleaned

## Metadata
- **Complexity**: Medium
- **Labels**: Cleanup, Worktree, Resource Management
- **Required Skills**: Rust, file system, git
