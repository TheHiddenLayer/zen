use std::io::{self, stdout, Stdout};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use clap::{Parser, Subcommand};
use crossbeam_channel::{Receiver, TryRecvError};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::{backend::CrosstermBackend, Terminal};

use zen::app::LogicThread;
use zen::cleanup::{CleanupConfig, CleanupManager};
use zen::config::Config;
use zen::git::GitOps;
use zen::orchestration::{SkillsOrchestrator, WorkflowResult};
use zen::render::RenderState;
use zen::state::GitStateManager;
use zen::workflow::{Workflow, WorkflowConfig, WorkflowId, WorkflowStatus};
use zen::{ui, zlog, Result};

use std::collections::HashSet;

const FRAME_DURATION: Duration = Duration::from_micros(16_666); // 60fps

/// Zen - AI coding session manager and parallel agent orchestrator
#[derive(Parser, Debug)]
#[command(name = "zen")]
#[command(version, about, long_about = None)]
#[command(after_help = "ENVIRONMENT:\n    ZEN_DEBUG=1     Enable debug logging (alternative to --debug)")]
pub struct Cli {
    /// Auto-approve agent prompts
    #[arg(short = 't', long)]
    pub trust: bool,

    /// Enable debug logging (writes to ~/.zen/zen.log)
    #[arg(short = 'd', long)]
    pub debug: bool,

    #[command(subcommand)]
    pub command: Option<Command>,
}

/// Workflow commands for Zen orchestration
#[derive(Subcommand, Debug, Clone, PartialEq)]
pub enum Command {
    /// Start a new workflow with a natural language prompt
    Run {
        /// The task description in natural language
        prompt: String,

        /// Run in headless mode (no TUI, JSON output)
        #[arg(long)]
        headless: bool,
    },

    /// Review completed workflow results
    Review {
        /// Workflow ID to review (uses latest if not specified)
        workflow_id: Option<String>,
    },

    /// Accept and merge completed work to main branch
    Accept {
        /// Workflow ID to accept (uses latest if not specified)
        workflow_id: Option<String>,

        /// Skip confirmation prompt
        #[arg(long, short = 'y')]
        yes: bool,
    },

    /// Reject and rollback workflow changes
    Reject {
        /// Workflow ID to reject
        workflow_id: String,
    },

    /// Show status of all workflows and agents
    Status,

    /// Attach to an agent's tmux session for direct interaction
    Attach {
        /// Agent ID to attach to
        agent_id: String,
    },

    /// Reset and delete all sessions
    Reset {
        /// Delete sessions even if they have uncommitted work
        #[arg(long)]
        force: bool,
    },

    /// Clean up orphaned resources (worktrees, tmux sessions, branches)
    Cleanup {
        /// Actually delete orphans (default: just report)
        #[arg(long)]
        delete: bool,

        /// Skip confirmation prompt when deleting
        #[arg(long, short = 'y')]
        yes: bool,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    // Initialize logging based on debug flag
    zen::log::init_with_debug(cli.debug);

    // Handle subcommands
    match cli.command {
        Some(Command::Reset { force }) => {
            return run_reset(force);
        }
        Some(Command::Run { prompt, headless }) => {
            return run_workflow(prompt, headless, cli.trust, cli.debug);
        }
        Some(Command::Review { workflow_id }) => {
            return run_review(workflow_id);
        }
        Some(Command::Accept { workflow_id, yes }) => {
            return run_accept(workflow_id, yes);
        }
        Some(Command::Reject { workflow_id }) => {
            return run_reject(workflow_id);
        }
        Some(Command::Status) => {
            return run_status();
        }
        Some(Command::Attach { agent_id }) => {
            return run_attach(agent_id);
        }
        Some(Command::Cleanup { delete, yes }) => {
            return run_cleanup(delete, yes);
        }
        None => {
            // No subcommand: launch TUI (existing behavior)
        }
    }

    if cli.debug {
        zlog!("Zen starting (debug mode enabled)");
    } else {
        zlog!("Zen starting");
    }

    let mut config = Config::load()?;
    if cli.trust {
        config.trust = true;
    }

    let shutdown = Arc::new(AtomicBool::new(false));
    let render_paused = Arc::new(AtomicBool::new(false));
    let render_acked = Arc::new(AtomicBool::new(false));
    let (state_tx, state_rx) = crossbeam_channel::bounded::<RenderState>(1);

    let shutdown_clone = shutdown.clone();
    let render_paused_clone = render_paused.clone();
    let render_acked_clone = render_acked.clone();
    let logic_handle = thread::spawn(move || {
        LogicThread::run(
            config,
            state_tx,
            shutdown_clone,
            render_paused_clone,
            render_acked_clone,
        )
    });

    let mut terminal = setup_terminal()?;
    let result = render_loop(
        &mut terminal,
        state_rx,
        &shutdown,
        &render_paused,
        &render_acked,
    );

    shutdown.store(true, Ordering::SeqCst);
    let _ = logic_handle.join();
    restore_terminal(&mut terminal)?;
    result
}

/// Start a new workflow with the given prompt.
///
/// Creates a SkillsOrchestrator and executes the full workflow.
/// In headless mode, outputs JSON status. Otherwise, launches TUI with progress.
fn run_workflow(prompt: String, headless: bool, trust: bool, debug: bool) -> Result<()> {
    zlog!(
        "Run command: prompt={:?}, headless={}, trust={}, debug={}",
        prompt,
        headless,
        trust,
        debug
    );

    // Get the current repository path
    let repo_path = std::env::current_dir()?;

    // Validate this is a git repository
    let git_ops = GitOps::new(&repo_path)?;
    let _ = git_ops; // Just validating the repo exists

    // Create workflow configuration
    let config = WorkflowConfig::default();

    if headless {
        // Headless mode: run workflow and output JSON status
        run_workflow_headless(prompt, config, repo_path)
    } else {
        // TUI mode: show progress while workflow runs
        run_workflow_with_tui(prompt, config, repo_path, trust)
    }
}

/// Run workflow in headless mode with JSON output.
fn run_workflow_headless(prompt: String, config: WorkflowConfig, repo_path: PathBuf) -> Result<()> {
    // Create async runtime for the orchestrator
    let rt = tokio::runtime::Runtime::new()?;

    let result = rt.block_on(async {
        // Create and execute the orchestrator
        let mut orchestrator = SkillsOrchestrator::new(config, &repo_path)?;
        orchestrator.execute(&prompt).await
    })?;

    // Save workflow state to git notes
    if let Err(e) = save_workflow_result(&repo_path, &result) {
        zlog!("Warning: Failed to save workflow state: {}", e);
    }

    // Output JSON result
    let json_output = serde_json::json!({
        "workflow_id": result.workflow_id.to_string(),
        "status": format!("{}", result.status),
        "summary": result.summary,
    });
    println!("{}", serde_json::to_string_pretty(&json_output)?);

    Ok(())
}

/// Run workflow with TUI progress display.
fn run_workflow_with_tui(
    prompt: String,
    config: WorkflowConfig,
    repo_path: PathBuf,
    trust: bool,
) -> Result<()> {
    println!("Starting workflow: {}", prompt);
    println!("Repository: {}", repo_path.display());
    println!();

    // Create async runtime for the orchestrator
    let rt = tokio::runtime::Runtime::new()?;

    // For now, run the orchestrator directly and show text progress
    // Full TUI integration will be added when we have the workflow TUI ready
    let result = rt.block_on(async {
        let mut orchestrator = SkillsOrchestrator::new(config.clone(), &repo_path)?;
        orchestrator.execute(&prompt).await
    })?;

    // Save workflow state
    if let Err(e) = save_workflow_result(&repo_path, &result) {
        zlog!("Warning: Failed to save workflow state: {}", e);
    }

    // Display result
    println!();
    println!("╔════════════════════════════════════════════════════════════╗");
    println!("║                    Workflow Complete                        ║");
    println!("╚════════════════════════════════════════════════════════════╝");
    println!();
    println!("  Workflow ID: {}", result.workflow_id.short());
    println!("  Status:      {}", format_status(result.status));
    println!("  Summary:     {}", result.summary);
    println!();

    if result.status == WorkflowStatus::Completed {
        println!("Next steps:");
        println!("  1. Review changes: zen review {}", result.workflow_id.short());
        println!("  2. Accept changes: zen accept {}", result.workflow_id.short());
        println!("  3. Reject changes: zen reject {}", result.workflow_id.short());
    }

    let _ = trust; // Silence unused warning for now
    Ok(())
}

/// Save workflow result to git state manager.
fn save_workflow_result(repo_path: &PathBuf, result: &WorkflowResult) -> Result<()> {
    let state_manager = GitStateManager::new(repo_path)?;

    // Create a workflow struct from the result
    let mut workflow = Workflow::new("", WorkflowConfig::default());
    workflow.id = result.workflow_id;
    workflow.status = result.status;
    if result.status == WorkflowStatus::Completed {
        workflow.complete();
    } else if result.status == WorkflowStatus::Failed {
        workflow.fail();
    }

    state_manager.save_workflow(&workflow)
}

/// Format workflow status with color codes for terminal.
fn format_status(status: WorkflowStatus) -> String {
    match status {
        WorkflowStatus::Completed => format!("\x1b[32m{}\x1b[0m", status), // Green
        WorkflowStatus::Failed => format!("\x1b[31m{}\x1b[0m", status),    // Red
        WorkflowStatus::Running => format!("\x1b[33m{}\x1b[0m", status),   // Yellow
        WorkflowStatus::Paused => format!("\x1b[34m{}\x1b[0m", status),    // Blue
        WorkflowStatus::Pending => format!("\x1b[90m{}\x1b[0m", status),   // Gray
        WorkflowStatus::Accepted => format!("\x1b[36m{}\x1b[0m", status),  // Cyan
        WorkflowStatus::Rejected => format!("\x1b[35m{}\x1b[0m", status),  // Magenta
    }
}

/// Review workflow results.
///
/// Loads the specified workflow (or latest if not specified) and displays:
/// - Workflow summary: tasks completed, status
/// - Files changed in the staging branch
/// - Diff vs main branch
fn run_review(workflow_id: Option<String>) -> Result<()> {
    zlog!("Review command: workflow_id={:?}", workflow_id);

    let repo_path = std::env::current_dir()?;
    let state_manager = GitStateManager::new(&repo_path)?;
    let git_ops = GitOps::new(&repo_path)?;

    // Load the workflow
    let workflow = match workflow_id {
        Some(id) => {
            // Try to parse as full UUID first, then as short prefix
            let wf_id = parse_workflow_id(&id, &state_manager)?;
            state_manager.load_workflow(&wf_id)?.ok_or_else(|| {
                zen::Error::Validation(format!("Workflow not found: {}", id))
            })?
        }
        None => {
            // Get the most recent workflow
            let workflows = state_manager.list_workflows()?;
            workflows
                .into_iter()
                .max_by_key(|w| w.created_at)
                .ok_or_else(|| zen::Error::Validation("No workflows found".to_string()))?
        }
    };

    // Display workflow summary
    println!();
    println!("╔════════════════════════════════════════════════════════════╗");
    println!("║                   Workflow Review                           ║");
    println!("╚════════════════════════════════════════════════════════════╝");
    println!();
    println!("  ID:          {}", workflow.id.short());
    println!("  Name:        {}", workflow.name);
    println!("  Prompt:      {}", truncate_string(&workflow.prompt, 50));
    println!("  Status:      {}", format_status(workflow.status));
    println!("  Phase:       {}", workflow.phase);
    println!("  Created:     {}", workflow.created_at.format("%Y-%m-%d %H:%M:%S UTC"));
    if let Some(started) = workflow.started_at {
        println!("  Started:     {}", started.format("%Y-%m-%d %H:%M:%S UTC"));
    }
    if let Some(completed) = workflow.completed_at {
        println!("  Completed:   {}", completed.format("%Y-%m-%d %H:%M:%S UTC"));
    }
    println!("  Tasks:       {}", workflow.task_ids.len());
    println!();

    // Get the staging branch name
    let staging_branch = format!(
        "{}{}",
        workflow.config.staging_branch_prefix,
        workflow.id.short()
    );

    // Check if staging branch exists and show diff
    let staging_exists = git_ops.branch_exists(&staging_branch)?;

    if staging_exists {
        println!("─────────────────────────────────────────────────────────────");
        println!("Staging Branch: {}", staging_branch);
        println!("─────────────────────────────────────────────────────────────");

        // Get diff summary between main and staging
        match get_diff_summary(&git_ops, &staging_branch) {
            Ok((files_changed, insertions, deletions)) => {
                println!();
                println!("  Changes vs main:");
                println!("    Files changed: {}", files_changed);
                println!("    Insertions:    \x1b[32m+{}\x1b[0m", insertions);
                println!("    Deletions:     \x1b[31m-{}\x1b[0m", deletions);
                println!();

                // Show file list
                if let Ok(files) = get_changed_files(&git_ops, &staging_branch) {
                    if !files.is_empty() {
                        println!("  Changed files:");
                        for file in files.iter().take(20) {
                            println!("    • {}", file);
                        }
                        if files.len() > 20 {
                            println!("    ... and {} more", files.len() - 20);
                        }
                        println!();
                    }
                }
            }
            Err(e) => {
                println!("  (Unable to compute diff: {})", e);
                println!();
            }
        }
    } else {
        println!("─────────────────────────────────────────────────────────────");
        println!("Staging Branch: {} (not found)", staging_branch);
        println!("─────────────────────────────────────────────────────────────");
        println!();
    }

    // Show warnings/issues
    let mut warnings = Vec::new();
    if workflow.status == WorkflowStatus::Failed {
        warnings.push("Workflow failed - check logs for details");
    }
    if !staging_exists && workflow.status == WorkflowStatus::Completed {
        warnings.push("Staging branch not found - workflow may not have completed merge phase");
    }

    if !warnings.is_empty() {
        println!("⚠️  Warnings:");
        for warning in &warnings {
            println!("    • {}", warning);
        }
        println!();
    }

    // Show next steps
    if workflow.status == WorkflowStatus::Completed && staging_exists {
        println!("Next steps:");
        println!("  • Accept changes:  zen accept {}", workflow.id.short());
        println!("  • Reject changes:  zen reject {}", workflow.id.short());
        println!("  • View full diff:  git diff main..{}", staging_branch);
    }

    Ok(())
}

/// Parse a workflow ID from string, supporting both full UUIDs and short prefixes.
fn parse_workflow_id(id: &str, state_manager: &GitStateManager) -> Result<WorkflowId> {
    // Try parsing as full UUID first
    if let Ok(wf_id) = id.parse::<WorkflowId>() {
        return Ok(wf_id);
    }

    // Try matching as a short prefix
    let workflows = state_manager.list_workflows()?;
    let matches: Vec<_> = workflows
        .iter()
        .filter(|w| w.id.short().starts_with(id) || w.id.to_string().starts_with(id))
        .collect();

    match matches.len() {
        0 => Err(zen::Error::Validation(format!("No workflow matching '{}'", id))),
        1 => Ok(matches[0].id),
        _ => Err(zen::Error::Validation(format!(
            "Ambiguous workflow ID '{}' matches {} workflows",
            id,
            matches.len()
        ))),
    }
}

/// Get diff summary (files changed, insertions, deletions) between main and a branch.
fn get_diff_summary(git_ops: &GitOps, branch: &str) -> Result<(usize, usize, usize)> {
    // Use git diff-tree to get stats
    let output = std::process::Command::new("git")
        .args(["diff", "--shortstat", "main", branch])
        .current_dir(git_ops.repo_path())
        .output()?;

    if !output.status.success() {
        return Err(zen::Error::Validation("Failed to get diff stats".to_string()));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    // Parse: "X files changed, Y insertions(+), Z deletions(-)"
    let mut files = 0;
    let mut insertions = 0;
    let mut deletions = 0;

    for part in stdout.split(',') {
        let part = part.trim();
        if part.contains("file") {
            files = part.split_whitespace().next().unwrap_or("0").parse().unwrap_or(0);
        } else if part.contains("insertion") {
            insertions = part.split_whitespace().next().unwrap_or("0").parse().unwrap_or(0);
        } else if part.contains("deletion") {
            deletions = part.split_whitespace().next().unwrap_or("0").parse().unwrap_or(0);
        }
    }

    Ok((files, insertions, deletions))
}

/// Get list of changed files between main and a branch.
fn get_changed_files(git_ops: &GitOps, branch: &str) -> Result<Vec<String>> {
    let output = std::process::Command::new("git")
        .args(["diff", "--name-only", "main", branch])
        .current_dir(git_ops.repo_path())
        .output()?;

    if !output.status.success() {
        return Err(zen::Error::Validation("Failed to get changed files".to_string()));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    Ok(stdout.lines().map(|s| s.to_string()).collect())
}

/// Truncate a string to a maximum length, adding "..." if truncated.
fn truncate_string(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len - 3])
    }
}

/// Accept workflow: merge staging branch to main and clean up worktrees.
///
/// This function:
/// 1. Loads the workflow (most recent or specified)
/// 2. Verifies the workflow is in Completed status
/// 3. Prompts for confirmation (unless --yes is passed)
/// 4. Merges the staging branch to main
/// 5. Cleans up task worktrees
/// 6. Marks the workflow as Accepted
/// 7. Optionally deletes the staging branch
fn run_accept(workflow_id: Option<String>, skip_confirm: bool) -> Result<()> {
    zlog!("Accept command: workflow_id={:?}, yes={}", workflow_id, skip_confirm);

    let repo_path = std::env::current_dir()?;
    let state_manager = GitStateManager::new(&repo_path)?;
    let git_ops = GitOps::new(&repo_path)?;

    // Load the workflow
    let mut workflow = match workflow_id {
        Some(id) => {
            let wf_id = parse_workflow_id(&id, &state_manager)?;
            state_manager.load_workflow(&wf_id)?.ok_or_else(|| {
                zen::Error::Validation(format!("Workflow not found: {}", id))
            })?
        }
        None => {
            let workflows = state_manager.list_workflows()?;
            workflows
                .into_iter()
                .filter(|w| w.status == WorkflowStatus::Completed)
                .max_by_key(|w| w.created_at)
                .ok_or_else(|| {
                    zen::Error::Validation("No completed workflows found to accept".to_string())
                })?
        }
    };

    // Verify workflow is completed
    if workflow.status != WorkflowStatus::Completed {
        return Err(zen::Error::Validation(format!(
            "Cannot accept workflow with status '{}'. Only completed workflows can be accepted.",
            workflow.status
        )));
    }

    // Get the staging branch name
    let staging_branch = format!(
        "{}{}",
        workflow.config.staging_branch_prefix,
        workflow.id.short()
    );

    // Verify staging branch exists
    if !git_ops.branch_exists(&staging_branch)? {
        return Err(zen::Error::Validation(format!(
            "Staging branch '{}' does not exist. The workflow may not have completed the merge phase.",
            staging_branch
        )));
    }

    // Get diff summary for display
    let (files_changed, insertions, deletions) = get_diff_summary(&git_ops, &staging_branch)
        .unwrap_or((0, 0, 0));

    // Display summary
    println!();
    println!("╔════════════════════════════════════════════════════════════╗");
    println!("║                   Accept Workflow                          ║");
    println!("╚════════════════════════════════════════════════════════════╝");
    println!();
    println!("  Workflow:     {} ({})", workflow.name, workflow.id.short());
    println!("  Branch:       {}", staging_branch);
    println!("  Changes:      {} files, \x1b[32m+{}\x1b[0m/\x1b[31m-{}\x1b[0m",
             files_changed, insertions, deletions);
    println!();

    // Confirmation prompt
    if !skip_confirm {
        println!("This will merge '{}' into the main branch.", staging_branch);
        print!("Continue? [y/N] ");
        std::io::Write::flush(&mut std::io::stdout())?;

        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;
        let input = input.trim().to_lowercase();

        if input != "y" && input != "yes" {
            println!("\nAccept cancelled.");
            return Ok(());
        }
    }

    println!();
    println!("Merging to main...");

    // Merge staging branch to main
    let merge_commit = git_ops.merge_branch_to_main(&staging_branch)?;
    println!("  Merged: {}", &merge_commit[..8]);

    // Clean up worktrees associated with this workflow
    println!("Cleaning up worktrees...");
    let worktree_prefix = workflow.id.short();
    let worktrees = git_ops.list_worktrees_with_prefix(&worktree_prefix)?;
    let mut cleaned = 0;
    for worktree_path in worktrees {
        if let Err(e) = git_ops.remove_worktree(&worktree_path) {
            zlog!("Warning: Failed to remove worktree {:?}: {}", worktree_path, e);
        } else {
            cleaned += 1;
        }
    }
    if cleaned > 0 {
        println!("  Removed {} worktree(s)", cleaned);
    }

    // Delete the staging branch (optional - keep for debugging if it fails)
    if let Err(e) = git_ops.delete_branch(&staging_branch) {
        zlog!("Warning: Failed to delete staging branch: {}", e);
    } else {
        println!("  Deleted staging branch");
    }

    // Mark workflow as accepted and save
    workflow.accept();
    state_manager.save_workflow(&workflow)?;

    println!();
    println!("\x1b[32mWorkflow accepted successfully!\x1b[0m");
    println!();
    println!("  Merge commit: {}", merge_commit);
    println!("  Status:       {}", format_status(workflow.status));

    Ok(())
}

/// Reject workflow: discard changes, delete staging branch, clean up worktrees.
///
/// This function:
/// 1. Loads the specified workflow
/// 2. Deletes the staging branch
/// 3. Cleans up task worktrees
/// 4. Marks the workflow as Rejected
/// 5. Preserves task branches for debugging
fn run_reject(workflow_id: String) -> Result<()> {
    zlog!("Reject command: workflow_id={}", workflow_id);

    let repo_path = std::env::current_dir()?;
    let state_manager = GitStateManager::new(&repo_path)?;
    let git_ops = GitOps::new(&repo_path)?;

    // Load the workflow
    let wf_id = parse_workflow_id(&workflow_id, &state_manager)?;
    let mut workflow = state_manager.load_workflow(&wf_id)?.ok_or_else(|| {
        zen::Error::Validation(format!("Workflow not found: {}", workflow_id))
    })?;

    // Check if workflow can be rejected
    if workflow.status == WorkflowStatus::Accepted {
        return Err(zen::Error::Validation(
            "Cannot reject an already accepted workflow.".to_string()
        ));
    }
    if workflow.status == WorkflowStatus::Rejected {
        return Err(zen::Error::Validation(
            "Workflow has already been rejected.".to_string()
        ));
    }

    // Get the staging branch name
    let staging_branch = format!(
        "{}{}",
        workflow.config.staging_branch_prefix,
        workflow.id.short()
    );

    // Display summary
    println!();
    println!("╔════════════════════════════════════════════════════════════╗");
    println!("║                   Reject Workflow                          ║");
    println!("╚════════════════════════════════════════════════════════════╝");
    println!();
    println!("  Workflow:     {} ({})", workflow.name, workflow.id.short());
    println!("  Status:       {}", format_status(workflow.status));
    println!();

    // Delete staging branch if it exists
    if git_ops.branch_exists(&staging_branch)? {
        println!("Deleting staging branch...");
        if let Err(e) = git_ops.delete_branch(&staging_branch) {
            zlog!("Warning: Failed to delete staging branch: {}", e);
            println!("  Warning: Could not delete '{}'", staging_branch);
        } else {
            println!("  Deleted: {}", staging_branch);
        }
    } else {
        println!("  Staging branch '{}' not found (already deleted?)", staging_branch);
    }

    // Clean up worktrees associated with this workflow
    println!("Cleaning up worktrees...");
    let worktree_prefix = workflow.id.short();
    let worktrees = git_ops.list_worktrees_with_prefix(&worktree_prefix)?;
    let mut cleaned = 0;
    for worktree_path in worktrees {
        if let Err(e) = git_ops.remove_worktree(&worktree_path) {
            zlog!("Warning: Failed to remove worktree {:?}: {}", worktree_path, e);
        } else {
            cleaned += 1;
        }
    }
    if cleaned > 0 {
        println!("  Removed {} worktree(s)", cleaned);
    } else {
        println!("  No worktrees to clean up");
    }

    // Note: Task branches are preserved for debugging (per requirements)
    println!();
    println!("Note: Task branches are preserved for debugging.");
    println!("To delete them manually, run: git branch -D <branch-name>");

    // Mark workflow as rejected and save
    workflow.reject();
    state_manager.save_workflow(&workflow)?;

    println!();
    println!("\x1b[33mWorkflow rejected.\x1b[0m");
    println!();
    println!("  Status: {}", format_status(workflow.status));

    Ok(())
}

/// Placeholder: Show status
fn run_status() -> Result<()> {
    zlog!("Status command");
    println!("Workflow Status:");
    println!("  No active workflows");
    // TODO: Implement with GitStateManager
    Ok(())
}

/// Placeholder: Attach to agent
fn run_attach(agent_id: String) -> Result<()> {
    zlog!("Attach command: agent_id={}", agent_id);
    println!("Attaching to agent: {}", agent_id);
    // TODO: Implement with tmux integration
    Ok(())
}

/// Clean up orphaned resources (worktrees, tmux sessions, branches).
///
/// This function:
/// 1. Scans for orphaned worktrees in ~/.zen/worktrees/
/// 2. Scans for orphaned tmux sessions (zen_* without active agents)
/// 3. Scans for orphaned branches (zen/* without linked workflows)
/// 4. Reports findings (or deletes with --delete flag)
fn run_cleanup(delete: bool, skip_confirm: bool) -> Result<()> {
    zlog!("Cleanup command: delete={}, yes={}", delete, skip_confirm);

    let repo_path = std::env::current_dir()?;
    let git_ops = GitOps::new(&repo_path)?;
    let state_manager = GitStateManager::new(&repo_path)?;

    // Get known workflow IDs
    let workflows = state_manager.list_workflows().unwrap_or_default();
    let known_ids: HashSet<String> = workflows
        .iter()
        .flat_map(|w| vec![w.id.short(), w.id.to_string()])
        .collect();

    // For now, we don't have a way to get active agent IDs without running workflows
    // Use the same known_ids for tmux session detection
    let active_agent_ids: HashSet<String> = known_ids.clone();

    let config = CleanupConfig::default();
    let cleanup_manager = CleanupManager::new(git_ops, config);

    // Detect orphans
    let orphaned_worktrees = cleanup_manager.detect_orphaned_worktrees(&known_ids);
    let orphaned_tmux = cleanup_manager.detect_orphaned_tmux(&active_agent_ids);
    let orphaned_branches = cleanup_manager.detect_orphaned_branches(&known_ids);

    let total_orphans = orphaned_worktrees.len() + orphaned_tmux.len() + orphaned_branches.len();

    // Display header
    println!();
    println!("╔════════════════════════════════════════════════════════════╗");
    println!("║                    Orphan Detection                         ║");
    println!("╚════════════════════════════════════════════════════════════╝");
    println!();

    // Report worktrees
    println!("📁 Orphaned Worktrees: {}", orphaned_worktrees.len());
    if !orphaned_worktrees.is_empty() {
        for path in &orphaned_worktrees {
            println!("   • {}", path.display());
        }
    }
    println!();

    // Report tmux sessions
    println!("🖥️  Orphaned Tmux Sessions: {}", orphaned_tmux.len());
    if !orphaned_tmux.is_empty() {
        for session in &orphaned_tmux {
            println!("   • {}", session);
        }
    }
    println!();

    // Report branches
    println!("🌿 Orphaned Branches: {}", orphaned_branches.len());
    if !orphaned_branches.is_empty() {
        for branch in &orphaned_branches {
            println!("   • {}", branch);
        }
    }
    println!();

    // Summary
    println!("─────────────────────────────────────────────────────────────");
    println!("Total orphaned resources: {}", total_orphans);
    println!();

    if total_orphans == 0 {
        println!("\x1b[32mNo orphaned resources found. System is clean!\x1b[0m");
        return Ok(());
    }

    if !delete {
        println!("Run with --delete to remove orphaned resources.");
        println!("Example: zen cleanup --delete");
        return Ok(());
    }

    // Confirmation prompt
    if !skip_confirm {
        println!("\x1b[33mWarning: This will permanently delete {} orphaned resource(s).\x1b[0m", total_orphans);
        print!("Continue? [y/N] ");
        std::io::Write::flush(&mut std::io::stdout())?;

        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;
        let input = input.trim().to_lowercase();

        if input != "y" && input != "yes" {
            println!("\nCleanup cancelled.");
            return Ok(());
        }
    }

    println!();
    println!("Cleaning up orphaned resources...");
    println!();

    // Clean up worktrees
    let mut worktrees_removed = 0;
    if !orphaned_worktrees.is_empty() {
        match cleanup_manager.remove_orphaned(&orphaned_worktrees) {
            Ok(report) => {
                worktrees_removed = report.removed_count();
                if report.failed_count() > 0 {
                    for (path, error) in &report.failed {
                        println!("  ⚠️  Failed to remove worktree: {} ({})", path.display(), error);
                    }
                }
            }
            Err(e) => {
                println!("  ⚠️  Error removing worktrees: {}", e);
            }
        }
    }

    // Clean up tmux sessions
    let tmux_killed = orphaned_tmux.len();
    if !orphaned_tmux.is_empty() {
        cleanup_manager.remove_orphaned_tmux(&orphaned_tmux);
    }

    // Clean up branches
    let branches_deleted = orphaned_branches.len();
    if !orphaned_branches.is_empty() {
        cleanup_manager.remove_orphaned_branches(&orphaned_branches);
    }

    // Report results
    println!("─────────────────────────────────────────────────────────────");
    println!("\x1b[32mCleanup complete!\x1b[0m");
    println!();
    println!("  Worktrees removed:      {}", worktrees_removed);
    println!("  Tmux sessions killed:   {}", tmux_killed);
    println!("  Branches deleted:       {}", branches_deleted);

    Ok(())
}

fn render_loop(
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    state_rx: Receiver<RenderState>,
    shutdown: &AtomicBool,
    render_paused: &AtomicBool,
    render_acked: &AtomicBool,
) -> Result<()> {
    let mut state = RenderState::default();
    let mut last_version: u64 = 0;
    let mut last_frame = Instant::now();
    let mut dirty = true;

    loop {
        if shutdown.load(Ordering::Relaxed) {
            break;
        }

        if render_paused.load(Ordering::Acquire) {
            render_acked.store(true, Ordering::Release);
            while render_paused.load(Ordering::Acquire) {
                thread::sleep(Duration::from_millis(1));
            }
            render_acked.store(false, Ordering::Release);
            terminal.clear()?;
            dirty = true;
            continue;
        }

        match state_rx.try_recv() {
            Ok(s) => {
                dirty = dirty || s.version != last_version;
                state = s;
            }
            Err(TryRecvError::Empty) => {}
            Err(TryRecvError::Disconnected) => break,
        }

        if last_frame.elapsed() < FRAME_DURATION {
            thread::sleep(Duration::from_micros(500));
            continue;
        }
        last_frame = Instant::now();

        if dirty {
            terminal.draw(|f| ui::draw(f, &state))?;
            last_version = state.version;
            dirty = false;
        }
    }
    Ok(())
}

fn setup_terminal() -> Result<Terminal<CrosstermBackend<Stdout>>> {
    enable_raw_mode()?;
    execute!(io::stdout(), EnterAlternateScreen)?;
    let mut terminal = Terminal::new(CrosstermBackend::new(stdout()))?;
    terminal.hide_cursor()?;
    terminal.clear()?;
    Ok(terminal)
}

fn restore_terminal(terminal: &mut Terminal<CrosstermBackend<Stdout>>) -> Result<()> {
    terminal.show_cursor()?;
    execute!(io::stdout(), LeaveAlternateScreen)?;
    Ok(disable_raw_mode()?)
}

fn run_reset(force: bool) -> Result<()> {
    use zen::session::State;

    println!("Resetting zen...");
    zlog!("Reset command initiated (force={})", force);

    let (session_count, tmux_count, worktree_count, branches_count, skipped_sessions) =
        State::reset_all(force)?;

    if !skipped_sessions.is_empty() {
        println!(
            "\nWarning: Skipping {} session(s) with uncommitted work:",
            skipped_sessions.len()
        );
        for name in &skipped_sessions {
            println!("  - {}", name);
        }
        println!("Use 'zen reset --force' to delete these sessions anyway.\n");
    }

    println!("\nReset complete!");
    println!("  Sessions deleted: {}", session_count);
    if !skipped_sessions.is_empty() {
        println!("  Sessions skipped (dirty): {}", skipped_sessions.len());
    }
    println!("  Tmux sessions killed: {}", tmux_count);
    println!("  Worktrees removed: {}", worktree_count);
    println!("  Branches deleted: {}", branches_count);
    zlog!(
        "Reset command completed: {} sessions, {} tmux, {} worktrees, {} branches, {} skipped",
        session_count,
        tmux_count,
        worktree_count,
        branches_count,
        skipped_sessions.len()
    );

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[test]
    fn test_run_command_basic() {
        let cli = Cli::try_parse_from(["zen", "run", "build auth"]).unwrap();
        assert!(!cli.trust);
        assert!(!cli.debug);
        match cli.command {
            Some(Command::Run { prompt, headless }) => {
                assert_eq!(prompt, "build auth");
                assert!(!headless);
            }
            _ => panic!("Expected Run command"),
        }
    }

    #[test]
    fn test_run_command_with_headless() {
        let cli = Cli::try_parse_from(["zen", "run", "--headless", "build auth"]).unwrap();
        match cli.command {
            Some(Command::Run { prompt, headless }) => {
                assert_eq!(prompt, "build auth");
                assert!(headless);
            }
            _ => panic!("Expected Run command with headless"),
        }
    }

    #[test]
    fn test_no_command_returns_none() {
        let cli = Cli::try_parse_from(["zen"]).unwrap();
        assert!(cli.command.is_none());
        assert!(!cli.trust);
        assert!(!cli.debug);
    }

    #[test]
    fn test_trust_flag_works() {
        let cli = Cli::try_parse_from(["zen", "--trust"]).unwrap();
        assert!(cli.trust);
        assert!(cli.command.is_none());
    }

    #[test]
    fn test_trust_flag_short() {
        let cli = Cli::try_parse_from(["zen", "-t"]).unwrap();
        assert!(cli.trust);
    }

    #[test]
    fn test_debug_flag_works() {
        let cli = Cli::try_parse_from(["zen", "--debug"]).unwrap();
        assert!(cli.debug);
    }

    #[test]
    fn test_debug_flag_short() {
        let cli = Cli::try_parse_from(["zen", "-d"]).unwrap();
        assert!(cli.debug);
    }

    #[test]
    fn test_combined_flags() {
        let cli = Cli::try_parse_from(["zen", "-t", "-d"]).unwrap();
        assert!(cli.trust);
        assert!(cli.debug);
    }

    #[test]
    fn test_review_command_no_id() {
        let cli = Cli::try_parse_from(["zen", "review"]).unwrap();
        match cli.command {
            Some(Command::Review { workflow_id }) => {
                assert!(workflow_id.is_none());
            }
            _ => panic!("Expected Review command"),
        }
    }

    #[test]
    fn test_review_command_with_id() {
        let cli = Cli::try_parse_from(["zen", "review", "wf-123"]).unwrap();
        match cli.command {
            Some(Command::Review { workflow_id }) => {
                assert_eq!(workflow_id, Some("wf-123".to_string()));
            }
            _ => panic!("Expected Review command"),
        }
    }

    #[test]
    fn test_accept_command_no_id() {
        let cli = Cli::try_parse_from(["zen", "accept"]).unwrap();
        match cli.command {
            Some(Command::Accept { workflow_id, yes }) => {
                assert!(workflow_id.is_none());
                assert!(!yes);
            }
            _ => panic!("Expected Accept command"),
        }
    }

    #[test]
    fn test_accept_command_with_id() {
        let cli = Cli::try_parse_from(["zen", "accept", "wf-456"]).unwrap();
        match cli.command {
            Some(Command::Accept { workflow_id, yes }) => {
                assert_eq!(workflow_id, Some("wf-456".to_string()));
                assert!(!yes);
            }
            _ => panic!("Expected Accept command"),
        }
    }

    #[test]
    fn test_accept_command_with_yes_flag() {
        let cli = Cli::try_parse_from(["zen", "accept", "--yes"]).unwrap();
        match cli.command {
            Some(Command::Accept { workflow_id, yes }) => {
                assert!(workflow_id.is_none());
                assert!(yes);
            }
            _ => panic!("Expected Accept command with --yes"),
        }
    }

    #[test]
    fn test_accept_command_with_yes_short_flag() {
        let cli = Cli::try_parse_from(["zen", "accept", "-y"]).unwrap();
        match cli.command {
            Some(Command::Accept { workflow_id, yes }) => {
                assert!(workflow_id.is_none());
                assert!(yes);
            }
            _ => panic!("Expected Accept command with -y"),
        }
    }

    #[test]
    fn test_accept_command_with_id_and_yes() {
        let cli = Cli::try_parse_from(["zen", "accept", "wf-123", "--yes"]).unwrap();
        match cli.command {
            Some(Command::Accept { workflow_id, yes }) => {
                assert_eq!(workflow_id, Some("wf-123".to_string()));
                assert!(yes);
            }
            _ => panic!("Expected Accept command with id and --yes"),
        }
    }

    #[test]
    fn test_reject_command() {
        let cli = Cli::try_parse_from(["zen", "reject", "wf-789"]).unwrap();
        match cli.command {
            Some(Command::Reject { workflow_id }) => {
                assert_eq!(workflow_id, "wf-789");
            }
            _ => panic!("Expected Reject command"),
        }
    }

    #[test]
    fn test_reject_command_requires_id() {
        // Reject requires a workflow_id argument
        let result = Cli::try_parse_from(["zen", "reject"]);
        assert!(result.is_err());
    }

    #[test]
    fn test_status_command() {
        let cli = Cli::try_parse_from(["zen", "status"]).unwrap();
        assert!(matches!(cli.command, Some(Command::Status)));
    }

    #[test]
    fn test_attach_command() {
        let cli = Cli::try_parse_from(["zen", "attach", "agent-abc"]).unwrap();
        match cli.command {
            Some(Command::Attach { agent_id }) => {
                assert_eq!(agent_id, "agent-abc");
            }
            _ => panic!("Expected Attach command"),
        }
    }

    #[test]
    fn test_reset_command_no_force() {
        let cli = Cli::try_parse_from(["zen", "reset"]).unwrap();
        match cli.command {
            Some(Command::Reset { force }) => {
                assert!(!force);
            }
            _ => panic!("Expected Reset command"),
        }
    }

    #[test]
    fn test_reset_command_with_force() {
        let cli = Cli::try_parse_from(["zen", "reset", "--force"]).unwrap();
        match cli.command {
            Some(Command::Reset { force }) => {
                assert!(force);
            }
            _ => panic!("Expected Reset command with force"),
        }
    }

    #[test]
    fn test_flags_with_subcommand() {
        let cli = Cli::try_parse_from(["zen", "-t", "-d", "run", "test"]).unwrap();
        assert!(cli.trust);
        assert!(cli.debug);
        match cli.command {
            Some(Command::Run { prompt, headless }) => {
                assert_eq!(prompt, "test");
                assert!(!headless);
            }
            _ => panic!("Expected Run command"),
        }
    }

    #[test]
    fn test_unknown_command_fails() {
        let result = Cli::try_parse_from(["zen", "unknown"]);
        assert!(result.is_err());
    }

    #[test]
    fn test_help_output_exists() {
        // Just ensure we can build the help without panicking
        use clap::CommandFactory;
        let help = Cli::command().render_help();
        let help_str = help.to_string();
        assert!(help_str.contains("run"));
        assert!(help_str.contains("review"));
        assert!(help_str.contains("accept"));
        assert!(help_str.contains("reject"));
        assert!(help_str.contains("status"));
        assert!(help_str.contains("attach"));
        assert!(help_str.contains("reset"));
        assert!(help_str.contains("cleanup"));
    }

    #[test]
    fn test_command_equality() {
        // Test that Command derives PartialEq correctly
        let cmd1 = Command::Run {
            prompt: "test".to_string(),
            headless: false,
        };
        let cmd2 = Command::Run {
            prompt: "test".to_string(),
            headless: false,
        };
        assert_eq!(cmd1, cmd2);

        let cmd3 = Command::Run {
            prompt: "test".to_string(),
            headless: true,
        };
        assert_ne!(cmd1, cmd3);
    }

    #[test]
    fn test_cleanup_command_no_flags() {
        let cli = Cli::try_parse_from(["zen", "cleanup"]).unwrap();
        match cli.command {
            Some(Command::Cleanup { delete, yes }) => {
                assert!(!delete);
                assert!(!yes);
            }
            _ => panic!("Expected Cleanup command"),
        }
    }

    #[test]
    fn test_cleanup_command_with_delete() {
        let cli = Cli::try_parse_from(["zen", "cleanup", "--delete"]).unwrap();
        match cli.command {
            Some(Command::Cleanup { delete, yes }) => {
                assert!(delete);
                assert!(!yes);
            }
            _ => panic!("Expected Cleanup command with delete"),
        }
    }

    #[test]
    fn test_cleanup_command_with_yes() {
        let cli = Cli::try_parse_from(["zen", "cleanup", "-y"]).unwrap();
        match cli.command {
            Some(Command::Cleanup { delete, yes }) => {
                assert!(!delete);
                assert!(yes);
            }
            _ => panic!("Expected Cleanup command with yes"),
        }
    }

    #[test]
    fn test_cleanup_command_with_delete_and_yes() {
        let cli = Cli::try_parse_from(["zen", "cleanup", "--delete", "--yes"]).unwrap();
        match cli.command {
            Some(Command::Cleanup { delete, yes }) => {
                assert!(delete);
                assert!(yes);
            }
            _ => panic!("Expected Cleanup command with both flags"),
        }
    }

    #[test]
    fn test_cleanup_command_short_yes_flag() {
        let cli = Cli::try_parse_from(["zen", "cleanup", "--delete", "-y"]).unwrap();
        match cli.command {
            Some(Command::Cleanup { delete, yes }) => {
                assert!(delete);
                assert!(yes);
            }
            _ => panic!("Expected Cleanup command with both flags"),
        }
    }

    // Task 17.2 tests for run and review commands

    #[test]
    fn test_format_status_completed() {
        let formatted = format_status(WorkflowStatus::Completed);
        assert!(formatted.contains("completed"));
        assert!(formatted.contains("\x1b[32m")); // Green color
    }

    #[test]
    fn test_format_status_failed() {
        let formatted = format_status(WorkflowStatus::Failed);
        assert!(formatted.contains("failed"));
        assert!(formatted.contains("\x1b[31m")); // Red color
    }

    #[test]
    fn test_format_status_running() {
        let formatted = format_status(WorkflowStatus::Running);
        assert!(formatted.contains("running"));
        assert!(formatted.contains("\x1b[33m")); // Yellow color
    }

    #[test]
    fn test_format_status_paused() {
        let formatted = format_status(WorkflowStatus::Paused);
        assert!(formatted.contains("paused"));
        assert!(formatted.contains("\x1b[34m")); // Blue color
    }

    #[test]
    fn test_format_status_pending() {
        let formatted = format_status(WorkflowStatus::Pending);
        assert!(formatted.contains("pending"));
        assert!(formatted.contains("\x1b[90m")); // Gray color
    }

    #[test]
    fn test_truncate_string_short() {
        let result = truncate_string("hello", 10);
        assert_eq!(result, "hello");
    }

    #[test]
    fn test_truncate_string_exact() {
        let result = truncate_string("hello", 5);
        assert_eq!(result, "hello");
    }

    #[test]
    fn test_truncate_string_long() {
        let result = truncate_string("hello world this is a long string", 20);
        assert_eq!(result, "hello world this ...");
    }

    #[test]
    fn test_truncate_string_minimum() {
        let result = truncate_string("hello world", 5);
        assert_eq!(result, "he...");
    }

    #[test]
    fn test_workflow_config_defaults() {
        let config = WorkflowConfig::default();
        assert!(config.update_docs);
        assert_eq!(config.max_parallel_agents, 4);
        assert_eq!(config.staging_branch_prefix, "zen/staging/");
    }

    #[test]
    fn test_run_command_variations() {
        // Test run with quoted multi-word prompt
        let cli = Cli::try_parse_from(["zen", "run", "build user authentication system"]).unwrap();
        match cli.command {
            Some(Command::Run { prompt, headless }) => {
                assert_eq!(prompt, "build user authentication system");
                assert!(!headless);
            }
            _ => panic!("Expected Run command"),
        }
    }

    #[test]
    fn test_run_with_all_flags() {
        let cli = Cli::try_parse_from(["zen", "-t", "-d", "run", "--headless", "test prompt"]).unwrap();
        assert!(cli.trust);
        assert!(cli.debug);
        match cli.command {
            Some(Command::Run { prompt, headless }) => {
                assert_eq!(prompt, "test prompt");
                assert!(headless);
            }
            _ => panic!("Expected Run command with all flags"),
        }
    }

    #[test]
    fn test_review_with_short_id() {
        let cli = Cli::try_parse_from(["zen", "review", "abc12345"]).unwrap();
        match cli.command {
            Some(Command::Review { workflow_id }) => {
                assert_eq!(workflow_id, Some("abc12345".to_string()));
            }
            _ => panic!("Expected Review command"),
        }
    }

    #[test]
    fn test_review_with_full_uuid() {
        let cli = Cli::try_parse_from(["zen", "review", "550e8400-e29b-41d4-a716-446655440000"]).unwrap();
        match cli.command {
            Some(Command::Review { workflow_id }) => {
                assert_eq!(workflow_id, Some("550e8400-e29b-41d4-a716-446655440000".to_string()));
            }
            _ => panic!("Expected Review command with full UUID"),
        }
    }

    // Task 17.3 tests for accept and reject commands

    #[test]
    fn test_format_status_accepted() {
        let formatted = format_status(WorkflowStatus::Accepted);
        assert!(formatted.contains("accepted"));
        assert!(formatted.contains("\x1b[36m")); // Cyan color
    }

    #[test]
    fn test_format_status_rejected() {
        let formatted = format_status(WorkflowStatus::Rejected);
        assert!(formatted.contains("rejected"));
        assert!(formatted.contains("\x1b[35m")); // Magenta color
    }

    #[test]
    fn test_accept_with_all_combinations() {
        // Test: zen accept -y wf-id
        let cli = Cli::try_parse_from(["zen", "accept", "-y", "my-workflow"]).unwrap();
        match cli.command {
            Some(Command::Accept { workflow_id, yes }) => {
                assert_eq!(workflow_id, Some("my-workflow".to_string()));
                assert!(yes);
            }
            _ => panic!("Expected Accept command"),
        }
    }

    #[test]
    fn test_reject_with_short_id() {
        let cli = Cli::try_parse_from(["zen", "reject", "abc12345"]).unwrap();
        match cli.command {
            Some(Command::Reject { workflow_id }) => {
                assert_eq!(workflow_id, "abc12345");
            }
            _ => panic!("Expected Reject command"),
        }
    }

    #[test]
    fn test_reject_with_full_uuid() {
        let cli = Cli::try_parse_from(["zen", "reject", "550e8400-e29b-41d4-a716-446655440000"]).unwrap();
        match cli.command {
            Some(Command::Reject { workflow_id }) => {
                assert_eq!(workflow_id, "550e8400-e29b-41d4-a716-446655440000");
            }
            _ => panic!("Expected Reject command with full UUID"),
        }
    }

    #[test]
    fn test_workflow_status_accept_method() {
        let mut workflow = Workflow::new("test prompt", WorkflowConfig::default());
        workflow.complete();
        assert_eq!(workflow.status, WorkflowStatus::Completed);

        workflow.accept();
        assert_eq!(workflow.status, WorkflowStatus::Accepted);
    }

    #[test]
    fn test_workflow_status_reject_method() {
        let mut workflow = Workflow::new("test prompt", WorkflowConfig::default());
        workflow.complete();
        assert_eq!(workflow.status, WorkflowStatus::Completed);

        workflow.reject();
        assert_eq!(workflow.status, WorkflowStatus::Rejected);
    }
}
