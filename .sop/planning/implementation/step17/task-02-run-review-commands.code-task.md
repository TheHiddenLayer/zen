# Task: Implement Run and Review Commands

## Description
Implement the `zen run` command that starts a workflow and `zen review` that shows completed workflow summary.

## Background
`zen run` is the primary entry point for autonomous execution. It takes a prompt and orchestrates the full workflow. `zen review` shows what was accomplished for user inspection.

## Reference Documentation
**Required:**
- Design: .sop/planning/design/detailed-design.md (Section 2.4 User Workflow)

**Note:** You MUST read the detailed design document before beginning implementation.

## Technical Requirements
1. Implement `zen run`:
   - Create workflow from prompt
   - Start SkillsOrchestrator.execute()
   - Show TUI with progress (unless --headless)
   - On completion, save workflow state
2. Implement `zen review`:
   - Load most recent or specified workflow
   - Show summary: tasks completed, files changed
   - Show diff of staging vs main
   - List any issues or warnings

## Dependencies
- CLI structure from task-01
- SkillsOrchestrator from Step 6
- GitStateManager from Step 2

## Implementation Approach
1. Create run_command() handler
2. Initialize SkillsOrchestrator
3. Execute workflow with TUI or headless mode
4. Create review_command() handler
5. Load workflow from GitStateManager
6. Format and display summary
7. Show git diff for staging branch
8. Add tests for both commands

## Acceptance Criteria

1. **Run Starts Workflow**
   - Given `zen run "build auth"`
   - When command executes
   - Then workflow is created and orchestrator starts

2. **TUI Display**
   - Given run without --headless
   - When workflow runs
   - Then TUI shows progress

3. **Headless Mode**
   - Given `zen run --headless "build auth"`
   - When workflow runs
   - Then no TUI, only JSON status output

4. **Review Summary**
   - Given completed workflow
   - When `zen review` is run
   - Then summary shows tasks, files, and status

5. **Review Diff**
   - Given staging branch with changes
   - When review shows diff
   - Then changes vs main are displayed

## Metadata
- **Complexity**: Medium
- **Labels**: CLI, Run, Review, Commands
- **Required Skills**: Rust, CLI handlers, formatting
