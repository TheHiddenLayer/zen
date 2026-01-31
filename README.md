# Zen

A parallel AI agent orchestrator that transforms natural language prompts into implemented code.

Zen spawns multiple Claude Code agents working concurrently across isolated git worktrees, with AI-driven dependency inference, automatic conflict resolution, and a reactive planning system.

## Key Features

- **Parallel Execution** - Multiple AI agents work simultaneously on independent tasks
- **DAG-Based Scheduling** - Respects task dependencies for correct execution order
- **Git Worktree Isolation** - Each agent operates in an isolated working directory
- **AI-Assisted Conflict Resolution** - Automatically resolves merge conflicts
- **Skills-Driven Workflow** - Orchestrates /pdd, /code-task-generator, and /code-assist skills
- **TUI Dashboard** - Real-time monitoring of agents and progress
- **Headless Mode** - JSON output for CI/CD integration

## Installation

### Prerequisites

- Rust 1.70+
- Git
- Tmux
- Claude Code CLI

### Build from Source

```bash
git clone https://github.com/your-org/zen.git
cd zen
cargo install --path .
```

## Quick Start

```bash
# Navigate to your project
cd my-project

# Start a workflow
zen run "add user authentication with login and logout"

# Review completed work
zen review

# Accept and merge to main
zen accept
```

## Usage

### Interactive TUI

```bash
zen                    # Launch dashboard
zen -t                 # Launch with auto-trust
```

### Workflow Commands

```bash
# Start a workflow
zen run "implement feature X"
zen run --headless "add tests"    # JSON output

# Review and manage
zen review [workflow_id]          # View changes
zen accept [workflow_id] [-y]     # Merge to main
zen reject <workflow_id>          # Discard changes

# Utilities
zen status                        # Show all workflows
zen attach <agent_id>             # Connect to agent session
zen cleanup --delete              # Remove orphaned resources
zen reset                         # Clear all sessions
```

### Global Options

```
-t, --trust    Auto-approve agent prompts
-d, --debug    Enable debug logging
-h, --help     Print help
-V, --version  Print version
```

## How It Works

Zen executes workflows through five phases:

1. **Planning** - Analyzes prompt and creates design documents
2. **Task Generation** - Breaks work into parallelizable tasks
3. **Implementation** - Runs concurrent AI agents in isolated worktrees
4. **Merging** - Combines completed work with conflict resolution
5. **Documentation** - Optionally updates project docs

```
User: "zen run 'add authentication'"
         │
         ▼
┌─────────────────────────────────────────┐
│  Phase 1: /pdd                          │
│  Creates design and implementation plan │
└─────────────────────────────────────────┘
         │
         ▼
┌─────────────────────────────────────────┐
│  Phase 2: /code-task-generator          │
│  Generates .code-task.md files          │
└─────────────────────────────────────────┘
         │
         ▼
┌─────────────────────────────────────────┐
│  Phase 3: /code-assist (parallel)       │
│  ┌─────┐ ┌─────┐ ┌─────┐ ┌─────┐       │
│  │Agent│ │Agent│ │Agent│ │Agent│       │
│  │  1  │ │  2  │ │  3  │ │  4  │       │
│  └─────┘ └─────┘ └─────┘ └─────┘       │
└─────────────────────────────────────────┘
         │
         ▼
┌─────────────────────────────────────────┐
│  Phase 4: Merge & Resolve               │
│  Combines work, resolves conflicts      │
└─────────────────────────────────────────┘
         │
         ▼
┌─────────────────────────────────────────┐
│  Phase 5: /codebase-summary (optional)  │
│  Updates documentation                  │
└─────────────────────────────────────────┘
         │
         ▼
    zen review → zen accept
```

## Documentation

- [User Guide](docs/user-guide.md) - Complete usage documentation
- [Architecture](docs/architecture.md) - Internal design details
- [Game Loop](docs/game-loop-architecture.md) - Two-thread architecture
- [TEA Pattern](docs/tea-pattern.md) - State management

## Configuration

Configuration file: `~/.zen/config.toml`

```toml
trust = false     # Auto-approve prompts
agent = "claude"  # Default AI model
```

## License

MIT
