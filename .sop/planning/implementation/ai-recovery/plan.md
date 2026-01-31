# Plan: AI-Driven Recovery Implementation

## Test Strategy

### Test Scenarios

#### 1. Retry Tracking Tests
- `test_retry_tracker_new_task_has_zero_retries` - New tasks start with 0 retries
- `test_retry_tracker_increment` - Incrementing retry count works correctly
- `test_retry_tracker_get_retries` - Getting retry count returns correct value
- `test_retry_tracker_reset` - Resetting clears retry count
- `test_retry_tracker_multiple_tasks` - Multiple tasks tracked independently

#### 2. Recovery Action Parsing Tests
- `test_parse_recovery_action_restart` - Parse "restart" response
- `test_parse_recovery_action_decompose` - Parse "decompose" with subtasks
- `test_parse_recovery_action_escalate` - Parse "escalate" with message
- `test_parse_recovery_action_abort` - Parse "abort" response
- `test_parse_recovery_action_default_restart` - Unknown response defaults to restart

#### 3. Determine Recovery Tests
- `test_determine_recovery_transient_error_returns_restart` - Transient errors trigger restart
- `test_determine_recovery_max_retries_returns_escalate` - Max retries triggers escalation
- `test_determine_recovery_complex_task_returns_decompose` - Complex tasks trigger decompose
- `test_determine_recovery_builds_proper_prompt` - Prompt includes agent output, task, retries

#### 4. Execute Recovery Tests
- `test_execute_recovery_restart_terminates_agent` - Restart terminates agent
- `test_execute_recovery_escalate_emits_event` - Escalate emits health event
- `test_execute_recovery_abort_terminates_and_marks_failed` - Abort terminates agent
- `test_execute_recovery_decompose_creates_subtasks` - Decompose creates new tasks

#### 5. Integration Tests
- `test_recovery_flow_transient_error` - Full flow for transient error
- `test_recovery_flow_max_retries_exceeded` - Full flow when retries exceeded

## Implementation Plan

### Phase 1: Retry Tracking
Add `RetryTracker` struct to track retry counts per task.

### Phase 2: Recovery Prompt Builder
Implement `build_recovery_prompt()` to create AI prompt with context.

### Phase 3: Response Parser
Implement `parse_recovery_action()` to convert AI response to RecoveryAction.

### Phase 4: determine_recovery()
Implement async method using ClaudeHeadless for AI analysis.

### Phase 5: execute_recovery()
Implement action execution for each RecoveryAction variant.

### Phase 6: Integration
Wire everything together and add integration tests.

## Implementation Checklist
- [ ] Add RetryTracker struct
- [ ] Implement retry tracking methods (increment, get, reset)
- [ ] Add retry_tracker field to HealthMonitor
- [ ] Implement build_recovery_prompt()
- [ ] Implement parse_recovery_action()
- [ ] Implement determine_recovery() with AI integration
- [ ] Implement execute_recovery() for Restart
- [ ] Implement execute_recovery() for Escalate
- [ ] Implement execute_recovery() for Abort
- [ ] Implement execute_recovery() for Decompose
- [ ] Add tests for all functionality
- [ ] Run cargo test to verify
- [ ] Run cargo build to verify compilation
