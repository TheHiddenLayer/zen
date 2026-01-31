# Task: Create SkillsOrchestrator Structure

## Description
Create the SkillsOrchestrator struct that coordinates the full workflow phases, composing AIHumanProxy, AgentPool, and ClaudeHeadless into the central orchestration engine.

## Background
The SkillsOrchestrator is the heart of Zen v2. It drives the 5-phase workflow (PDD -> TaskGen -> Implementation -> Merge -> Docs) by spawning agents for each skill and using AIHumanProxy to answer questions autonomously.

## Reference Documentation
**Required:**
- Design: .sop/planning/design/detailed-design.md (Section 4.2 Skills Orchestrator code)

**Note:** You MUST read the detailed design document before beginning implementation. Section 4.2 has the full struct definition.

## Technical Requirements
1. Create `src/orchestration/skills.rs` with `SkillsOrchestrator`:
   ```rust
   pub struct SkillsOrchestrator {
       ai_human: AIHumanProxy,
       agent_pool: Arc<RwLock<AgentPool>>,
       state: Arc<RwLock<WorkflowState>>,
       claude: ClaudeHeadless,
   }
   ```
2. Implement constructor:
   - `new(config: WorkflowConfig, repo_path: &Path) -> Result<Self>`
3. Implement main execution skeleton:
   - `execute(&self, prompt: &str) -> Result<WorkflowResult>`
   - Stub out phase methods (will be implemented in later steps)
4. Define `WorkflowResult` for completion status

## Dependencies
- AIHumanProxy from Step 3
- AgentPool from Step 4
- ClaudeHeadless from Step 5
- WorkflowState from Step 1

## Implementation Approach
1. Define WorkflowResult struct
2. Create SkillsOrchestrator with all dependencies
3. Implement execute() skeleton calling phase stubs
4. Add logging for phase transitions
5. Wire up module exports
6. Add basic tests for construction

## Acceptance Criteria

1. **Orchestrator Construction**
   - Given valid config and repo path
   - When `SkillsOrchestrator::new(config, path)` is called
   - Then orchestrator is created with all components initialized

2. **Execute Skeleton**
   - Given an orchestrator instance
   - When `execute(prompt)` is called
   - Then it runs through phase stubs and returns WorkflowResult

3. **Phase Logging**
   - Given execute() is called
   - When each phase stub runs
   - Then phase transitions are logged

4. **WorkflowResult**
   - Given workflow completes
   - When result is returned
   - Then it contains workflow_id, status, and summary

5. **Module Export**
   - Given orchestration module
   - When importing SkillsOrchestrator
   - Then it's accessible via `zen::orchestration::SkillsOrchestrator`

## Metadata
- **Complexity**: Medium
- **Labels**: Orchestration, Core, Skills, Architecture
- **Required Skills**: Rust, Arc/RwLock, async, composition
