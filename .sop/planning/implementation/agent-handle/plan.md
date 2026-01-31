# Plan: AgentHandle Implementation

## Test Strategy

### Test Scenarios

#### 1. AgentOutput Enum Tests
- `test_agent_output_text_variant` - Text variant holds string content
- `test_agent_output_question_variant` - Question variant holds question string
- `test_agent_output_completed_variant` - Completed variant exists
- `test_agent_output_error_variant` - Error variant holds error message
- `test_agent_output_debug_format` - Debug trait implementation
- `test_agent_output_clone` - Clone trait implementation

#### 2. AgentHandle Struct Tests
- `test_agent_handle_new` - Creates handle with all required fields
- `test_agent_handle_worktree_path` - Returns correct worktree path
- `test_agent_handle_tmux_session` - Returns correct tmux session name
- `test_agent_handle_started_at` - Tracks start time
- `test_agent_handle_last_activity` - Tracks last activity time

#### 3. Output Parsing Tests
- `test_parse_output_detects_question_mark` - "?" at end triggers Question
- `test_parse_output_detects_do_you_want` - "Do you want" triggers Question
- `test_parse_output_detects_would_you_like` - "Would you like" triggers Question
- `test_parse_output_detects_completion` - Task completion markers trigger Completed
- `test_parse_output_detects_error` - Error patterns trigger Error
- `test_parse_output_returns_text_default` - Default is Text variant

#### 4. Communication Methods (unit testable with mocking)
- `test_send_constructs_correct_input` - Validates input string handling
- `test_read_output_parses_content` - Validates output parsing pipeline

#### 5. Last Commit Detection
- `test_last_commit_returns_some_when_exists` - Returns commit SHA
- `test_last_commit_returns_none_for_clean` - No new commits returns None

## Implementation Plan

### Step 1: Define AgentOutput Enum
Location: `src/orchestration/pool.rs`

```rust
#[derive(Debug, Clone)]
pub enum AgentOutput {
    Text(String),
    Question(String),
    Completed,
    Error(String),
}
```

### Step 2: Update AgentHandle Struct
Replace the current placeholder with full implementation:

```rust
pub struct AgentHandle {
    pub id: AgentId,
    pub status: AgentStatus,
    pub task_id: Option<TaskId>,
    pub tmux_session: String,
    pub worktree_path: PathBuf,
    pub started_at: Instant,
    pub last_activity: Instant,
    cancel_token: CancellationToken,
}
```

### Step 3: Implement Constructor
- `AgentHandle::new()` - Initialize with all required fields
- Update `AgentHandle::with_task()` to include new fields

### Step 4: Implement Accessor Methods
- `worktree_path(&self) -> &Path`
- `tmux_session(&self) -> &str`
- `cancel_token(&self) -> &CancellationToken`

### Step 5: Implement Communication Methods
- `send(&self, input: &str) -> Result<()>` - Uses Tmux::send_keys_enter
- `read_output(&self) -> Result<AgentOutput>` - Uses Tmux::capture_pane_plain + parsing

### Step 6: Implement Output Parsing
- `parse_output(content: &str) -> AgentOutput` - Detects questions, completion, errors

### Step 7: Implement last_commit
- `last_commit(&self) -> Result<Option<String>>` - Uses git operations

### Step 8: Update AgentPool Integration
- Update pool.spawn() to create handles with new fields
- Ensure event emission still works

## Implementation Checklist

- [ ] Define AgentOutput enum
- [ ] Add tests for AgentOutput
- [ ] Update AgentHandle struct with new fields
- [ ] Add constructor and accessor methods
- [ ] Add tests for struct and accessors
- [ ] Implement parse_output function
- [ ] Add tests for output parsing
- [ ] Implement send method
- [ ] Implement read_output method
- [ ] Implement last_commit method
- [ ] Add integration tests
- [ ] Update AgentPool to use new AgentHandle
- [ ] Run all tests and verify passing
- [ ] Run cargo build and verify success
