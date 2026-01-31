# Plan: State Migration Tool

## Test Strategy

### Test Scenarios

1. **test_needs_migration_fresh_install**
   - Given: No state.json exists, no marker ref
   - When: `needs_migration()` called
   - Then: Returns `false` (nothing to migrate)

2. **test_needs_migration_existing_state_no_marker**
   - Given: state.json exists, no marker ref
   - When: `needs_migration()` called
   - Then: Returns `true` (migration needed)

3. **test_needs_migration_already_migrated**
   - Given: Marker ref exists
   - When: `needs_migration()` called
   - Then: Returns `false` (already migrated)

4. **test_migrate_creates_marker**
   - Given: state.json exists, no marker
   - When: `migrate()` called
   - Then: Marker ref is created at HEAD commit

5. **test_migrate_idempotent**
   - Given: Migration already completed
   - When: `migrate_if_needed()` called again
   - Then: No error, marker still exists

6. **test_migrate_without_state_file**
   - Given: No state.json exists
   - When: `migrate()` called
   - Then: Marker ref is created (fresh install case)

7. **test_marker_ref_points_to_head**
   - Given: Migration completed
   - When: Read marker ref
   - Then: Points to valid commit SHA

## Implementation Plan

### Step 1: Add Constants
Add migration marker ref constant to `src/state/manager.rs`:
```rust
const MIGRATION_MARKER_REF: &str = "migrated";
```

### Step 2: Implement needs_migration
```rust
pub fn needs_migration(&self) -> Result<bool>
```
- Check if marker ref exists
- If marker exists, return false
- Check if state.json exists via Config::state_path()
- If state.json doesn't exist, return false (fresh install)
- Return true (migration needed)

### Step 3: Implement migrate
```rust
pub fn migrate(&self) -> Result<()>
```
- Get HEAD commit SHA
- Create marker ref pointing to HEAD
- Log migration completion

### Step 4: Implement migrate_if_needed
```rust
pub fn migrate_if_needed(&self) -> Result<bool>
```
- Check needs_migration()
- If needed, call migrate()
- Return whether migration was performed

### Step 5: Add Tests
- Test all scenarios listed above
- Use tempfile for test repos
- Create mock state.json files for testing

## Checklist

- [ ] Add MIGRATION_MARKER_REF constant
- [ ] Implement needs_migration() method
- [ ] Implement migrate() method
- [ ] Implement migrate_if_needed() method
- [ ] Add unit tests for all scenarios
- [ ] Run cargo test to verify
- [ ] Run cargo build to verify compilation
