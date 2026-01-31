# Task: Create Architecture Documentation - Context

## Project Overview

Zen v2 is a parallel AI agent orchestrator written in Rust that transforms natural language prompts into implemented code through a 5-phase workflow orchestrating multiple Claude Code agents.

## Requirements Summary

1. **Update docs/architecture.md** - Add system overview with mermaid diagrams, component descriptions, data flow, thread model, and state management (TEA pattern)
2. **Create docs/skills-integration.md** - Document how Zen orchestrates Skills, the AI-as-Human pattern, and how to add new skills
3. **Create/update CONTRIBUTING.md** - Development setup, code style guide, testing requirements, PR process

## Existing Documentation

### docs/architecture.md (existing)
- Contains basic architecture overview for the original Zen TUI
- Documents two-thread decoupled game loop (render + logic threads)
- Documents TEA pattern (Model, Message, Command, Update)
- Documents module overview and data flow
- **Needs updating** to include v2 orchestration components

### README.md (existing)
- Contains quick start and usage documentation
- Links to detailed docs

### Detailed Design (.sop/planning/design/detailed-design.md)
- Comprehensive design document for Zen v2
- Contains component interfaces, data models, algorithms
- Key innovations: Skills-Driven Workflow, AI-as-Human Pattern
- 5 workflow phases: Planning, TaskGeneration, Implementation, Merging, Documentation

## Key Components to Document

### Core Orchestration (new in v2)
- **SkillsOrchestrator** - Coordinates 5-phase workflow
- **AIHumanProxy** - Autonomously answers skill clarification questions
- **PhaseController** - Manages workflow phase transitions
- **AgentPool** - Multi-agent lifecycle management
- **Scheduler** - DAG-based parallel task execution
- **ConflictResolver** - AI-assisted merge conflict resolution
- **HealthMonitor** - Stuck agent detection and recovery
- **ReactivePlanner** - Plan change detection and DAG updates

### Core Models (new in v2)
- **Workflow** - Complete orchestration state
- **Task** - Single unit of work
- **TaskDAG** - Dependency graph (petgraph)
- **CodeTask** - Parsed .code-task.md file

### Existing Components (preserved)
- Two-thread game loop (main + logic)
- TEA pattern (Model, Message, Command, Update)
- GitRefs, GitNotes, GitOps
- Session, Tmux, Agent abstractions

## Implementation Approach

1. **architecture.md** - Extend existing doc with v2 orchestration layer
2. **skills-integration.md** - New file documenting skills workflow
3. **CONTRIBUTING.md** - New file with development guide

## Dependencies

- Existing docs/ folder with architecture.md, tea-pattern.md, actors.md, etc.
- Detailed design document with complete interface definitions
- Implemented codebase with all 48 tasks complete
