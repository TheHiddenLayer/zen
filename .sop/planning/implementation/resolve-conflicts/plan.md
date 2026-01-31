# AI-Assisted Conflict Resolution - Plan

## Test Scenarios

### 1. Resolver Spawn Test
- **Given**: Conflicts to resolve
- **When**: resolve_conflicts() is called
- **Then**: A dedicated resolver agent is spawned via agent_pool

### 2. Context Provision Test
- **Given**: ConflictFile with ours/theirs content
- **When**: format_conflict_prompt() is called
- **Then**: Both versions are clearly presented in structured format

### 3. Multiple Conflicts Formatting Test
- **Given**: Multiple ConflictFiles
- **When**: format_conflict_prompt() is called
- **Then**: All conflicts are included in the prompt

### 4. Conflict Marker Detection Test
- **Given**: File content with conflict markers
- **When**: has_conflict_markers() is called
- **Then**: Returns true

### 5. Clean Content Detection Test
- **Given**: File content without conflict markers
- **When**: has_conflict_markers() is called
- **Then**: Returns false

### 6. Resolution Failure Test
- **Given**: Resolver can't fix conflicts (markers remain)
- **When**: verification fails
- **Then**: ConflictResolutionFailed error is returned

### 7. Commit Creation Test
- **Given**: Successful resolution
- **When**: process completes
- **Then**: Resolution commit is created

## Implementation Tasks

- [ ] Add ConflictResolutionFailed error variant to error.rs
- [ ] Implement format_conflict_prompt() helper
- [ ] Implement has_conflict_markers() helper
- [ ] Implement verify_resolution() method
- [ ] Implement commit_resolution() method
- [ ] Implement resolve_conflicts() main method
- [ ] Add unit tests for all helpers
- [ ] Add integration-style tests for resolve_conflicts()

## API Design

```rust
impl ConflictResolver {
    /// Resolve merge conflicts using a dedicated AI agent.
    pub async fn resolve_conflicts(
        &self,
        conflicts: Vec<ConflictFile>,
        repo_path: &Path,
    ) -> Result<String>; // Returns commit hash

    /// Format conflicts into a clear prompt for the resolver agent.
    fn format_conflict_prompt(&self, conflicts: &[ConflictFile]) -> String;

    /// Check if file content contains conflict markers.
    fn has_conflict_markers(content: &str) -> bool;

    /// Verify all conflict files have been resolved.
    fn verify_resolution(&self, conflict_paths: &[PathBuf], repo_path: &Path) -> Result<()>;

    /// Commit the resolution.
    fn commit_resolution(&self, repo_path: &Path, message: &str) -> Result<String>;
}
```
