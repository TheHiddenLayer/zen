# Zen v2 Implementation PRD

**Project:** Zen v2 - Parallel AI Agent Orchestrator
**Status:** Ready for Implementation
**Date:** 2026-01-30

---

## Overview

This PRD guides the implementation of Zen v2, transforming it from a single-session manager into a parallel multi-agent orchestrator. The implementation is broken into 20 steps with 48 total tasks.

**For each task: Run `/code-assist` with the task file as input.**

---

## Implementation Instructions

For each task listed below:

1. Read the `.code-task.md` file
2. Run `/code-assist` with the task
3. Verify the acceptance criteria pass
4. Commit the changes
5. Move to the next task

**Important:** Tasks within a step should be completed in order. Steps can only be started after all tasks in previous steps are complete.

---

## Step 1: Core Workflow Models and Types

| # | Task | File |
|---|------|------|
| 1.1 | Create Workflow Core Types | `.sop/planning/implementation/step01/task-01-workflow-core-types.code-task.md` |
| 1.2 | Create Workflow and WorkflowConfig Structs | `.sop/planning/implementation/step01/task-02-workflow-config-struct.code-task.md` |
| 1.3 | Implement WorkflowState with Phase Transitions | `.sop/planning/implementation/step01/task-03-workflow-state.code-task.md` |

**Demo:** Run `cargo test workflow` to verify all types work correctly.

---

## Step 2: Git State Manager Migration

| # | Task | File |
|---|------|------|
| 2.1 | Create GitStateManager Structure | `.sop/planning/implementation/step02/task-01-git-state-manager.code-task.md` |
| 2.2 | Implement Workflow Persistence via Git Notes | `.sop/planning/implementation/step02/task-02-workflow-persistence.code-task.md` |
| 2.3 | Create State Migration Tool | `.sop/planning/implementation/step02/task-03-state-migration.code-task.md` |

**Demo:** Create a workflow, verify it persists in git notes, restart Zen, workflow still exists.

---

## Step 3: AI-as-Human Proxy Foundation

| # | Task | File |
|---|------|------|
| 3.1 | Create AIHumanProxy Core Structure | `.sop/planning/implementation/step03/task-01-ai-human-proxy-core.code-task.md` |
| 3.2 | Implement ConversationContext Tracking | `.sop/planning/implementation/step03/task-02-conversation-context.code-task.md` |

**Demo:** Unit tests show AI-as-Human answering questions with context tracking.

---

## Step 4: Agent Pool Enhancements

| # | Task | File |
|---|------|------|
| 4.1 | Add AgentId and AgentStatus Types | `.sop/planning/implementation/step04/task-01-agent-id-status.code-task.md` |
| 4.2 | Create AgentPool for Multi-Agent Management | `.sop/planning/implementation/step04/task-02-agent-pool.code-task.md` |
| 4.3 | Implement AgentHandle for Agent Communication | `.sop/planning/implementation/step04/task-03-agent-handle.code-task.md` |

**Demo:** Spawn 3 agents, verify each has isolated worktree and tmux session.

---

## Step 5: Claude Code Headless Integration

| # | Task | File |
|---|------|------|
| 5.1 | Create ClaudeHeadless Executor | `.sop/planning/implementation/step05/task-01-claude-headless-executor.code-task.md` |
| 5.2 | Implement Claude Session Continuation | `.sop/planning/implementation/step05/task-02-session-continuation.code-task.md` |

**Demo:** Execute Claude headless, parse JSON response, continue session.

---

## Step 6: Skills Orchestrator Skeleton

| # | Task | File |
|---|------|------|
| 6.1 | Create SkillsOrchestrator Structure | `.sop/planning/implementation/step06/task-01-skills-orchestrator-struct.code-task.md` |
| 6.2 | Implement PhaseController | `.sop/planning/implementation/step06/task-02-phase-controller.code-task.md` |
| 6.3 | Implement Agent Output Monitor Loop | `.sop/planning/implementation/step06/task-03-agent-monitor-loop.code-task.md` |

**Demo:** Start workflow skeleton, verify phase transitions are logged.

---

## Step 7: Phase 1 - PDD Skill Integration

| # | Task | File |
|---|------|------|
| 7.1 | Implement PDD Phase Runner | `.sop/planning/implementation/step07/task-01-pdd-phase-runner.code-task.md` |
| 7.2 | Implement Question Detection in Agent Output | `.sop/planning/implementation/step07/task-02-question-detection.code-task.md` |

**Demo:** Run workflow, watch /pdd execute with AI answering questions.

---

## Step 8: Task and DAG Data Models

| # | Task | File |
|---|------|------|
| 8.1 | Create Task Data Model | `.sop/planning/implementation/step08/task-01-task-model.code-task.md` |
| 8.2 | Create TaskDAG Structure with petgraph | `.sop/planning/implementation/step08/task-02-dag-structure.code-task.md` |
| 8.3 | Implement DAG Scheduling Operations | `.sop/planning/implementation/step08/task-03-dag-operations.code-task.md` |

**Demo:** Create DAG with dependencies, verify ready_tasks returns correct tasks.

---

## Step 9: Phase 2 - Code Task Generator Integration

| # | Task | File |
|---|------|------|
| 9.1 | Create CodeTask Parser | `.sop/planning/implementation/step09/task-01-code-task-parser.code-task.md` |
| 9.2 | Implement Task Generation Phase | `.sop/planning/implementation/step09/task-02-task-generation-phase.code-task.md` |

**Demo:** After PDD, see /code-task-generator create .code-task.md files.

---

## Step 10: DAG Scheduler with Parallel Execution

| # | Task | File |
|---|------|------|
| 10.1 | Create Scheduler Core | `.sop/planning/implementation/step10/task-01-scheduler-core.code-task.md` |
| 10.2 | Implement Task Spawning with Worktree Isolation | `.sop/planning/implementation/step10/task-02-task-spawning.code-task.md` |

**Demo:** Schedule 5 tasks with dependencies, verify parallel execution.

---

## Step 11: Phase 3 - Parallel Code Assist Execution

| # | Task | File |
|---|------|------|
| 11.1 | Implement Implementation Phase Runner | `.sop/planning/implementation/step11/task-01-implementation-phase.code-task.md` |
| 11.2 | Implement Progress Tracking | `.sop/planning/implementation/step11/task-02-progress-tracking.code-task.md` |

**Demo:** Start workflow with 4 tasks, see 4 parallel agents working.

---

## Step 12: Phase 4 - Merge and Conflict Resolution

| # | Task | File |
|---|------|------|
| 12.1 | Create ConflictResolver Structure | `.sop/planning/implementation/step12/task-01-conflict-resolver-struct.code-task.md` |
| 12.2 | Implement Merge Logic | `.sop/planning/implementation/step12/task-02-merge-logic.code-task.md` |
| 12.3 | Implement AI-Assisted Conflict Resolution | `.sop/planning/implementation/step12/task-03-resolution-agent.code-task.md` |

**Demo:** Simulate merge conflict, watch resolver agent fix it.

---

## Step 13: Phase 5 - Codebase Summary Integration

| # | Task | File |
|---|------|------|
| 13.1 | Implement Documentation Phase | `.sop/planning/implementation/step13/task-01-documentation-phase.code-task.md` |
| 13.2 | Implement Merge Phase Runner | `.sop/planning/implementation/step13/task-02-merge-phase-runner.code-task.md` |

**Demo:** Complete full workflow, see documentation updated.

---

## Step 14: Health Monitor and Stuck Detection

| # | Task | File |
|---|------|------|
| 14.1 | Create Health Monitor | `.sop/planning/implementation/step14/task-01-health-monitor.code-task.md` |
| 14.2 | Implement AI-Driven Recovery | `.sop/planning/implementation/step14/task-02-ai-recovery.code-task.md` |

**Demo:** Simulate stuck agent, watch health monitor detect and recover.

---

## Step 15: Reactive Planner (Plan Change Detection)

| # | Task | File |
|---|------|------|
| 15.1 | Implement File Watcher for Plan Changes | `.sop/planning/implementation/step15/task-01-file-watcher.code-task.md` |
| 15.2 | Implement Replanning Logic | `.sop/planning/implementation/step15/task-02-replanning-logic.code-task.md` |

**Demo:** Edit plan.md during execution, see Zen adapt.

---

## Step 16: TUI Dashboard Enhancements

| # | Task | File |
|---|------|------|
| 16.1 | Create Workflow UI Components | `.sop/planning/implementation/step16/task-01-workflow-ui.code-task.md` |
| 16.2 | Create Multi-Agent Grid Display | `.sop/planning/implementation/step16/task-02-agent-grid.code-task.md` |
| 16.3 | Create DAG Visualization | `.sop/planning/implementation/step16/task-03-dag-visualization.code-task.md` |

**Demo:** Start workflow, see TUI with phases, agent grid, and DAG.

---

## Step 17: CLI Commands (run, review, accept, reject)

| # | Task | File |
|---|------|------|
| 17.1 | Define CLI Command Structure | `.sop/planning/implementation/step17/task-01-cli-commands.code-task.md` |
| 17.2 | Implement Run and Review Commands | `.sop/planning/implementation/step17/task-02-run-review-commands.code-task.md` |
| 17.3 | Implement Accept and Reject Commands | `.sop/planning/implementation/step17/task-03-accept-reject-commands.code-task.md` |

**Demo:** Run `zen run`, `zen review`, `zen accept` workflow.

---

## Step 18: Worktree Auto-Cleanup

| # | Task | File |
|---|------|------|
| 18.1 | Create CleanupManager | `.sop/planning/implementation/step18/task-01-cleanup-manager.code-task.md` |
| 18.2 | Implement Orphan Detection and Background Cleanup | `.sop/planning/implementation/step18/task-02-orphan-detection.code-task.md` |

**Demo:** Complete workflow, see worktrees cleaned up automatically.

---

## Step 19: Integration Testing and Polish

| # | Task | File |
|---|------|------|
| 19.1 | Create Integration Test Suite | `.sop/planning/implementation/step19/task-01-integration-tests.code-task.md` |
| 19.2 | Create Performance Tests | `.sop/planning/implementation/step19/task-02-performance-tests.code-task.md` |

**Demo:** Run `cargo test --test integration`, all tests pass.

---

## Step 20: Documentation and User Guide

| # | Task | File |
|---|------|------|
| 20.1 | Create User Documentation | `.sop/planning/implementation/step20/task-01-user-documentation.code-task.md` |
| 20.2 | Create Architecture Documentation | `.sop/planning/implementation/step20/task-02-architecture-documentation.code-task.md` |

**Demo:** New user follows README, runs first workflow successfully.

---

## Summary

**Total Steps:** 20
**Total Tasks:** 48

### Execution Order

```
Step 1  → Step 2  → Step 3  → Step 4  → Step 5
   ↓
Step 6  → Step 7  → Step 8  → Step 9  → Step 10
   ↓
Step 11 → Step 12 → Step 13 → Step 14 → Step 15
   ↓
Step 16 → Step 17 → Step 18 → Step 19 → Step 20
```

### Key Milestones

| Milestone | After Step | Capability |
|-----------|------------|------------|
| Foundation Complete | 5 | Core types, state, AI proxy, agents, Claude integration |
| Skills Orchestration | 7 | Can run /pdd with AI-as-Human |
| Parallel Execution | 11 | Full parallel task execution |
| Complete Workflow | 13 | All 5 phases working |
| Production Ready | 18 | Health monitoring, cleanup, CLI |
| Release Ready | 20 | Full testing and documentation |

---

## Reference Documents

- **Detailed Design:** `.sop/planning/design/detailed-design.md`
- **Research - Existing Code:** `.sop/planning/research/existing-code.md`
- **Research - Rust Ecosystem:** `.sop/planning/research/rust-ecosystem.md`
- **Research - Claude Integration:** `.sop/planning/research/claude-code-integration.md`
- **Research - Skills:** `.sop/planning/research/skills-integration.md`
- **Implementation Plan:** `.sop/planning/implementation/plan.md`

---

*End of PRD*
