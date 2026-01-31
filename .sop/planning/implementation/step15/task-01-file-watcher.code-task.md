# Task: Implement File Watcher for Plan Changes

## Description
Create a file watcher that monitors .sop/planning/ for changes to plan or design files, enabling reactive replanning during workflow execution.

## Background
The reactive planning system auto-adapts when plans change. If a user edits plan.md or detailed-design.md during execution, the system detects this and triggers replanning.

## Reference Documentation
**Required:**
- Design: .sop/planning/design/detailed-design.md (Section 4.5 Reactive Planner)

**Note:** You MUST read the detailed design document before beginning implementation.

## Technical Requirements
1. Create `src/orchestration/planner.rs` with:
   ```rust
   pub struct ReactivePlanner {
       dag: Arc<RwLock<TaskDAG>>,
       watch_paths: Vec<PathBuf>,
       watcher: notify::RecommendedWatcher,
       event_tx: mpsc::Sender<PlanEvent>,
   }

   pub enum PlanEvent {
       FileChanged { path: PathBuf },
       ReplanTriggered,
       TasksAdded { tasks: Vec<Task> },
       TasksCancelled { tasks: Vec<TaskId> },
   }
   ```
2. Use notify crate for file watching
3. Watch .sop/planning/ directory
4. Debounce rapid changes (1 second window)

## Dependencies
- notify crate for file watching
- TaskDAG from Step 8
- Add notify to Cargo.toml

## Implementation Approach
1. Add notify = "6" to Cargo.toml
2. Define PlanEvent enum
3. Create ReactivePlanner struct
4. Implement file watcher setup
5. Implement debouncing logic
6. Emit PlanEvent on detected changes
7. Add tests with mock file changes

## Acceptance Criteria

1. **Watch Setup**
   - Given ReactivePlanner is created
   - When .sop/planning/ is specified
   - Then watcher is configured for that path

2. **Change Detection**
   - Given watcher is active
   - When plan.md is modified
   - Then FileChanged event is emitted

3. **Debouncing**
   - Given 5 rapid changes within 1 second
   - When changes are detected
   - Then only 1 FileChanged event is emitted

4. **Relevant Files Only**
   - Given a change to unrelated file
   - When watcher processes it
   - Then no event is emitted (filtered)

5. **Error Handling**
   - Given file watching fails
   - When error occurs
   - Then error is logged but doesn't crash system

## Metadata
- **Complexity**: Medium
- **Labels**: Reactive, Watcher, Files, Planning
- **Required Skills**: Rust, notify crate, async, debouncing
