# Zen User Guide

Zen is a parallel AI agent orchestrator that transforms natural language prompts into implemented code through multiple concurrent Claude Code agents.

## Table of Contents

- [Installation](#installation)
- [Quick Start](#quick-start)
- [Workflow Phases](#workflow-phases)
- [CLI Reference](#cli-reference)
- [Configuration](#configuration)
- [Troubleshooting](#troubleshooting)

---

## Installation

### Prerequisites

Before installing Zen, ensure you have:

- **Rust** (1.70+) - [Install Rust](https://rustup.rs/)
- **Git** - Version control system
- **Tmux** - Terminal multiplexer
- **Claude Code CLI** - AI coding assistant

Verify prerequisites:
```bash
rustc --version    # Should show 1.70.0 or higher
git --version      # Any recent version
tmux -V            # Any recent version
claude --version   # Claude Code CLI
```

### Building from Source

```bash
# Clone the repository
git clone https://github.com/your-org/zen.git
cd zen

# Build in release mode
cargo build --release

# Install to ~/.cargo/bin
cargo install --path .

# Verify installation
zen --version
```

### Post-Installation Setup

1. **Initialize your repository** (if not already a git repo):
   ```bash
   cd your-project
   git init
   ```

2. **Verify Claude Code is configured**:
   ```bash
   claude --help
   ```

---

## Quick Start

### Your First Workflow

1. **Navigate to your project**:
   ```bash
   cd my-project
   ```

2. **Start a workflow with a natural language prompt**:
   ```bash
   zen run "add user authentication with login and logout"
   ```

3. **Watch the workflow execute** - Zen will:
   - Create a design document
   - Break the task into parallel subtasks
   - Spawn AI agents in isolated git worktrees
   - Merge completed work to a staging branch

4. **Review the completed work**:
   ```bash
   zen review
   ```

5. **Accept and merge to main**:
   ```bash
   zen accept
   ```

### Interactive TUI Mode

Launch the TUI dashboard for managing coding sessions:

```bash
zen
```

Use arrow keys to navigate and press `?` for help.

---

## Workflow Phases

Zen executes workflows through five distinct phases:

### Phase 1: Planning

Zen runs the `/pdd` (Prompt-Driven Development) skill to analyze your request and create:
- A detailed design document (`.sop/planning/design/detailed-design.md`)
- An implementation plan (`.sop/planning/implementation/plan.md`)

The AI-as-Human proxy automatically answers clarification questions based on your original prompt.

### Phase 2: Task Generation

The `/code-task-generator` skill converts the plan into discrete tasks:
- Creates `.code-task.md` files for each implementation step
- Infers dependencies between tasks
- Estimates complexity levels

### Phase 3: Implementation

Zen executes tasks in parallel using a DAG (Directed Acyclic Graph) scheduler:
- Each task runs in an isolated git worktree
- Multiple Claude Code agents work concurrently
- Tasks respect dependency ordering
- Progress is tracked and displayed

### Phase 4: Merging

Completed work is merged to a staging branch:
- Branch name: `zen/staging/{workflow-id}`
- Conflicts are detected and resolved automatically
- AI-assisted conflict resolution when needed

### Phase 5: Documentation (Optional)

If enabled, the `/codebase-summary` skill updates:
- README.md
- AGENTS.md (documentation for AI assistants)
- Other relevant documentation

---

## CLI Reference

### Global Options

```
-t, --trust     Auto-approve agent prompts
-d, --debug     Enable debug logging (writes to ~/.zen/zen.log)
-h, --help      Print help
-V, --version   Print version
```

### Commands

#### `zen` (no subcommand)

Launch the interactive TUI dashboard.

```bash
zen              # Launch TUI
zen -t           # Launch with auto-trust enabled
zen -d           # Launch with debug logging
```

#### `zen run <prompt>`

Start a new workflow with a natural language prompt.

```bash
zen run "implement user registration"
zen run --headless "add payment processing"
zen run "refactor auth module" --headless
```

Options:
- `--headless` - Run without TUI, output JSON result

Headless output format:
```json
{
  "workflow_id": "abc12345",
  "status": "completed",
  "summary": "Successfully implemented user registration"
}
```

#### `zen review [workflow_id]`

Review completed workflow results.

```bash
zen review              # Review most recent workflow
zen review abc12345     # Review specific workflow by short ID
zen review 550e8400-e29b-41d4-a716-446655440000  # Full UUID
```

Displays:
- Workflow summary (ID, name, status, phase)
- Timestamps (created, started, completed)
- Task count
- Changed files and diff statistics
- Suggestions for next steps

#### `zen accept [workflow_id] [-y]`

Accept and merge completed work to the main branch.

```bash
zen accept              # Accept most recent completed workflow
zen accept abc12345     # Accept specific workflow
zen accept -y           # Skip confirmation prompt
zen accept abc12345 --yes
```

This command:
1. Verifies workflow is completed
2. Shows confirmation prompt (unless `-y` flag)
3. Merges staging branch to main
4. Cleans up worktrees
5. Deletes staging branch
6. Marks workflow as "Accepted"

#### `zen reject <workflow_id>`

Reject and rollback workflow changes.

```bash
zen reject abc12345
```

This command:
1. Deletes the staging branch
2. Cleans up worktrees
3. Preserves task branches for debugging
4. Marks workflow as "Rejected"

#### `zen status`

Show status of all workflows and agents.

```bash
zen status
```

#### `zen attach <agent_id>`

Attach to an agent's tmux session for direct interaction.

```bash
zen attach agent-abc123
```

This opens the tmux session where the agent is running, allowing you to:
- View real-time agent output
- Interact directly with the agent
- Debug issues

Press `Ctrl-b d` to detach from the tmux session.

#### `zen reset [--force]`

Delete all sessions and reset Zen state.

```bash
zen reset           # Skip sessions with uncommitted work
zen reset --force   # Delete all sessions including dirty ones
```

#### `zen cleanup [--delete] [-y]`

Detect and optionally clean up orphaned resources.

```bash
zen cleanup                 # Report orphans (dry run)
zen cleanup --delete        # Delete orphans with confirmation
zen cleanup --delete -y     # Delete orphans without confirmation
```

Detects:
- Orphaned worktrees in `~/.zen/worktrees/`
- Orphaned tmux sessions (`zen_*`)
- Orphaned branches (`zen/*`)

---

## Configuration

### Configuration File

Zen looks for configuration in `~/.zen/config.toml` (or `zen.toml` in project root):

```toml
# Auto-approve agent prompts
trust = false

# Default AI model
agent = "claude"
```

### Workflow Configuration

When programmatically creating workflows, these options are available:

| Option | Default | Description |
|--------|---------|-------------|
| `update_docs` | `true` | Run documentation phase after merge |
| `max_parallel_agents` | `4` | Maximum concurrent agents |
| `staging_branch_prefix` | `"zen/staging/"` | Prefix for staging branches |

### Environment Variables

```bash
ZEN_DEBUG=1     # Enable debug logging (alternative to --debug)
```

### Data Directories

| Path | Purpose |
|------|---------|
| `~/.zen/` | Configuration and state directory |
| `~/.zen/zen.log` | Debug log file |
| `~/.zen/worktrees/` | Git worktrees for agents |
| `~/.zen/state/` | Session state (JSON) |

---

## Troubleshooting

### Common Issues

#### "Workflow not found"

```
Error: Workflow not found: abc12345
```

**Cause:** The specified workflow ID doesn't exist or was deleted.

**Solution:**
1. List available workflows: `zen status`
2. Use a valid workflow ID
3. Note: workflow IDs are case-sensitive

#### "Cannot accept workflow with status 'failed'"

```
Error: Cannot accept workflow with status 'failed'. Only completed workflows can be accepted.
```

**Cause:** The workflow encountered an error during execution.

**Solution:**
1. Review the workflow: `zen review <workflow_id>`
2. Check logs for error details
3. Try running the workflow again or reject it: `zen reject <workflow_id>`

#### "Staging branch does not exist"

```
Error: Staging branch 'zen/staging/abc12345' does not exist
```

**Cause:** The workflow didn't complete the merge phase.

**Solution:**
1. Check workflow status: `zen review <workflow_id>`
2. The workflow may need to be re-run
3. Reject and start fresh: `zen reject <workflow_id>`

#### Agent appears stuck

**Symptoms:** Progress stops, agent shows no new output.

**Solution:**
1. Attach to the agent: `zen attach <agent_id>`
2. Check for prompts or errors
3. If truly stuck, the health monitor will attempt recovery
4. Manually restart if needed

#### "Tmux session not found"

```
Error: Tmux error: session not found
```

**Cause:** The tmux session was killed externally.

**Solution:**
1. Clean up orphans: `zen cleanup --delete`
2. Reset if needed: `zen reset`

#### Merge conflicts not resolving

**Cause:** AI conflict resolution failed or timed out.

**Solution:**
1. Check the staging branch for conflict markers
2. Resolve manually: `git checkout zen/staging/<id> && git status`
3. After manual resolution, the workflow can be accepted

### Debug Logging

Enable verbose logging for troubleshooting:

```bash
# Via flag
zen -d run "my task"

# Via environment variable
ZEN_DEBUG=1 zen run "my task"
```

Logs are written to `~/.zen/zen.log`.

### Getting Help

- View all commands: `zen --help`
- Command-specific help: `zen run --help`
- In TUI mode: Press `?` for keybindings

### Reporting Issues

When reporting issues, include:
1. Zen version: `zen --version`
2. Relevant log output from `~/.zen/zen.log`
3. Steps to reproduce
4. Expected vs actual behavior
