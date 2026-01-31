# Context: Workflow Persistence via Git Notes

## Task Overview
Implement workflow persistence methods in `GitStateManager` that save and load `Workflow` structs using git notes under the `refs/notes/zen/workflows` namespace.

## Requirements

### Functional Requirements
1. **save_workflow(&self, workflow: &Workflow) -> Result<()>**
   - Serialize workflow to JSON
   - Create a ref at `refs/zen/workflows/{workflow-id}` pointing to HEAD
   - Attach workflow JSON as note to that ref's target commit

2. **load_workflow(&self, id: &WorkflowId) -> Result<Option<Workflow>>**
   - Read the ref `refs/zen/workflows/{workflow-id}`
   - If exists, retrieve note and deserialize
   - If not exists, return `Ok(None)`

3. **list_workflows(&self) -> Result<Vec<Workflow>>**
   - List all refs under `refs/zen/workflows/`
   - Load each workflow and return

4. **delete_workflow(&self, id: &WorkflowId) -> Result<()>**
   - Delete the note
   - Delete the ref

### Storage Schema (from detailed-design.md)
```
refs/zen/workflows/{workflow-id}  -> Points to workflow commit
refs/notes/zen/workflows/         -> Workflow metadata (JSON)
```

### Workflow JSON Format (Section 5.2)
```json
{
  "id": "wf-001",
  "name": "build-user-auth",
  "status": "running",
  "prompt": "Build user authentication...",
  "created_at": "2026-01-30T10:00:00Z",
  "started_at": "2026-01-30T10:00:05Z",
  "tasks": ["task-001", "task-002"],
  ...
}
```

## Existing Patterns

### GitStateManager (src/state/manager.rs)
- Composes `GitRefs`, `GitNotes`, `GitOps`
- Provides `refs()`, `notes()`, `ops()` accessors
- Tests use `tempfile::TempDir` for isolated repos

### GitRefs (src/git_refs.rs)
- `create_ref(name, target)` - Creates ref at `refs/zen/{name}`
- `read_ref(name)` - Returns `Option<String>` commit SHA
- `delete_ref(name)` - Idempotent delete
- `list_refs(prefix)` - Lists refs matching prefix

### GitNotes (src/git_notes.rs)
- `set_note<T>(commit, namespace, data)` - JSON serializes and attaches
- `get_note<T>(commit, namespace)` - Returns `Option<T>`
- `delete_note(commit, namespace)` - Idempotent delete
- `list_notes(namespace)` - Lists commit SHAs with notes
- Namespace: `refs/notes/zen/{namespace}`

### Workflow Types (src/workflow/types.rs)
- `Workflow` - Main struct with id, name, prompt, phase, status, config, timestamps, task_ids
- `WorkflowId` - UUID-based, implements `Display`, `FromStr`, serde traits
- Already implements `Serialize` and `Deserialize`

## Implementation Approach

1. Use `refs/zen/workflows/{id}` for ref storage
2. Use `workflows` as the notes namespace (becomes `refs/notes/zen/workflows`)
3. For save: get HEAD commit, create/update ref, set note on ref target
4. For load: read ref to get commit, get note from commit
5. For list: list refs with `workflows/` prefix, load each
6. For delete: delete note, then delete ref

## Dependencies
- `serde_json` for JSON serialization (already in project)
- `GitRefs` for ref operations
- `GitNotes` for note operations
- `GitOps` for getting current HEAD
