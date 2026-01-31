# Context: Agent Output Monitor Loop (Task 6.3)

## Task Description

Create the shared `monitor_agent_output()` method that watches an agent's output, detects questions, and answers them via AIHumanProxy. This is the core interaction pattern for all skill phases.

## Requirements

1. **SkillResult struct** - Define a struct to hold skill completion data
2. **Monitor loop** - Implement async loop with polling interval
3. **Question detection** - Detect questions via AgentOutput parsing
4. **AI answer integration** - Use AIHumanProxy to answer questions
5. **Completion detection** - Exit loop when AgentOutput::Completed
6. **Error handling** - Return errors, don't swallow them
7. **Timeout protection** - Timeout for unresponsive agents

## Existing Patterns

### AgentOutput (src/orchestration/pool.rs)
```rust
pub enum AgentOutput {
    Text(String),
    Question(String),
    Completed,
    Error(String),
}
```

Parsing is already implemented with pattern detection for:
- Questions: `?`, "do you want", "would you like", etc.
- Completion: "task completed", "all tests pass", etc.
- Errors: "error:", "failed:", etc.

### AgentHandle (src/orchestration/pool.rs)
Key methods:
- `read_output(&self) -> Result<AgentOutput>` - synchronous
- `send(&self, input: &str) -> Result<()>` - synchronous
- `touch_activity(&mut self)` - updates last_activity
- `idle_duration(&self) -> Duration`

### AIHumanProxy (src/orchestration/ai_human.rs)
Key methods:
- `answer_question(&self, question: &str) -> String` - returns answer
- `needs_escalation(&self, question: &str) -> bool` - check if needs human

### Error Types (src/error.rs)
Existing relevant errors:
- `Timeout(Duration)` - for timeout handling

## Design Reference (Section 4.2)

The design doc shows:
```rust
loop {
    match agent.read_output().await? {
        AgentOutput::Question(q) => {
            let answer = self.ai_human.answer_question(&q).await;
            agent.send(&answer).await?;
        }
        AgentOutput::Completed => break,
        AgentOutput::Error(e) => return Err(e.into()),
        _ => continue,
    }
}
```

Note: Current `read_output()` is synchronous. We need to adapt for async with polling.

## Implementation Path

1. Add `SkillResult` struct to `src/orchestration/skills.rs`
2. Add `MonitorConfig` struct for polling/timeout settings
3. Implement `monitor_agent_output()` method on SkillsOrchestrator
4. Add new error variant `AgentMonitorTimeout` if needed
5. Add tests for all acceptance criteria

## Dependencies

- AgentHandle from Step 4 - COMPLETED
- AIHumanProxy from Step 3 - COMPLETED
- AgentOutput enum from Step 4 - COMPLETED
- SkillsOrchestrator from Step 6.1 - COMPLETED
