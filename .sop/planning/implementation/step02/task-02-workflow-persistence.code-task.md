# Task: Implement Workflow Persistence via Git Notes

## Description
Add workflow persistence methods to GitStateManager that save and load Workflow structs using git notes under the refs/notes/zen/workflows namespace.

## Background
Git notes allow attaching arbitrary data to commits. By storing workflow state as JSON in git notes, we get versioned, portable state that travels with the repository. The namespace refs/notes/zen/workflows isolates workflow data from other zen state.

## Reference Documentation
**Required:**
- Design: .sop/planning/design/detailed-design.md (Section 5.1 Git State Schema, Section 5.2 Workflow State JSON)

**Note:** You MUST read the detailed design document before beginning implementation. The JSON schema shows the expected format.

## Technical Requirements
1. Implement workflow persistence methods in GitStateManager:
   - `save_workflow(&self, workflow: &Workflow) -> Result<()>`
   - `load_workflow(&self, id: &WorkflowId) -> Result<Option<Workflow>>`
   - `list_workflows(&self) -> Result<Vec<Workflow>>`
   - `delete_workflow(&self, id: &WorkflowId) -> Result<()>`
2. Use namespace `refs/notes/zen/workflows` for storage
3. Store workflow JSON attached to a dedicated workflow commit/ref
4. Handle concurrent workflow storage (multiple active workflows)

## Dependencies
- GitStateManager from task-01
- Workflow types from Step 1
- GitNotes module methods

## Implementation Approach
1. Create a ref for each workflow: `refs/zen/workflows/{workflow-id}`
2. Attach workflow JSON as a note to the ref's target commit
3. Implement save: create/update ref and note
4. Implement load: read ref, retrieve note, deserialize
5. Implement list: iterate refs/zen/workflows/*, load each
6. Add comprehensive tests with temporary git repos

## Acceptance Criteria

1. **Save and Load Round-Trip**
   - Given a Workflow instance
   - When saved via `save_workflow()` and loaded via `load_workflow()`
   - Then the loaded workflow matches the original exactly

2. **List Multiple Workflows**
   - Given 3 workflows saved to the manager
   - When `list_workflows()` is called
   - Then all 3 workflows are returned

3. **Load Non-Existent**
   - Given a WorkflowId that doesn't exist
   - When `load_workflow(id)` is called
   - Then `Ok(None)` is returned (not an error)

4. **Delete Workflow**
   - Given a saved workflow
   - When `delete_workflow(id)` is called
   - Then subsequent load returns None

5. **Git Notes Verification**
   - Given a saved workflow
   - When running `git notes --ref=refs/notes/zen/workflows list`
   - Then the workflow data is visible in git

## Metadata
- **Complexity**: Medium
- **Labels**: Git, State Management, Persistence, Workflow
- **Required Skills**: Rust, git2, git notes, JSON serialization
