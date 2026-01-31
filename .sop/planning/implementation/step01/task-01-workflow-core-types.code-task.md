# Task: Create Workflow Core Types

## Description
Create the foundational type definitions for the workflow system including WorkflowId, WorkflowPhase enum, and WorkflowStatus enum. These types form the basis of the entire orchestration system.

## Background
Zen v2 transforms from a single-session manager to a parallel multi-agent orchestrator. The workflow system needs strongly-typed identifiers and enums to track workflow lifecycle and phases. These types must support serialization for git-native state persistence.

## Reference Documentation
**Required:**
- Design: .sop/planning/design/detailed-design.md (Section 4.2 Skills Orchestrator, Section 5 Data Models)

**Note:** You MUST read the detailed design document before beginning implementation. Pay special attention to the WorkflowPhase enum and its relationship to the Skills-driven workflow.

## Technical Requirements
1. Create `src/workflow/mod.rs` with module exports
2. Create `src/workflow/types.rs` with core type definitions
3. Implement `WorkflowId` as UUID-based newtype (similar to existing `SessionId` pattern)
4. Implement `WorkflowPhase` enum with variants: Planning, TaskGeneration, Implementation, Merging, Documentation, Complete
5. Implement `WorkflowStatus` enum with variants: Pending, Running, Paused, Completed, Failed
6. Derive `Serialize`, `Deserialize`, `Debug`, `Clone`, `PartialEq`, `Eq`, `Hash` as appropriate
7. Implement `Display` for human-readable output

## Dependencies
- serde (already in Cargo.toml)
- uuid (already in Cargo.toml)
- Existing `SessionId` pattern in `src/session.rs` for reference

## Implementation Approach
1. Study the existing `SessionId` implementation in `src/session.rs` for the newtype pattern
2. Create the workflow module directory structure
3. Define `WorkflowId` with UUID generation and short form display
4. Define `WorkflowPhase` enum matching the 5-phase Skills workflow
5. Define `WorkflowStatus` enum for lifecycle tracking
6. Add comprehensive unit tests for serialization and equality

## Acceptance Criteria

1. **WorkflowId Generation**
   - Given a new workflow is created
   - When `WorkflowId::new()` is called
   - Then a unique UUID-based identifier is generated with short form display

2. **WorkflowPhase Ordering**
   - Given the WorkflowPhase enum
   - When comparing phases
   - Then Planning < TaskGeneration < Implementation < Merging < Documentation < Complete

3. **Serialization Round-Trip**
   - Given any WorkflowId, WorkflowPhase, or WorkflowStatus value
   - When serialized to JSON and deserialized back
   - Then the original value is preserved exactly

4. **Unit Test Coverage**
   - Given the workflow types implementation
   - When running `cargo test workflow`
   - Then all type creation, serialization, and comparison tests pass

## Metadata
- **Complexity**: Low
- **Labels**: Foundation, Types, Serialization, Workflow
- **Required Skills**: Rust, serde, UUID, newtype pattern
