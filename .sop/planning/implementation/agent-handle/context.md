# Context: AgentHandle Implementation

## Task Description
Implement the full AgentHandle struct that wraps a running agent instance, providing methods to send input and read output from the agent's tmux session.

## Requirements

### From Task File
1. Create `AgentHandle` struct with fields:
   - `id: AgentId`
   - `status: AgentStatus`
   - `tmux_session: String`
   - `worktree_path: PathBuf`
   - `started_at: Instant`
   - `last_activity: Instant`
   - `cancel_token: CancellationToken`

2. Implement communication methods:
   - `send(&self, input: &str) -> Result<()>` - send to tmux pane
   - `read_output(&self) -> Result<AgentOutput>` - capture pane content
   - `worktree_path(&self) -> &Path`
   - `last_commit(&self) -> Result<Option<String>>`

3. Define `AgentOutput` enum: `Text(String)`, `Question(String)`, `Completed`, `Error(String)`

### Acceptance Criteria
1. Send Input - `handle.send("yes")` sends input to tmux pane
2. Read Output - `handle.read_output()` returns current pane content
3. Question Detection - Output with question pattern returns `AgentOutput::Question`
4. Completion Detection - Output indicating completion returns `AgentOutput::Completed`
5. Worktree Access - `handle.worktree_path()` returns the agent's worktree path

## Existing Patterns

### AgentId/AgentStatus (src/agent.rs)
- UUID-based newtype pattern for IDs
- Status enum with lifecycle states (Idle, Running, Stuck, Failed, Terminated)

### Tmux Module (src/tmux.rs)
- `Tmux::send_keys(name: &str, keys: &str) -> Result<()>` - send keys to pane
- `Tmux::send_keys_enter(name: &str, keys: &str) -> Result<()>` - send with Enter
- `Tmux::capture_pane_plain(name: &str) -> Result<String>` - capture without ANSI
- `Tmux::capture_pane_tail(name: &str, lines: u16) -> Result<String>` - capture last N lines
- `Tmux::session_exists(name: &str) -> bool`

### Current AgentHandle Placeholder (src/orchestration/pool.rs)
```rust
pub struct AgentHandle {
    pub id: AgentId,
    pub status: AgentStatus,
    pub task_id: Option<TaskId>,
}
```

### Git Operations (src/git.rs)
- `GitOps::head_commit(&self) -> Result<String>` - get HEAD commit SHA

## Dependencies
- `tokio_util::sync::CancellationToken`
- `crate::agent::{AgentId, AgentStatus}`
- `crate::tmux::Tmux`
- `crate::error::Result`
- `std::path::{Path, PathBuf}`
- `std::time::Instant`

## Implementation Notes
- The existing AgentHandle in pool.rs is a placeholder that needs to be replaced
- Communication with agent happens via tmux session
- Output parsing should detect questions (patterns like "?", "Do you want", etc.)
- Completion detection should look for skill completion markers
- Tests should use mock patterns since real tmux operations require infrastructure
