# Context: Implement Merge Logic

## Task Overview
Implement the `merge()` method in `ConflictResolver` that attempts to merge a task worktree into a staging branch, detecting and reporting conflicts.

## Requirements
1. Add `merge(&self, worktree: &Path, staging_branch: &str) -> Result<MergeResult>` to ConflictResolver
2. Implement merge workflow:
   - Checkout staging branch
   - Attempt merge from worktree's branch
   - If clean: return Success with commit hash
   - If conflicts: extract conflict info, return Conflicts
   - If failed: return Failed with error
3. Use git2 for merge operations
4. Extract ours/theirs/base content from conflict markers

## Existing Patterns

### ConflictResolver (src/orchestration/resolver.rs)
- Already has `ConflictResolver` struct with `git_ops: GitOps` and `agent_pool: Arc<RwLock<AgentPool>>`
- `MergeResult` enum with Success, Conflicts, Failed variants
- `ConflictFile` struct with path, ours, theirs, base fields

### GitOps (src/git.rs)
- Uses `git2::Repository` for git operations
- Pattern: `Repository::discover(&self.repo_path)` for main repo
- Pattern: `Repository::open(worktree_path)` for worktree operations
- Has helper methods for branch operations, commits, worktrees

### git2 Merge Patterns
- `repo.find_branch()` to get branch reference
- `repo.merge_commits()` or `repo.merge()` for merge operations
- `repo.index()` to check for conflicts via `index.has_conflicts()`
- `index.conflicts()` iterator for conflict entries

## Implementation Paths
- Target file: `src/orchestration/resolver.rs`
- Test location: Same file in `#[cfg(test)] mod tests`

## Dependencies
- git2 crate (already in Cargo.toml v0.20)
- ConflictResolver from task-01 (already implemented)
