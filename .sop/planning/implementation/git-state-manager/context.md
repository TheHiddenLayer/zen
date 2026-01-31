# Git State Manager - Context

## Task Description
Create the GitStateManager struct that wraps GitRefs, GitNotes, and GitOps to provide a unified interface for git-native state persistence.

## Requirements
1. Create `src/state/mod.rs` with module exports
2. Create `src/state/manager.rs` with `GitStateManager` struct
3. Implement constructor: `GitStateManager::new(repo_path: &Path) -> Result<Self>`
4. Add `pub mod state;` to `src/lib.rs`
5. Add basic health check method to verify git repo access

## Acceptance Criteria
1. Manager Construction - valid repo path creates manager with access to refs, notes, ops
2. Invalid Repo Handling - invalid path returns appropriate error
3. Component Access - refs, notes, ops are accessible for operations
4. Module Integration - `cargo build` compiles with `pub mod state;` in lib.rs

## Existing Patterns

### GitRefs (src/git_refs.rs)
- `GitRefs::new(repo_path: &Path) -> Result<Self>` - validates path is a git repo
- Uses `Repository::discover()` for validation
- Stores `repo_path: PathBuf`
- Private `repo()` method to get fresh Repository handle

### GitNotes (src/git_notes.rs)
- `GitNotes::new(repo_path: &Path) -> Result<Self>` - validates path is a git repo
- Same pattern: uses `Repository::discover()` for validation
- Stores `repo_path: PathBuf`
- Private `repo()` method to get fresh Repository handle

### GitOps (src/git.rs)
- `GitOps::new(repo_path: &Path) -> Result<Self>` - validates path is a git repo
- Same pattern: uses `Repository::discover()` for validation
- Has `repo_path()` accessor method

### Error Handling
- Uses `thiserror::Error` derive macro
- Git errors convert via `#[from] git2::Error`
- Custom errors for RefExists, RefNotFound, InvalidPhaseTransition

### Logging
- Uses `zlog_debug!` macro for debug logging

## Implementation Approach
1. Create state module directory: `src/state/`
2. Create `src/state/mod.rs` - exports manager module
3. Create `src/state/manager.rs` - GitStateManager struct composing GitRefs, GitNotes, GitOps
4. Add `pub mod state;` to `src/lib.rs`
5. Validate single repo_path is shared across all three components
