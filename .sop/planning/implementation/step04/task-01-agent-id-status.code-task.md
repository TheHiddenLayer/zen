# Task: Add AgentId and AgentStatus Types

## Description
Enhance the Agent module with AgentId (UUID-based identifier) and AgentStatus enum to support multiple concurrent agents with lifecycle tracking.

## Background
The current Agent struct is simple and doesn't support multiple instances. For parallel execution, each agent needs a unique identifier and status tracking. This builds on the existing Agent abstraction.

## Reference Documentation
**Required:**
- Design: .sop/planning/design/detailed-design.md (Section 4.3 Agent Management)
- Research: .sop/planning/research/existing-code.md (Section on src/agent.rs)

**Note:** You MUST read both documents to understand existing Agent implementation and target design.

## Technical Requirements
1. Update `src/agent.rs` with new types:
   - `AgentId` - UUID-based newtype (similar to SessionId, WorkflowId)
   - `AgentStatus` enum: Idle, Running { task_id: TaskId }, Stuck { since: Instant, reason: String }, Failed { error: String }, Terminated
2. Add task association: `current_task: Option<TaskId>` (use placeholder TaskId for now)
3. Implement `Display` for AgentStatus
4. Add serialization support for persistence

## Dependencies
- Existing Agent struct in `src/agent.rs`
- uuid crate (already in Cargo.toml)
- TaskId placeholder (will be properly defined in Step 8)

## Implementation Approach
1. Study existing Agent implementation
2. Add AgentId newtype following SessionId pattern
3. Add AgentStatus enum with all variants
4. Keep existing Agent struct unchanged (it's the config)
5. Prepare for AgentHandle in task-03 which will use these types
6. Add unit tests for new types

## Acceptance Criteria

1. **AgentId Generation**
   - Given a new agent is spawned
   - When `AgentId::new()` is called
   - Then a unique UUID-based identifier is generated

2. **AgentStatus Transitions**
   - Given an agent in Idle status
   - When assigned a task
   - Then status can transition to Running { task_id }

3. **Stuck Detection Preparation**
   - Given AgentStatus::Stuck variant
   - When created with since and reason
   - Then both fields are accessible for health monitoring

4. **Display Implementation**
   - Given any AgentStatus variant
   - When formatted with Display
   - Then a human-readable status string is produced

5. **Serialization**
   - Given AgentId and AgentStatus values
   - When serialized and deserialized
   - Then values are preserved correctly

## Metadata
- **Complexity**: Low
- **Labels**: Agent, Types, Foundation
- **Required Skills**: Rust, enums, newtype pattern
