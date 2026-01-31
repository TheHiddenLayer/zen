# Task: Implement Accept and Reject Commands

## Description
Implement `zen accept` that merges staging to main and `zen reject` that discards the workflow changes.

## Background
After reviewing a completed workflow, users decide whether to accept (merge to main) or reject (discard) the changes. These commands finalize the workflow lifecycle.

## Reference Documentation
**Required:**
- Design: .sop/planning/design/detailed-design.md (Section 2.4 User Workflow)

**Note:** You MUST read the detailed design document before beginning implementation.

## Technical Requirements
1. Implement `zen accept`:
   - Load workflow (most recent or specified)
   - Merge staging branch to main
   - Clean up worktrees
   - Mark workflow as accepted
   - Delete staging branch (optional)
2. Implement `zen reject`:
   - Load specified workflow
   - Delete staging branch
   - Clean up worktrees
   - Mark workflow as rejected
   - Keep branch history for debugging

## Dependencies
- CLI structure from task-01
- GitStateManager from Step 2
- CleanupManager from Step 18 (or inline cleanup)

## Implementation Approach
1. Create accept_command() handler
2. Verify workflow is complete
3. Perform git merge to main
4. Run cleanup
5. Update workflow status
6. Create reject_command() handler
7. Delete staging branch
8. Clean up worktrees
9. Add confirmation prompts
10. Add tests for both commands

## Acceptance Criteria

1. **Accept Merges**
   - Given completed workflow with staging branch
   - When `zen accept` is run
   - Then staging is merged to main

2. **Accept Cleanup**
   - Given accept completes
   - When checking worktrees
   - Then task worktrees are removed

3. **Reject Discards**
   - Given `zen reject wf-123`
   - When command executes
   - Then staging branch is deleted

4. **Reject Preserves History**
   - Given reject completes
   - When checking git log
   - Then task branches still exist for debugging

5. **Confirmation Prompt**
   - Given `zen accept` without --yes
   - When command runs
   - Then confirmation is requested

## Metadata
- **Complexity**: Medium
- **Labels**: CLI, Accept, Reject, Git
- **Required Skills**: Rust, git operations, CLI
