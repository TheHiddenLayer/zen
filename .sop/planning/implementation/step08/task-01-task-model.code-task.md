# Task: Create Task Data Model

## Description
Create the Task struct and related types that represent individual work items in the execution DAG. Tasks are the atomic units of work assigned to agents.

## Background
After /code-task-generator creates .code-task.md files, each becomes a Task in the system. Tasks track status, assignment, worktree location, and results. They're the nodes in the execution DAG.

## Reference Documentation
**Required:**
- Design: .sop/planning/design/detailed-design.md (Section 4.3 Task and DAG Models, Section 5.3 Task State JSON)

**Note:** You MUST read the detailed design document before beginning implementation.

## Technical Requirements
1. Create `src/core/mod.rs` with module exports
2. Create `src/core/task.rs` with:
   ```rust
   pub struct TaskId(Uuid);

   pub enum TaskStatus {
       Pending,
       Ready,
       Running,
       Completed,
       Failed { error: String },
       Blocked { reason: String },
   }

   pub struct Task {
       pub id: TaskId,
       pub name: String,
       pub description: String,
       pub status: TaskStatus,
       pub worktree_path: Option<PathBuf>,
       pub branch_name: Option<String>,
       pub agent_id: Option<AgentId>,
       pub created_at: DateTime<Utc>,
       pub started_at: Option<DateTime<Utc>>,
       pub completed_at: Option<DateTime<Utc>>,
       pub commit_hash: Option<String>,
   }
   ```
3. Implement lifecycle methods: start(), complete(), fail()
4. Add serialization for git notes persistence
5. Add `pub mod core;` to `src/lib.rs`

## Dependencies
- AgentId from Step 4
- uuid, chrono, serde

## Implementation Approach
1. Create core module structure
2. Define TaskId newtype
3. Define TaskStatus enum with all variants
4. Define Task struct with all fields
5. Implement lifecycle methods
6. Add serialization tests matching JSON schema
7. Wire up to lib.rs

## Acceptance Criteria

1. **Task Creation**
   - Given a task name and description
   - When `Task::new(name, description)` is called
   - Then task is created with Pending status and generated id

2. **Task Lifecycle**
   - Given a Pending task
   - When start() then complete() are called
   - Then status transitions correctly and timestamps are set

3. **Task Failure**
   - Given a Running task
   - When fail(error) is called
   - Then status becomes Failed with error message

4. **Serialization**
   - Given a Task instance
   - When serialized to JSON
   - Then format matches Section 5.3 schema

5. **Module Integration**
   - Given core module is complete
   - When `cargo build` is run
   - Then project compiles with `pub mod core;` in lib.rs

## Metadata
- **Complexity**: Low
- **Labels**: Core, Task, Model, Foundation
- **Required Skills**: Rust, serde, chrono, newtype pattern
