# Task: Implement AgentHandle for Agent Communication

## Description
Create the AgentHandle struct that wraps a running agent instance, providing methods to send input and read output from the agent's tmux session.

## Background
Each spawned agent runs in its own tmux session and worktree. AgentHandle provides the interface to interact with the agent: sending commands/input and capturing output. This enables the orchestrator to drive skills and capture results.

## Reference Documentation
**Required:**
- Design: .sop/planning/design/detailed-design.md (Section 4.3 Agent Management)
- Research: .sop/planning/research/existing-code.md (Section on src/tmux.rs)

**Note:** You MUST read both documents to understand tmux integration.

## Technical Requirements
1. Create `AgentHandle` struct in `src/orchestration/pool.rs`:
   ```rust
   pub struct AgentHandle {
       pub id: AgentId,
       pub status: AgentStatus,
       pub tmux_session: String,
       pub worktree_path: PathBuf,
       pub started_at: Instant,
       pub last_activity: Instant,
       cancel_token: CancellationToken,
   }
   ```
2. Implement communication methods:
   - `send(&self, input: &str) -> Result<()>` - send to tmux pane
   - `read_output(&self) -> Result<AgentOutput>` - capture pane content
   - `worktree_path(&self) -> &Path`
   - `last_commit(&self) -> Result<Option<String>>`
3. Define `AgentOutput` enum: Text(String), Question(String), Completed, Error(String)

## Dependencies
- AgentId, AgentStatus from task-01
- Existing Tmux module for pane operations
- tokio-util CancellationToken

## Implementation Approach
1. Define AgentOutput enum for parsed output types
2. Create AgentHandle struct with all fields
3. Implement send() using Tmux::send_keys (may need to add to tmux.rs)
4. Implement read_output() using Tmux::capture_pane_plain
5. Add output parsing to detect questions vs completion
6. Add tests with mock tmux operations

## Acceptance Criteria

1. **Send Input**
   - Given an active agent handle
   - When `handle.send("yes")` is called
   - Then input is sent to the agent's tmux pane

2. **Read Output**
   - Given an agent that has produced output
   - When `handle.read_output()` is called
   - Then the current pane content is returned

3. **Question Detection**
   - Given agent output containing a question pattern
   - When read_output() parses it
   - Then AgentOutput::Question is returned

4. **Completion Detection**
   - Given agent output indicating task completion
   - When read_output() parses it
   - Then AgentOutput::Completed is returned

5. **Worktree Access**
   - Given an agent handle
   - When `handle.worktree_path()` is called
   - Then the agent's isolated worktree path is returned

## Metadata
- **Complexity**: Medium
- **Labels**: Agent, Tmux, Communication, I/O
- **Required Skills**: Rust, tmux integration, output parsing
