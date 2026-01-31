# Task: Implement AI-Assisted Conflict Resolution

## Description
Implement the resolve_conflicts() method that spawns a dedicated AI agent to resolve merge conflicts automatically.

## Background
When merges conflict, a specialized resolver agent is spawned with context about both versions. The agent uses the Edit tool to fix each conflicted file, then the resolution is verified before committing.

## Reference Documentation
**Required:**
- Design: .sop/planning/design/detailed-design.md (Section 4.6 resolve_conflicts, Section 6.3 Conflict Resolution pseudocode)

**Note:** You MUST read the detailed design document before beginning implementation.

## Technical Requirements
1. Add to ConflictResolver:
   - `resolve_conflicts(&self, conflicts: Vec<ConflictFile>) -> Result<()>`
2. Implementation:
   - Spawn dedicated resolver agent
   - Provide conflict context in prompt
   - Agent uses Edit tool to fix each file
   - Verify all conflicts resolved
   - Commit resolution
3. Format conflict context with ours/theirs/base content

## Dependencies
- ConflictResolver from task-01
- AgentPool for spawning resolver
- monitor_agent_output pattern

## Implementation Approach
1. Format conflicts into a clear prompt
2. Spawn resolver agent in merge worktree
3. Provide instructions to resolve each conflict
4. Monitor for completion
5. Verify no conflict markers remain
6. Create merge commit
7. Add tests with mock resolution

## Acceptance Criteria

1. **Resolver Spawn**
   - Given conflicts to resolve
   - When resolve_conflicts() is called
   - Then dedicated resolver agent is spawned

2. **Context Provision**
   - Given ConflictFile with ours/theirs content
   - When prompt is formatted
   - Then both versions are clearly presented

3. **Verification**
   - Given resolver claims completion
   - When verification runs
   - Then all conflict markers are confirmed removed

4. **Commit Creation**
   - Given successful resolution
   - When process completes
   - Then merge commit is created with resolution

5. **Resolution Failure**
   - Given resolver can't fix conflicts
   - When verification fails
   - Then appropriate error is returned for escalation

## Metadata
- **Complexity**: High
- **Labels**: Conflict, Resolution, AI, Agent
- **Required Skills**: Rust, git, prompt engineering
