# Plan: AgentId and AgentStatus Types

## Test Strategy

### AgentId Tests
1. `test_agent_id_new` - Each call generates unique ID
2. `test_agent_id_default` - Default impl creates valid ID
3. `test_agent_id_short` - Returns 8-character prefix
4. `test_agent_id_display` - Full UUID string display
5. `test_agent_id_from_str` - Parse from string
6. `test_agent_id_from_str_invalid` - Reject invalid strings
7. `test_agent_id_serialization` - JSON round-trip
8. `test_agent_id_equality` - Same UUID equals
9. `test_agent_id_hash` - Works in HashSet/HashMap

### AgentStatus Tests
1. `test_agent_status_idle_default` - Default is Idle
2. `test_agent_status_display_idle` - "idle" display
3. `test_agent_status_display_running` - "running (task: ...)" display
4. `test_agent_status_display_stuck` - "stuck: ..." display
5. `test_agent_status_display_failed` - "failed: ..." display
6. `test_agent_status_display_terminated` - "terminated" display
7. `test_agent_status_running_with_task` - Can access task_id
8. `test_agent_status_stuck_fields` - Can access since and reason
9. `test_agent_status_failed_error` - Can access error string
10. `test_agent_status_serialization` - Serialize/deserialize (Stuck skipped due to Instant)

## Implementation Plan

### Step 1: Add AgentId
- Follow SessionId/WorkflowId pattern exactly
- Derive traits: Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize
- Implement: new(), short(), Default, Display, FromStr

### Step 2: Add AgentStatus enum
- All 5 variants as specified
- Default = Idle
- Display implementation with human-readable output
- Partial serialization (skip Stuck due to Instant)

### Step 3: Re-export from workflow::types
- Import TaskId for Running variant

## Checklist
- [ ] Add AgentId type
- [ ] Add AgentStatus enum
- [ ] Implement Display for AgentStatus
- [ ] Add serialization support
- [ ] Write all tests
- [ ] Verify tests pass
