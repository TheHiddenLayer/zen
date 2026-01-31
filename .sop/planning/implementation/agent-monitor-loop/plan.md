# Implementation Plan: Agent Output Monitor Loop

## Test Scenarios

### 1. SkillResult Tests
- [ ] SkillResult can be constructed with success status
- [ ] SkillResult can be constructed with error status
- [ ] SkillResult is Debug and Clone

### 2. MonitorConfig Tests
- [ ] MonitorConfig has sensible defaults (100ms poll, 10min timeout)
- [ ] MonitorConfig can be customized

### 3. Question Detection and Response
- [ ] Given agent outputs "Which database should we use?"
- [ ] When monitor loop processes it
- [ ] Then AIHumanProxy generates answer and sends to agent

### 4. Completion Detection
- [ ] Given agent outputs skill completion marker (AgentOutput::Completed)
- [ ] When monitor loop processes it
- [ ] Then loop exits with SkillResult::Success

### 5. Error Handling
- [ ] Given agent outputs an error (AgentOutput::Error)
- [ ] When monitor loop processes it
- [ ] Then error is returned, not swallowed

### 6. Timeout Protection
- [ ] Given agent produces no output for timeout period
- [ ] When timeout is exceeded
- [ ] Then timeout error is returned

### 7. Continuous Monitoring
- [ ] Given agent outputs multiple questions
- [ ] When each question appears
- [ ] Then each is answered and loop continues

### 8. Activity Tracking
- [ ] Monitor loop updates agent last_activity on each output
- [ ] Idle duration can be tracked through agent handle

## Implementation Tasks

### Phase 1: Data Structures
- [ ] Add `SkillResult` struct with status and optional data
- [ ] Add `MonitorConfig` struct with poll_interval and timeout

### Phase 2: Monitor Loop Implementation
- [ ] Implement `monitor_agent_output()` method
- [ ] Add polling with configurable interval
- [ ] Add timeout handling
- [ ] Integrate with AIHumanProxy for question answering

### Phase 3: Integration
- [ ] Export new types from orchestration module
- [ ] Add documentation

## Implementation Details

### SkillResult Structure
```rust
pub struct SkillResult {
    pub success: bool,
    pub output: Option<String>,
    pub questions_answered: usize,
    pub duration: Duration,
}
```

### MonitorConfig Structure
```rust
pub struct MonitorConfig {
    pub poll_interval: Duration,
    pub timeout: Duration,
}
```

### Monitor Loop Pattern
```rust
async fn monitor_agent_output(
    &self,
    agent: &AgentHandle,
    config: &MonitorConfig,
) -> Result<SkillResult>
```

The loop will:
1. Start timer for timeout tracking
2. Poll agent output at poll_interval
3. Match on AgentOutput variant
4. For Question: answer via AIHumanProxy, send response
5. For Completed: exit with success
6. For Error: return error
7. For Text: continue polling
8. Check timeout on each iteration
