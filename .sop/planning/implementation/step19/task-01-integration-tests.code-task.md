# Task: Create Integration Test Suite

## Description
Create comprehensive integration tests that exercise the full workflow from prompt to completion, including parallel execution and conflict resolution.

## Background
Integration tests verify that all components work together correctly. They catch issues that unit tests miss, especially around async behavior and git operations.

## Reference Documentation
**Required:**
- Design: .sop/planning/design/detailed-design.md (Section 8 Testing Strategy)

**Note:** You MUST read the detailed design document before beginning implementation.

## Technical Requirements
1. Create `tests/integration/` directory:
   - `workflow_e2e.rs` - Full workflow execution
   - `parallel_agents.rs` - Parallel execution correctness
   - `conflict_resolution.rs` - Merge conflict handling
   - `recovery.rs` - Health monitor and recovery
2. Create test fixtures:
   - Temporary git repos
   - Mock Claude responses
   - Predefined task sets
3. Test scenarios from design doc

## Dependencies
- All workflow components
- tempfile crate for temp repos
- tokio-test for async testing

## Implementation Approach
1. Create test fixture helpers
2. Create mock Claude responder
3. Implement workflow_e2e test
4. Implement parallel_agents test
5. Implement conflict_resolution test
6. Implement recovery test
7. Add test for headless mode
8. Ensure CI-friendly (no real Claude calls)

## Acceptance Criteria

1. **E2E Happy Path**
   - Given mock workflow with 3 tasks
   - When zen run executes
   - Then all tasks complete and merge succeeds

2. **Parallel Execution**
   - Given 4 independent tasks
   - When scheduler runs
   - Then 4 agents run concurrently

3. **Conflict Resolution**
   - Given 2 tasks modifying same file
   - When merge phase runs
   - Then resolver handles conflict

4. **Recovery Test**
   - Given simulated stuck agent
   - When health monitor detects
   - Then recovery action is executed

5. **No Real Claude**
   - Given integration tests
   - When CI runs them
   - Then no actual Claude API calls are made

## Metadata
- **Complexity**: High
- **Labels**: Testing, Integration, E2E, CI
- **Required Skills**: Rust, testing, mocking, async
