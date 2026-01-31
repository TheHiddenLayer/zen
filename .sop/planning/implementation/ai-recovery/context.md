# Context: AI-Driven Recovery Implementation

## Task Overview
Implement `determine_recovery()` and `execute_recovery()` methods in HealthMonitor that use AI judgment to decide and execute the best recovery action for stuck or failing agents.

## Requirements

### Functional Requirements
1. **determine_recovery()**: Async method that analyzes agent state and returns appropriate RecoveryAction
2. **execute_recovery()**: Execute the recovery action (restart, reassign, decompose, escalate, abort)
3. **Retry Tracking**: Track retry counts per task to inform recovery decisions
4. **AI Analysis**: Use ClaudeHeadless to analyze agent output, task description, error patterns

### Acceptance Criteria
1. Restart decision for transient errors
2. Decompose decision for complex tasks
3. Escalate when max retries exceeded
4. Execute recovery actions (terminate agent, re-queue task)
5. Track retries and consider in decisions

## Existing Implementation

### RecoveryAction Enum (Already Exists)
Located in `src/orchestration/health.rs`:
- `Restart` - Restart the agent
- `Reassign { to_agent: AgentId }` - Reassign to another agent
- `Decompose { into_tasks: Vec<String> }` - Break into smaller tasks
- `Escalate { message: String }` - Escalate to user
- `Abort` - Abort the task

### HealthMonitor (Already Exists)
- `check_agent()` - Detects stuck agents based on idle time and output patterns
- `check_all()` - Checks all agents in pool
- `is_healthy()` - Returns whether agent is healthy

### ClaudeHeadless (Available)
- `execute()` - Execute prompt and get response
- `execute_with_model()` - Execute with specific model (e.g., haiku for fast responses)

### AgentPool (Available)
- `terminate()` - Terminate an agent
- `spawn()` - Spawn new agent for task

## Implementation Patterns

### AI Prompt Pattern (from AIHumanProxy)
```rust
let prompt = format!(
    "Analyze the situation and provide a recovery recommendation...",
    context
);
let response = claude.execute_with_model(&prompt, cwd, "haiku").await?;
```

### Response Parsing Pattern
Parse structured AI responses into enum variants by looking for keywords or JSON.

## Dependencies
- `ClaudeHeadless` for AI analysis
- `AgentPool` for agent lifecycle operations
- `AgentHandle` for agent state
- `HealthConfig` for max_retries setting

## Implementation Path
1. Add retry tracking via `HashMap<TaskId, u32>` in HealthMonitor
2. Implement `determine_recovery()` with AI prompt
3. Implement `parse_recovery_action()` to parse AI response
4. Implement `execute_recovery()` for each action type
5. Add comprehensive tests with mock responses
