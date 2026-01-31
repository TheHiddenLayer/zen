# Zen v2: Parallel AI Agent Orchestrator - Project Summary

**Date:** 2026-01-30
**Status:** Ready for Implementation

---

## Artifacts Created

### 1. Requirements (`.sop/planning/`)
- **rough-idea.md** - Initial vision from Ralph audit
- **idea-honing.md** - 14 Q&A pairs capturing all requirements

### 2. Research (`.sop/planning/research/`)
- **existing-code.md** - Zen codebase analysis (~1000 lines)
- **rust-ecosystem.md** - Library recommendations (petgraph, ratatui, git2, tokio)
- **claude-code-integration.md** - Headless mode, JSON output, session management
- **skills-integration.md** - SKILL.md format, /pdd, /code-task-generator, /code-assist

### 3. Design (`.sop/planning/design/`)
- **detailed-design.md** - Comprehensive design document (~1500 lines)
  - Architecture overview with mermaid diagrams
  - Component interfaces with Rust code
  - Data models and git-native schema
  - Error handling and recovery strategies
  - Testing strategy
  - **Key Innovation:** Skills Orchestrator + AI-as-Human pattern

### 4. Implementation (`.sop/planning/implementation/`)
- **plan.md** - 20-step implementation plan with checklist
  - Each step is demoable
  - TDD approach with test requirements
  - Incremental build-up from foundation to complete system

---

## Design Overview

### Core Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                     ZEN ORCHESTRATION FLOW                      │
├─────────────────────────────────────────────────────────────────┤
│  User: "zen 'build user authentication'"                        │
│                           │                                     │
│                           ▼                                     │
│  PHASE 1: /pdd → detailed-design.md + plan.md                  │
│                           │                                     │
│                           ▼                                     │
│  PHASE 2: /code-task-generator → .code-task.md files           │
│                           │                                     │
│                           ▼                                     │
│  PHASE 3: /code-assist (PARALLEL in worktrees)                 │
│                           │                                     │
│                           ▼                                     │
│  PHASE 4: Merge & Resolve (AI conflict resolution)             │
│                           │                                     │
│                           ▼                                     │
│  PHASE 5: /codebase-summary (update docs)                      │
│                           │                                     │
│                           ▼                                     │
│  User: zen review → zen accept                                 │
└─────────────────────────────────────────────────────────────────┘
```

### Key Innovations

1. **Skills-Driven Workflow**: Zen orchestrates existing Skills (/pdd, /code-task-generator, /code-assist, /codebase-summary) rather than implementing custom planning/coding logic.

2. **AI-as-Human Pattern**: An AI proxy autonomously answers skill clarification questions based on the original user intent, enabling fully autonomous execution.

3. **Preserve Architecture**: The existing decoupled game loop (60 FPS render + Tokio logic thread) is extended, not replaced.

4. **Git-Native State**: All state stored in git refs/notes - portable, versioned, no external dependencies.

5. **Parallel Execution**: DAG-based scheduler runs independent tasks in parallel across isolated worktrees.

---

## Implementation Plan Summary

### Phase 1: Foundation (Steps 1-5)
- Core workflow models and types
- Git state manager migration
- AI-as-Human proxy
- Enhanced agent pool
- Claude Code headless integration

### Phase 2: Skills Orchestration (Steps 6-7)
- Skills orchestrator skeleton
- PDD skill integration (Phase 1 of workflow)

### Phase 3: Task Management (Steps 8-11)
- Task and DAG data models
- Code task generator integration (Phase 2)
- DAG scheduler with parallel execution
- Parallel code-assist execution (Phase 3)

### Phase 4: Merge & Quality (Steps 12-15)
- Merge and conflict resolution (Phase 4)
- Codebase summary integration (Phase 5)
- Health monitor and stuck detection
- Reactive planner

### Phase 5: User Experience (Steps 16-20)
- TUI dashboard enhancements
- CLI commands
- Worktree auto-cleanup
- Integration testing
- Documentation

---

## User Workflow

```bash
# Start a workflow
zen "build user authentication"

# User goes to do other work...

# Check status (optional)
zen status

# Review completed work
zen review

# Accept and merge to main
zen accept

# Or reject if issues
zen reject wf-001
```

---

## Key Decisions Made

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Agent CLI | Claude Code only | Pluggable adapter for future, CC is must-have |
| State Storage | Git-native (refs/notes) | Fits scrappy philosophy, portable |
| Task Dependencies | AI-inferred | Sledgehammer - AI handles complexity |
| Conflict Resolution | Dedicated resolver agent | Separation of concerns |
| Skills Integration | Hardcoded workflow | Predictable, debuggable |
| UI | TUI (htop-style) | Survives SSH, developer-focused |

---

## Out of Scope (v1)

- Multi-agent providers (Amp, Aider)
- Cost tracking/budgets
- Web UI
- Remote/distributed execution
- CI/CD integration

---

## Next Steps

1. **Review** the detailed design document for any questions
2. **Start implementation** following plan.md step-by-step
3. **Use /code-task-generator** to convert plan.md into .code-task.md files
4. **Use /code-assist** for each implementation step

The implementation plan is designed to be executed with Zen itself once the foundation steps are complete (bootstrapping).

---

## File References

| File | Description |
|------|-------------|
| `.sop/planning/rough-idea.md` | Initial vision |
| `.sop/planning/idea-honing.md` | Requirements Q&A |
| `.sop/planning/research/*.md` | Technology research |
| `.sop/planning/design/detailed-design.md` | Full design |
| `.sop/planning/implementation/plan.md` | Implementation checklist |
| `.sop/planning/summary.md` | This file |

---

*End of Summary*
