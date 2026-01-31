# User Documentation Context

## Task Overview
Create comprehensive user documentation for Zen v2, a parallel AI agent orchestrator.

## Requirements Summary
1. **docs/user-guide.md** - Full user guide with:
   - Installation instructions
   - Quick start guide
   - Workflow phases explained
   - CLI command reference
   - Configuration options
   - Troubleshooting guide

2. **README.md** - Project overview with:
   - Project description
   - Key features
   - Installation
   - Basic usage example
   - Links to detailed docs

## Existing Documentation
- `docs/architecture.md` - Internal architecture docs
- `docs/game-loop-architecture.md` - Two-thread design
- `docs/tea-pattern.md` - TEA state management
- Other internal technical docs

## Key Information from Code Analysis

### CLI Commands (from src/main.rs)
| Command | Description |
|---------|-------------|
| `zen` | Launch TUI dashboard |
| `zen run "<prompt>"` | Start a workflow with natural language |
| `zen run --headless "<prompt>"` | Run workflow without TUI (JSON output) |
| `zen review [workflow_id]` | Review completed workflow |
| `zen accept [workflow_id] [-y]` | Merge completed work to main |
| `zen reject <workflow_id>` | Discard workflow changes |
| `zen status` | Show status of workflows |
| `zen attach <agent_id>` | Attach to agent's tmux session |
| `zen reset [--force]` | Delete all sessions |
| `zen cleanup [--delete] [-y]` | Clean up orphaned resources |

### Global Flags
- `-t, --trust` - Auto-approve agent prompts
- `-d, --debug` - Enable debug logging (writes to ~/.zen/zen.log)

### Workflow Phases (from detailed-design.md)
1. **Planning** - /pdd skill creates design and plan
2. **TaskGeneration** - /code-task-generator creates .code-task.md files
3. **Implementation** - Parallel /code-assist agents execute tasks
4. **Merging** - Merge worktrees, resolve conflicts
5. **Documentation** - /codebase-summary updates docs (optional)

### Configuration
- `WorkflowConfig.update_docs` - Enable/disable documentation phase (default: true)
- `WorkflowConfig.max_parallel_agents` - Max concurrent agents (default: 4)
- `WorkflowConfig.staging_branch_prefix` - Branch prefix (default: "zen/staging/")

### Prerequisites
- Rust (for building from source)
- Git
- Tmux
- Claude Code CLI

## Implementation Approach
1. Create user-guide.md with all sections
2. Create/update README.md with overview
3. Verify all CLI commands documented correctly
4. Test examples are accurate
