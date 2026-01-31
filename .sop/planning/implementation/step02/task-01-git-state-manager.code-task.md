# Task: Create GitStateManager Structure

## Description
Create the GitStateManager struct that wraps GitRefs, GitNotes, and GitOps to provide a unified interface for git-native state persistence.

## Background
Zen currently uses a JSON state file (~/.zen/state.json) for persistence. The git-native approach stores all state in git refs and notes, making it portable, versioned, and eliminating external dependencies. The existing GitRefs and GitNotes modules are production-ready and just need to be composed.

## Reference Documentation
**Required:**
- Design: .sop/planning/design/detailed-design.md (Section 4.7 Git State Manager, Section 5.1 Git State Schema)
- Research: .sop/planning/research/existing-code.md (Sections on GitRefs and GitNotes)

**Note:** You MUST read the detailed design document and existing code research before beginning implementation.

## Technical Requirements
1. Create `src/state/mod.rs` with module exports
2. Create `src/state/manager.rs` with `GitStateManager` struct:
   ```rust
   pub struct GitStateManager {
       refs: GitRefs,
       notes: GitNotes,
       ops: GitOps,
       repo_path: PathBuf,
   }
   ```
3. Implement constructor: `GitStateManager::new(repo_path: &Path) -> Result<Self>`
4. Add `pub mod state;` to `src/lib.rs`

## Dependencies
- Existing `GitRefs` module (`src/git_refs.rs`)
- Existing `GitNotes` module (`src/git_notes.rs`)
- Existing `GitOps` struct (`src/git.rs`)

## Implementation Approach
1. Study the existing GitRefs and GitNotes implementations
2. Create the state module directory structure
3. Implement GitStateManager composing the three git modules
4. Add basic health check method to verify git repo access
5. Wire up to lib.rs

## Acceptance Criteria

1. **Manager Construction**
   - Given a valid git repository path
   - When `GitStateManager::new(path)` is called
   - Then a manager is created with access to refs, notes, and ops

2. **Invalid Repo Handling**
   - Given an invalid or non-git directory
   - When `GitStateManager::new(path)` is called
   - Then an appropriate error is returned

3. **Component Access**
   - Given a GitStateManager instance
   - When accessing refs, notes, or ops
   - Then the underlying modules are accessible for operations

4. **Module Integration**
   - Given the state module is complete
   - When `cargo build` is run
   - Then the project compiles with `pub mod state;` in lib.rs

## Metadata
- **Complexity**: Low
- **Labels**: Foundation, Git, State Management, Infrastructure
- **Required Skills**: Rust, git2, module composition
