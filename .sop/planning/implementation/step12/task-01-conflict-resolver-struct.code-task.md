# Task: Create ConflictResolver Structure

## Description
Create the ConflictResolver struct that handles merging task worktrees to the staging branch and detecting merge conflicts.

## Background
After implementation phase, each task's worktree needs to merge into a staging branch. Some merges may conflict if tasks modified the same files. The ConflictResolver manages this process.

## Reference Documentation
**Required:**
- Design: .sop/planning/design/detailed-design.md (Section 4.6 Conflict Resolver)

**Note:** You MUST read the detailed design document before beginning implementation.

## Technical Requirements
1. Create `src/orchestration/resolver.rs` with:
   ```rust
   pub struct ConflictResolver {
       git_ops: GitOps,
       agent_pool: Arc<RwLock<AgentPool>>,
   }

   pub enum MergeResult {
       Success { commit: String },
       Conflicts { files: Vec<ConflictFile> },
       Failed { error: String },
   }

   pub struct ConflictFile {
       pub path: PathBuf,
       pub ours: String,
       pub theirs: String,
       pub base: Option<String>,
   }
   ```
2. Implement basic structure and types
3. Add module to orchestration

## Dependencies
- GitOps from existing code
- AgentPool from Step 4

## Implementation Approach
1. Define MergeResult and ConflictFile types
2. Create ConflictResolver struct
3. Implement constructor with git_ops and agent_pool
4. Prepare for merge and resolution methods
5. Add to orchestration module exports
6. Add basic tests for types

## Acceptance Criteria

1. **Struct Creation**
   - Given git_ops and agent_pool
   - When ConflictResolver::new() is called
   - Then resolver is created with access to both

2. **MergeResult Types**
   - Given different merge outcomes
   - When represented as MergeResult
   - Then Success, Conflicts, or Failed variant is appropriate

3. **ConflictFile Data**
   - Given a merge conflict
   - When ConflictFile is created
   - Then ours, theirs, and base content are captured

4. **Module Export**
   - Given resolver module
   - When importing from orchestration
   - Then ConflictResolver is accessible

## Metadata
- **Complexity**: Low
- **Labels**: Merge, Conflict, Resolution, Structure
- **Required Skills**: Rust, git concepts, types
