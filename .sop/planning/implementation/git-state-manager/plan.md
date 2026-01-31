# Git State Manager - Plan

## Test Scenarios

### 1. Constructor Tests
- `test_new_with_valid_repo` - Given a valid git repo path, constructor returns Ok(GitStateManager)
- `test_new_with_invalid_path` - Given non-existent path, constructor returns Err
- `test_new_with_non_git_dir` - Given a directory that's not a git repo, constructor returns Err

### 2. Component Access Tests
- `test_refs_accessible` - Can access and use refs() to create/read refs
- `test_notes_accessible` - Can access and use notes() to set/get notes
- `test_ops_accessible` - Can access and use ops() for git operations

### 3. Health Check Tests
- `test_health_check_valid_repo` - Health check returns Ok for valid repo
- `test_repo_path_accessor` - repo_path() returns correct path

## Implementation Tasks

- [ ] Create `src/state/mod.rs` with module export
- [ ] Create `src/state/manager.rs` with GitStateManager struct
- [ ] Implement `GitStateManager::new()` constructor
- [ ] Implement accessor methods: `refs()`, `notes()`, `ops()`, `repo_path()`
- [ ] Add `pub mod state;` to `src/lib.rs`
- [ ] Write tests for all acceptance criteria
