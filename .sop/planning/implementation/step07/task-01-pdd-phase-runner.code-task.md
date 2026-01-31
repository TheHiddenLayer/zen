# Task: Implement PDD Phase Runner

## Description
Implement the run_pdd_phase() method that executes the /pdd skill as Phase 1 of the workflow, using AIHumanProxy to answer clarification questions.

## Background
The /pdd skill transforms a rough idea into a detailed design and implementation plan. It asks clarifying questions one at a time. The PDD phase runner spawns an agent, invokes /pdd with the user's prompt, and monitors for completion.

## Reference Documentation
**Required:**
- Design: .sop/planning/design/detailed-design.md (Section 4.2 run_pdd_phase code)

**Note:** You MUST read the detailed design document before beginning implementation. The exact implementation is shown in Section 4.2.

## Technical Requirements
1. Add to SkillsOrchestrator:
   ```rust
   async fn run_pdd_phase(&self, prompt: &str) -> Result<PDDResult> {
       let agent = self.agent_pool.write().await
           .spawn_for_skill("pdd").await?;
       agent.send(&format!("/pdd\n\nrough_idea: {}", prompt)).await?;
       self.monitor_agent_output(&agent).await?;
       PDDResult::from_directory(&agent.worktree_path().join(".sop/planning"))
   }
   ```
2. Define `PDDResult` struct:
   - `design_path: PathBuf` (detailed-design.md)
   - `plan_path: PathBuf` (plan.md)
   - `research_dir: PathBuf`
3. Implement artifact validation and parsing
4. Wire into execute() as Phase 1

## Dependencies
- SkillsOrchestrator from Step 6
- AgentPool.spawn_for_skill() from Step 4
- monitor_agent_output() from Step 6

## Implementation Approach
1. Define PDDResult struct with artifact paths
2. Implement spawn_for_skill() in AgentPool for PDD
3. Implement run_pdd_phase() following design pattern
4. Add artifact validation (check files exist)
5. Wire into execute() Phase 1
6. Add tests with mock PDD output

## Acceptance Criteria

1. **PDD Skill Invocation**
   - Given a user prompt
   - When run_pdd_phase() is called
   - Then /pdd is invoked with the prompt as rough_idea

2. **Question Handling**
   - Given /pdd asks clarifying questions
   - When questions appear in output
   - Then AIHumanProxy answers them automatically

3. **Artifact Validation**
   - Given /pdd completes successfully
   - When PDDResult is created
   - Then detailed-design.md and plan.md paths are valid

4. **PDDResult Parsing**
   - Given .sop/planning/ directory with PDD output
   - When PDDResult::from_directory() is called
   - Then all artifact paths are populated

5. **Error Handling**
   - Given /pdd fails or times out
   - When error occurs
   - Then appropriate error is returned with context

## Metadata
- **Complexity**: Medium
- **Labels**: Skills, PDD, Phase 1, Orchestration
- **Required Skills**: Rust, async, file system, skill integration
