# Plan: Implement Merge Logic

## Test Strategy

### Test Scenarios

1. **Clean Merge Test**
   - Create temp git repo with initial commit
   - Create staging branch from HEAD
   - Create worktree with feature branch
   - Make non-conflicting changes in worktree
   - Call merge() - expect MergeResult::Success with commit hash

2. **Conflict Detection Test**
   - Create temp git repo with a file
   - Create staging branch, modify the file
   - Create worktree, modify same file differently
   - Call merge() - expect MergeResult::Conflicts with file list

3. **Conflict Content Extraction Test**
   - Setup conflict scenario as above
   - Verify ConflictFile has correct ours/theirs/base content

4. **Merge Failure Test**
   - Create scenario where merge fails (invalid branch)
   - Call merge() - expect MergeResult::Failed

5. **Multiple Conflicts Test**
   - Create repo with 3 files
   - Create conflicting changes in all 3 files
   - Call merge() - expect 3 ConflictFiles

## Implementation Plan

### Phase 1: Add merge() method signature
- Add async fn merge(&self, worktree: &Path, staging_branch: &str) -> Result<MergeResult>
- Import necessary git2 types

### Phase 2: Implement staging branch checkout
- Open the main repository (not worktree)
- Find the staging branch
- Checkout the staging branch

### Phase 3: Implement merge from worktree branch
- Get worktree's current branch name
- Find the commit at the worktree's HEAD
- Attempt merge using git2's merge functionality

### Phase 4: Handle merge results
- If clean merge: create merge commit, return Success
- If conflicts: extract conflict info, return Conflicts
- If error: return Failed

### Phase 5: Implement conflict extraction
- Parse conflicted index entries
- Extract ours/theirs/base content from git blobs
- Create ConflictFile structs

## Technical Approach

The merge will use git2's low-level merge APIs:
1. `repo.merge_analysis()` to determine if merge is needed
2. `repo.merge_commits()` to perform the actual merge
3. Check `index.has_conflicts()` for conflict detection
4. Use `index.conflicts()` iterator to get conflict entries
5. For each conflict entry, read blob content for ours/theirs/base
