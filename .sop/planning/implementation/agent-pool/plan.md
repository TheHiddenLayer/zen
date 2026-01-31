# AgentPool Implementation Plan

## Test Scenarios

### 1. AgentEvent Tests
- [ ] test_agent_event_started_has_required_fields
- [ ] test_agent_event_completed_has_exit_code
- [ ] test_agent_event_failed_has_error
- [ ] test_agent_event_stuck_detected_has_duration
- [ ] test_agent_event_debug_format

### 2. AgentHandle Tests (Placeholder)
- [ ] test_agent_handle_creation
- [ ] test_agent_handle_has_id_and_status

### 3. AgentPool Creation Tests
- [ ] test_agent_pool_new
- [ ] test_agent_pool_new_with_capacity_zero
- [ ] test_agent_pool_starts_empty

### 4. Capacity Tests
- [ ] test_has_capacity_when_empty
- [ ] test_has_capacity_below_limit
- [ ] test_has_capacity_at_limit
- [ ] test_active_count_empty
- [ ] test_active_count_with_agents

### 5. Agent Lifecycle Tests
- [ ] test_spawn_returns_agent_id
- [ ] test_spawn_adds_to_pool
- [ ] test_spawn_sends_started_event
- [ ] test_spawn_respects_capacity
- [ ] test_terminate_removes_agent
- [ ] test_terminate_sends_event
- [ ] test_terminate_decreases_count
- [ ] test_terminate_nonexistent_returns_error
- [ ] test_get_returns_agent
- [ ] test_get_nonexistent_returns_none

## Implementation Steps

1. **Define AgentEvent enum**
   - Create variants: Started, Completed, Failed, StuckDetected
   - Add Debug derive

2. **Define AgentHandle placeholder**
   - Minimal struct with id and status fields

3. **Create AgentPool struct**
   - agents: HashMap<AgentId, AgentHandle>
   - max_concurrent: usize
   - event_tx: mpsc::Sender<AgentEvent>

4. **Implement AgentPool methods**
   - new() - constructor
   - spawn() - stub that creates handle, adds to map, sends event
   - terminate() - removes from map, returns Result
   - get() - returns Option<&AgentHandle>
   - active_count() - returns agents.len()
   - has_capacity() - returns active_count() < max_concurrent

5. **Export from orchestration module**
   - Add pool module
   - Export AgentPool, AgentEvent, AgentHandle

## File Structure
```
src/orchestration/
├── mod.rs          # Add: mod pool; pub use pool::*;
├── ai_human.rs     # Existing
└── pool.rs         # NEW: AgentPool, AgentEvent, AgentHandle
```
