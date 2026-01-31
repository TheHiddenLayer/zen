# Task: Create AgentPool for Multi-Agent Management

## Description
Create the AgentPool struct that manages multiple concurrent agents, enforcing capacity limits and providing lifecycle operations.

## Background
Parallel task execution requires managing multiple Claude Code agents simultaneously. The AgentPool tracks all active agents, enforces the max_concurrent limit from WorkflowConfig, and emits events for status changes.

## Reference Documentation
**Required:**
- Design: .sop/planning/design/detailed-design.md (Section 4.3 Agent Management, AgentPool code)

**Note:** You MUST read the detailed design document before beginning implementation.

## Technical Requirements
1. Create `src/orchestration/pool.rs` with `AgentPool`:
   ```rust
   pub struct AgentPool {
       agents: HashMap<AgentId, AgentHandle>,
       max_concurrent: usize,
       event_tx: mpsc::Sender<AgentEvent>,
   }
   ```
2. Implement pool operations:
   - `new(max_concurrent: usize, event_tx: Sender<AgentEvent>) -> Self`
   - `spawn(&mut self, task: &Task, skill: &str) -> Result<AgentId>` (stub)
   - `terminate(&mut self, id: &AgentId) -> Result<()>`
   - `get(&self, id: &AgentId) -> Option<&AgentHandle>`
   - `active_count(&self) -> usize`
   - `has_capacity(&self) -> bool`
3. Define `AgentEvent` enum for pool events

## Dependencies
- AgentId, AgentStatus from task-01
- tokio mpsc channel
- AgentHandle from task-03 (implement placeholder)

## Implementation Approach
1. Define AgentEvent enum (Started, Completed, Failed, StuckDetected)
2. Create AgentPool struct with HashMap storage
3. Implement capacity checking (has_capacity, active_count)
4. Implement spawn stub (actual spawning in task-03)
5. Implement terminate with cleanup
6. Add tests for capacity enforcement

## Acceptance Criteria

1. **Capacity Enforcement**
   - Given AgentPool with max_concurrent=3
   - When 3 agents are active
   - Then has_capacity() returns false

2. **Agent Tracking**
   - Given agents spawned via the pool
   - When get(id) is called
   - Then the correct AgentHandle is returned

3. **Active Count**
   - Given 2 active agents in the pool
   - When active_count() is called
   - Then 2 is returned

4. **Terminate Cleanup**
   - Given an active agent
   - When terminate(id) is called
   - Then agent is removed and active_count decreases

5. **Event Emission**
   - Given an agent state change
   - When the change occurs
   - Then appropriate AgentEvent is sent via event_tx

## Metadata
- **Complexity**: Medium
- **Labels**: Agent, Pool, Orchestration, Concurrency
- **Required Skills**: Rust, HashMap, mpsc channels, async
