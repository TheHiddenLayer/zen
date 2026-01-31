# Task: Implement Merge Logic

## Description
Implement the merge() method that attempts to merge a task worktree into the staging branch, detecting and reporting conflicts.

## Background
Each task worktree contains commits that need to merge into a shared staging branch. Git's merge may succeed cleanly or produce conflicts that need resolution. The merge logic handles both cases.

## Reference Documentation
**Required:**
- Design: .sop/planning/design/detailed-design.md (Section 4.6 merge method, Section 6.3 Conflict Resolution pseudocode)

**Note:** You MUST read the detailed design document before beginning implementation.

## Technical Requirements
1. Add to ConflictResolver:
   - `merge(&self, worktree: &Path, staging_branch: &str) -> Result<MergeResult>`
2. Implement merge workflow:
   - Checkout staging branch
   - Attempt merge from worktree's branch
   - If clean: return Success with commit hash
   - If conflicts: extract conflict info, return Conflicts
   - If failed: return Failed with error
3. Use git2 for merge operations
4. Extract ours/theirs/base content from conflict markers

## Dependencies
- ConflictResolver from task-01
- git2 crate for merge operations

## Implementation Approach
1. Implement staging branch checkout
2. Use git2 merge with worktree's HEAD
3. Check merge result for conflicts
4. Extract conflict markers and content
5. Parse into ConflictFile structs
6. Add tests with intentional conflicts

## Acceptance Criteria

1. **Clean Merge**
   - Given worktree with non-conflicting changes
   - When merge() is called
   - Then MergeResult::Success is returned with commit

2. **Conflict Detection**
   - Given worktree modifying same file as staging
   - When merge() is called
   - Then MergeResult::Conflicts is returned with file list

3. **Conflict Content Extraction**
   - Given a conflicted file
   - When ConflictFile is created
   - Then ours and theirs content are extracted correctly

4. **Merge Failure Handling**
   - Given an unrecoverable merge error
   - When merge() is called
   - Then MergeResult::Failed is returned

5. **Multiple Conflicts**
   - Given 3 files with conflicts
   - When merge() returns Conflicts
   - Then all 3 ConflictFiles are included

## Metadata
- **Complexity**: High
- **Labels**: Git, Merge, Conflict, git2
- **Required Skills**: Rust, git2, merge algorithms
