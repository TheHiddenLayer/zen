# Task: Define CLI Command Structure

## Description
Define the CLI command structure using clap, adding all workflow commands (run, review, accept, reject, status, attach).

## Background
Users interact with Zen via CLI commands. The new workflow features need commands for starting workflows, reviewing results, and accepting/rejecting work.

## Reference Documentation
**Required:**
- Design: .sop/planning/design/detailed-design.md (Section 4.1 CLI Interface)

**Note:** You MUST read the detailed design document before beginning implementation.

## Technical Requirements
1. Update `src/main.rs` with clap:
   ```rust
   #[derive(Parser)]
   #[command(name = "zen")]
   struct Cli {
       #[command(subcommand)]
       command: Option<Command>,
       // Existing flags
       #[arg(short, long)]
       trust: bool,
       #[arg(short, long)]
       debug: bool,
   }

   #[derive(Subcommand)]
   enum Command {
       Run { prompt: String, #[arg(long)] headless: bool },
       Review { workflow_id: Option<String> },
       Accept { workflow_id: Option<String> },
       Reject { workflow_id: String },
       Status,
       Attach { agent_id: String },
       Reset { #[arg(long)] force: bool },
   }
   ```
2. Add clap dependency if not present
3. Keep backward compatibility (no subcommand = TUI)

## Dependencies
- clap crate with derive feature
- Existing main.rs

## Implementation Approach
1. Add clap = { version = "4", features = ["derive"] }
2. Define Cli and Command structures
3. Parse args in main()
4. Route to appropriate handler (placeholder)
5. Keep default behavior (TUI) when no command
6. Add help text for all commands
7. Add tests for CLI parsing

## Acceptance Criteria

1. **Run Command**
   - Given `zen run "build auth"`
   - When parsed
   - Then Run command with prompt is returned

2. **No Command**
   - Given `zen` with no args
   - When parsed
   - Then TUI is launched (existing behavior)

3. **Headless Flag**
   - Given `zen run --headless "build auth"`
   - When parsed
   - Then headless=true in Run command

4. **Help Text**
   - Given `zen --help`
   - When executed
   - Then all commands are listed with descriptions

5. **Backward Compatibility**
   - Given existing `zen --trust` usage
   - When executed
   - Then trust flag works as before

## Metadata
- **Complexity**: Low
- **Labels**: CLI, Commands, clap, Interface
- **Required Skills**: Rust, clap, CLI design
