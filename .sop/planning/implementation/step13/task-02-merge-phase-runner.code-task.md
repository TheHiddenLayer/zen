# Task: Implement Merge Phase Runner

## Description
Implement the run_merge_phase() method that orchestrates merging all completed task worktrees into the staging branch.

## Background
Phase 4 takes all ImplResults from Phase 3 and merges each task's worktree into a shared staging branch. Conflicts are resolved by the resolver agent. This creates a unified staging branch for user review.

## Reference Documentation
**Required:**
- Design: .sop/planning/design/detailed-design.md (Section 4.2 run_merge_phase code)

**Note:** You MUST read the detailed design document before beginning implementation.

## Technical Requirements
1. Add to SkillsOrchestrator:
   ```rust
   async fn run_merge_phase(&self, results: &[ImplResult]) -> Result<()> {
       let resolver = ConflictResolver::new(...);
       let staging = format!("zen/staging/{}", self.workflow_id);

       // Create staging branch from base
       self.git_ops.create_branch(&staging, base_commit)?;

       for result in results {
           match resolver.merge(&result.worktree, &staging).await? {
               MergeResult::Success { .. } => continue,
               MergeResult::Conflicts { files } => {
                   resolver.resolve_conflicts(files).await?;
               }
               MergeResult::Failed { error } => return Err(...),
           }
       }
   }
   ```
2. Create staging branch from workflow base
3. Merge each worktree in sequence
4. Handle conflicts via resolver
5. Wire into execute() as Phase 4

## Dependencies
- ConflictResolver from Step 12
- ImplResult from Step 11
- GitOps for branch operations

## Implementation Approach
1. Create staging branch name from workflow id
2. Create branch from base commit
3. Iterate through ImplResults
4. Merge each worktree using resolver
5. Handle conflicts automatically
6. Complete when all merged
7. Add tests for clean and conflicted merges

## Acceptance Criteria

1. **Staging Branch Creation**
   - Given workflow with id wf-123
   - When merge phase starts
   - Then branch zen/staging/wf-123 is created

2. **Sequential Merging**
   - Given 5 ImplResults
   - When merge phase runs
   - Then each worktree is merged in order

3. **Conflict Handling**
   - Given a worktree that conflicts
   - When merge produces conflicts
   - Then resolver.resolve_conflicts() is called

4. **All Merged**
   - Given all worktrees merge successfully
   - When phase completes
   - Then staging branch contains all changes

5. **Failure Handling**
   - Given unresolvable conflict
   - When resolution fails
   - Then phase returns error for user escalation

## Metadata
- **Complexity**: Medium
- **Labels**: Merge, Phase 4, Orchestration, Integration
- **Required Skills**: Rust, git branching, async
