# Task: Implement Documentation Phase

## Description
Implement the run_documentation_phase() method that executes /codebase-summary to update documentation with new components from the workflow.

## Background
Phase 5 is optional and runs /codebase-summary to update AGENTS.md, README.md, and other documentation to reflect the new code added during the workflow.

## Reference Documentation
**Required:**
- Design: .sop/planning/design/detailed-design.md (Section 4.2 run_documentation_phase code)

**Note:** You MUST read the detailed design document before beginning implementation.

## Technical Requirements
1. Add to SkillsOrchestrator:
   ```rust
   async fn run_documentation_phase(&self) -> Result<()> {
       if !self.state.read().await.config.update_docs {
           return Ok(());
       }
       let agent = self.agent_pool.write().await
           .spawn_for_skill("codebase-summary").await?;
       agent.send("/codebase-summary").await?;
       self.monitor_agent_output(&agent).await?;
   }
   ```
2. Make phase optional based on WorkflowConfig.update_docs
3. Run on staging branch after merge
4. Wire into execute() as Phase 5

## Dependencies
- SkillsOrchestrator from Step 6
- WorkflowConfig from Step 1
- monitor_agent_output from Step 6

## Implementation Approach
1. Check config.update_docs flag
2. If disabled, return Ok immediately
3. Spawn agent for codebase-summary
4. Run skill with default options
5. Monitor for completion
6. Commit documentation updates
7. Add tests for skip and execute paths

## Acceptance Criteria

1. **Phase Skip**
   - Given config.update_docs = false
   - When run_documentation_phase() is called
   - Then phase returns immediately without spawning agent

2. **Phase Execute**
   - Given config.update_docs = true
   - When run_documentation_phase() is called
   - Then /codebase-summary is executed

3. **Documentation Update**
   - Given new code in staging
   - When codebase-summary runs
   - Then AGENTS.md and other docs are updated

4. **Commit Creation**
   - Given documentation updates
   - When phase completes
   - Then changes are committed to staging

5. **Error Handling**
   - Given codebase-summary fails
   - When error occurs
   - Then workflow continues (documentation is optional)

## Metadata
- **Complexity**: Low
- **Labels**: Skills, Documentation, Phase 5, Optional
- **Required Skills**: Rust, async, skill integration
