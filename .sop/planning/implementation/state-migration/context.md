# Context: State Migration Tool

## Overview
Create a migration tool that converts existing JSON state file data to the new git-native storage format, ensuring backward compatibility for existing Zen users.

## Requirements

### Functional Requirements
1. **Migration Detection**: Detect if migration is needed (JSON state exists, git-native doesn't have marker)
2. **Migration Marker**: Use `refs/zen/migrated` to track migration completion
3. **Idempotent**: Safe to run multiple times - skip if already migrated
4. **Session Preservation**: Sessions continue using JSON (backward compatible)
5. **Auto-Migration**: Call migration check on GitStateManager construction

### Acceptance Criteria
1. Fresh install with no state.json - no migration occurs, system works normally
2. Existing state.json - migration runs, marker ref created
3. Marker exists - migration is skipped (idempotent)
4. Existing sessions remain accessible after migration
5. `refs/zen/migrated` exists after successful migration

## Existing Patterns

### State File Location
From `src/config.rs:24-26`:
```rust
pub fn state_path() -> Result<PathBuf> {
    Ok(Self::zen_dir()?.join("state.json"))
}
```
State is stored at `~/.zen/state.json`.

### State JSON Format
From `src/session.rs:494-498`:
```rust
pub struct State {
    pub version: u32,
    pub sessions: Vec<Session>,
}
```

### GitStateManager Structure
From `src/state/manager.rs`:
- Already has `GitRefs`, `GitNotes`, and `GitOps` components
- Uses `refs/zen/workflows/{id}` for workflow refs
- Uses `refs/notes/zen/workflows/{id}` for workflow notes

### GitRefs Pattern
From `src/git_refs.rs`:
- Refs stored under `refs/zen/{name}` namespace
- `ref_exists()` returns bool for checking
- `create_ref()` for creating new refs
- `read_ref()` returns `Option<String>` for commit SHA

## Implementation Approach

1. Add `MIGRATION_MARKER_REF` constant: `"migrated"`
2. Add `needs_migration()` method:
   - Check if marker ref exists → if yes, return false
   - Check if state.json exists → if no, return false
   - Return true (migration needed)
3. Add `migrate()` method:
   - Read state.json if exists (sessions stay in JSON)
   - Create marker ref pointing to HEAD
4. Add `migrate_if_needed()` method:
   - Combines needs_migration + migrate
   - Called from constructor or separately

## Dependencies
- `GitStateManager` from task-01 (already implemented)
- `Config::state_path()` for JSON state location
- `GitRefs` for marker ref operations
