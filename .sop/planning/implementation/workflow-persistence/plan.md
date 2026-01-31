# Plan: Workflow Persistence via Git Notes

## Test Strategy

### Test Scenarios

#### 1. Save and Load Round-Trip
- **Given:** A new Workflow instance with all fields populated
- **When:** `save_workflow()` then `load_workflow()` with same ID
- **Then:** Loaded workflow matches original (id, name, prompt, phase, status, config, timestamps)

#### 2. Save Overwrites Existing
- **Given:** A workflow saved once
- **When:** Workflow is modified and saved again
- **Then:** `load_workflow()` returns the updated version

#### 3. List Multiple Workflows
- **Given:** 3 different workflows saved
- **When:** `list_workflows()` called
- **Then:** All 3 workflows returned, each matching originals

#### 4. Load Non-Existent Workflow
- **Given:** A WorkflowId that was never saved
- **When:** `load_workflow(id)` called
- **Then:** Returns `Ok(None)` (not an error)

#### 5. Delete Workflow
- **Given:** A saved workflow
- **When:** `delete_workflow(id)` called
- **Then:** Subsequent `load_workflow(id)` returns `None`

#### 6. Delete Non-Existent (Idempotent)
- **Given:** A WorkflowId that doesn't exist
- **When:** `delete_workflow(id)` called
- **Then:** Returns `Ok(())` without error

#### 7. List Empty (No Workflows)
- **Given:** Fresh repository with no workflows saved
- **When:** `list_workflows()` called
- **Then:** Returns empty `Vec<Workflow>`

#### 8. Git Notes Verification (Manual/Integration)
- **Given:** A saved workflow
- **When:** Running `git notes --ref=refs/notes/zen/workflows list`
- **Then:** The workflow commit is listed

## Implementation Plan

### Step 1: Add Workflow Persistence Methods to GitStateManager

Add the following methods to `src/state/manager.rs`:

1. **Helper: workflow_ref_name(id)** - Returns `workflows/{id}` for ref storage
2. **save_workflow(&self, workflow: &Workflow)**
   - Get current HEAD from `ops().current_head()`
   - Create or update ref at `workflows/{id}` pointing to HEAD
   - Set note on HEAD commit in `workflows` namespace
3. **load_workflow(&self, id: &WorkflowId)**
   - Read ref `workflows/{id}` to get commit SHA
   - If None, return Ok(None)
   - Get note from commit in `workflows` namespace
   - Deserialize and return
4. **list_workflows(&self)**
   - List refs with prefix `workflows/`
   - For each ref, load the workflow
   - Return collected workflows
5. **delete_workflow(&self, id: &WorkflowId)**
   - Get ref target commit SHA
   - Delete note from that commit
   - Delete the ref

### Step 2: Handle Edge Cases
- Ref exists but note doesn't (corrupted state) -> return None or error
- Note exists but ref doesn't (shouldn't happen with our implementation)

### Step 3: Add Tests
- Follow existing test patterns in manager.rs
- Use `setup_test_repo()` helper
- Test all acceptance criteria

## Implementation Checklist

- [ ] Add `save_workflow` method
- [ ] Add `load_workflow` method
- [ ] Add `list_workflows` method
- [ ] Add `delete_workflow` method
- [ ] Add test: save and load round-trip
- [ ] Add test: save overwrites existing
- [ ] Add test: list multiple workflows
- [ ] Add test: load non-existent returns None
- [ ] Add test: delete workflow
- [ ] Add test: delete non-existent is idempotent
- [ ] Add test: list empty returns empty vec
- [ ] Run `cargo test state` - all pass
- [ ] Run `cargo build` - no errors
