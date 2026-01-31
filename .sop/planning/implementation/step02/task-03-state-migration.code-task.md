# Task: Create State Migration Tool

## Description
Create a migration tool that converts existing JSON state file data to the new git-native storage format, ensuring backward compatibility for existing Zen users.

## Background
Existing Zen installations use ~/.zen/state.json for session state. The migration tool must preserve this data while transitioning to git-native storage. The migration should be automatic on first run and idempotent (safe to run multiple times).

## Reference Documentation
**Required:**
- Design: .sop/planning/design/detailed-design.md (Section 4.7 Git State Manager)
- Research: .sop/planning/research/existing-code.md (Section 7 State Persistence)

**Note:** You MUST read the existing code research to understand the current JSON state format.

## Technical Requirements
1. Create `src/state/migration.rs` with migration functions
2. Detect if migration is needed (JSON state exists, git-native doesn't)
3. Read existing JSON state file format
4. Convert session data to new format (sessions remain in JSON for now, workflows use git-native)
5. Mark migration as complete (create marker ref)
6. Make migration idempotent - skip if already migrated

## Dependencies
- GitStateManager from task-01
- Existing State struct in `src/session.rs`

## Implementation Approach
1. Create migration marker ref: `refs/zen/migrated`
2. Implement `needs_migration(&self) -> bool` - check for marker
3. Implement `migrate(&self) -> Result<()>`:
   - Read ~/.zen/state.json if exists
   - Sessions continue using JSON (backward compatible)
   - Create marker ref to indicate migration complete
4. Call migration check on GitStateManager construction
5. Add tests with mock state files

## Acceptance Criteria

1. **Fresh Install (No Migration)**
   - Given a new installation with no state.json
   - When GitStateManager is created
   - Then no migration occurs and system works normally

2. **Existing Install Migration**
   - Given an existing state.json file
   - When GitStateManager is created for the first time
   - Then migration runs and marker ref is created

3. **Idempotent Migration**
   - Given migration has already run (marker exists)
   - When GitStateManager is created again
   - Then migration is skipped

4. **Session Backward Compatibility**
   - Given existing sessions in state.json
   - When migration completes
   - Then existing sessions remain accessible

5. **Migration Marker**
   - Given migration has completed
   - When checking `refs/zen/migrated`
   - Then the ref exists indicating successful migration

## Metadata
- **Complexity**: Medium
- **Labels**: Migration, Backward Compatibility, State Management
- **Required Skills**: Rust, file I/O, git refs, JSON parsing
