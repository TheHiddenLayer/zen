# AI-Assisted Conflict Resolution - Context

## Task Summary
Implement `resolve_conflicts()` method that spawns a dedicated AI agent to resolve merge conflicts automatically.

## Requirements
1. Add `resolve_conflicts(&self, conflicts: Vec<ConflictFile>) -> Result<()>` to ConflictResolver
2. Spawn a dedicated resolver agent
3. Provide conflict context in prompt (ours/theirs/base content)
4. Agent uses Edit tool to fix each file
5. Verify all conflicts resolved (no conflict markers remain)
6. Commit resolution

## Dependencies
- ConflictResolver struct (already implemented in task-01 and task-02)
- AgentPool for spawning resolver agents
- AgentHandle for communication
- monitor_agent_output pattern from SkillsOrchestrator

## Existing Patterns

### spawn_for_skill() from AgentPool
```rust
pub async fn spawn_for_skill(&mut self, skill: &str) -> Result<AgentHandle>
```
Creates agent with synthetic task ID and skill-named tmux session.

### ConflictFile Structure
```rust
pub struct ConflictFile {
    pub path: PathBuf,
    pub ours: String,
    pub theirs: String,
    pub base: Option<String>,
}
```

### Error Types (src/error.rs)
Need to add: `ConflictResolutionFailed` error variant

## Implementation Approach
1. Format conflicts into a clear prompt with ours/theirs/base content
2. Spawn resolver agent via agent_pool.spawn_for_skill("conflict-resolver")
3. Send prompt with instructions to resolve each conflict
4. Monitor for completion (similar to monitor_agent_output pattern)
5. Verify no conflict markers remain in files
6. Create merge commit with resolution

## Conflict Markers to Detect
- `<<<<<<<`
- `=======`
- `>>>>>>>`
