# Task: Create Performance Tests

## Description
Create performance tests that verify the system meets performance requirements: 60 FPS TUI, low scheduling overhead, and reasonable memory usage.

## Background
Performance is critical for user experience. The TUI must remain responsive even during heavy workflow execution. Scheduling overhead must not bottleneck parallel execution.

## Reference Documentation
**Required:**
- Design: .sop/planning/design/detailed-design.md (Section 8 Testing Strategy)
- Research: .sop/planning/research/existing-code.md (Architecture tests section)

**Note:** You MUST read both documents to understand existing performance patterns.

## Technical Requirements
1. Add to `tests/integration/`:
   - `performance.rs` - Performance benchmarks
2. Test scenarios:
   - TUI renders at 60 FPS during execution
   - Scheduler overhead < 10ms per dispatch
   - Memory usage < 100MB for 10 parallel agents
   - State snapshots don't block render thread
3. Use criterion or built-in benchmarks

## Dependencies
- criterion crate for benchmarking (optional)
- All workflow components

## Implementation Approach
1. Create render performance test
2. Measure frame time during active workflow
3. Create scheduler overhead test
4. Measure time to dispatch ready tasks
5. Create memory usage test
6. Monitor heap during parallel execution
7. Document performance baselines
8. Add to CI with thresholds

## Acceptance Criteria

1. **60 FPS Render**
   - Given active workflow with 4 agents
   - When TUI renders
   - Then frame time < 16.67ms (60 FPS)

2. **Scheduler Overhead**
   - Given 10 tasks in DAG
   - When ready_tasks() is called
   - Then computation < 10ms

3. **Memory Baseline**
   - Given 10 parallel agents
   - When at peak execution
   - Then memory usage < 100MB

4. **No Render Blocking**
   - Given heavy state updates
   - When render thread runs
   - Then no blocking occurs (lock-free)

5. **CI Integration**
   - Given performance tests
   - When CI runs
   - Then failures are reported if thresholds exceeded

## Metadata
- **Complexity**: Medium
- **Labels**: Testing, Performance, Benchmark
- **Required Skills**: Rust, benchmarking, profiling
