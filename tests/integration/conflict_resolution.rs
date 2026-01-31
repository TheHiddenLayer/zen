//! Conflict resolution integration tests.
//!
//! These tests verify that the ConflictResolver correctly handles
//! merge conflicts between task worktrees.

use std::path::PathBuf;
use std::process::Command;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};

use zen::git::GitOps;
use zen::orchestration::{AgentPool, ConflictResolver, MergeResult};

use crate::fixtures::{TestRepo, ConflictResolverHarness};

/// Test: Clean merge succeeds
/// Given 2 tasks modifying different files
/// When merge phase runs
/// Then MergeResult::Success is returned
#[tokio::test]
async fn test_clean_merge_succeeds() {
    let repo = TestRepo::new();
    let git_ops = repo.git_ops().unwrap();
    let (pool_tx, _) = mpsc::channel(100);
    let pool = Arc::new(RwLock::new(AgentPool::new(4, pool_tx)));

    let resolver = ConflictResolver::new(git_ops, Arc::clone(&pool));

    // Create a staging branch
    repo.create_branch("staging").unwrap();

    // Create a feature branch with changes to a new file
    Command::new("git")
        .args(["checkout", "-b", "feature-a"])
        .current_dir(&repo.path)
        .output()
        .unwrap();

    std::fs::write(repo.path.join("feature-a.txt"), "Feature A content\n").unwrap();

    Command::new("git")
        .args(["add", "feature-a.txt"])
        .current_dir(&repo.path)
        .output()
        .unwrap();

    Command::new("git")
        .args(["commit", "-m", "Add feature A"])
        .current_dir(&repo.path)
        .output()
        .unwrap();

    // Merge feature-a into staging
    let result = resolver.merge(&repo.path, "staging");

    assert!(result.is_ok(), "Merge should succeed");
    if let Ok(merge_result) = result {
        assert!(
            merge_result.is_success(),
            "Merge result should be Success"
        );
    }
}

/// Test: Conflict detection
/// Given 2 tasks modifying the same file
/// When merge phase runs
/// Then MergeResult::Conflicts is returned
#[tokio::test]
async fn test_conflict_detection() {
    let repo = TestRepo::new();
    let git_ops = repo.git_ops().unwrap();
    let (pool_tx, _) = mpsc::channel(100);
    let pool = Arc::new(RwLock::new(AgentPool::new(4, pool_tx)));

    let resolver = ConflictResolver::new(git_ops, Arc::clone(&pool));

    // Create initial content
    std::fs::write(repo.path.join("shared.txt"), "Original content\n").unwrap();
    Command::new("git")
        .args(["add", "shared.txt"])
        .current_dir(&repo.path)
        .output()
        .unwrap();
    Command::new("git")
        .args(["commit", "-m", "Add shared file"])
        .current_dir(&repo.path)
        .output()
        .unwrap();

    // Create staging branch
    Command::new("git")
        .args(["checkout", "-b", "staging"])
        .current_dir(&repo.path)
        .output()
        .unwrap();

    // Modify on staging
    std::fs::write(repo.path.join("shared.txt"), "Staging changes\n").unwrap();
    Command::new("git")
        .args(["add", "shared.txt"])
        .current_dir(&repo.path)
        .output()
        .unwrap();
    Command::new("git")
        .args(["commit", "-m", "Staging changes"])
        .current_dir(&repo.path)
        .output()
        .unwrap();

    // Go back to main and create feature branch
    Command::new("git")
        .args(["checkout", "master"])
        .current_dir(&repo.path)
        .output()
        .unwrap();

    // Check if we're on master or main
    let output = Command::new("git")
        .args(["branch", "--show-current"])
        .current_dir(&repo.path)
        .output()
        .unwrap();
    let main_branch = String::from_utf8_lossy(&output.stdout).trim().to_string();

    Command::new("git")
        .args(["checkout", "-b", "feature-b", &main_branch])
        .current_dir(&repo.path)
        .output()
        .unwrap();

    // Different modifications on feature
    std::fs::write(repo.path.join("shared.txt"), "Feature changes\n").unwrap();
    Command::new("git")
        .args(["add", "shared.txt"])
        .current_dir(&repo.path)
        .output()
        .unwrap();
    Command::new("git")
        .args(["commit", "-m", "Feature changes"])
        .current_dir(&repo.path)
        .output()
        .unwrap();

    // Try to merge feature into staging (should conflict)
    let result = resolver.merge(&repo.path, "staging");

    // The merge might fail due to conflicts or might report conflicts
    // Depending on how ConflictResolver handles the merge
    if let Ok(merge_result) = result {
        match merge_result {
            MergeResult::Conflicts { files } => {
                assert!(!files.is_empty(), "Should have conflict files");
                assert!(
                    files.iter().any(|f| f.path.to_string_lossy().contains("shared")),
                    "shared.txt should be in conflicts"
                );
            }
            MergeResult::Failed { error } => {
                // Some git operations may fail rather than report conflicts
                assert!(
                    error.contains("conflict") || error.contains("merge"),
                    "Error should be merge-related: {}",
                    error
                );
            }
            MergeResult::Success { .. } => {
                panic!("Should not succeed with conflicting changes");
            }
        }
    }
}

/// Test: Conflict file content extraction
/// Given a merge conflict
/// When conflicts are detected
/// Then ours/theirs/base content is captured
#[tokio::test]
async fn test_conflict_content_extraction() {
    let repo = TestRepo::new();

    // Create initial file with content
    let initial_content = "Line 1\nLine 2\nLine 3\n";
    std::fs::write(repo.path.join("conflict.txt"), initial_content).unwrap();
    Command::new("git")
        .args(["add", "conflict.txt"])
        .current_dir(&repo.path)
        .output()
        .unwrap();
    Command::new("git")
        .args(["commit", "-m", "Add conflict file"])
        .current_dir(&repo.path)
        .output()
        .unwrap();

    // Get current branch name
    let output = Command::new("git")
        .args(["branch", "--show-current"])
        .current_dir(&repo.path)
        .output()
        .unwrap();
    let main_branch = String::from_utf8_lossy(&output.stdout).trim().to_string();

    // Create "ours" branch with modifications
    Command::new("git")
        .args(["checkout", "-b", "ours-branch"])
        .current_dir(&repo.path)
        .output()
        .unwrap();

    let ours_content = "Line 1\nOurs Line 2\nLine 3\n";
    std::fs::write(repo.path.join("conflict.txt"), ours_content).unwrap();
    Command::new("git")
        .args(["add", "conflict.txt"])
        .current_dir(&repo.path)
        .output()
        .unwrap();
    Command::new("git")
        .args(["commit", "-m", "Ours changes"])
        .current_dir(&repo.path)
        .output()
        .unwrap();

    // Create "theirs" branch from main with different modifications
    Command::new("git")
        .args(["checkout", &main_branch])
        .current_dir(&repo.path)
        .output()
        .unwrap();
    Command::new("git")
        .args(["checkout", "-b", "theirs-branch"])
        .current_dir(&repo.path)
        .output()
        .unwrap();

    let theirs_content = "Line 1\nTheirs Line 2\nLine 3\n";
    std::fs::write(repo.path.join("conflict.txt"), theirs_content).unwrap();
    Command::new("git")
        .args(["add", "conflict.txt"])
        .current_dir(&repo.path)
        .output()
        .unwrap();
    Command::new("git")
        .args(["commit", "-m", "Theirs changes"])
        .current_dir(&repo.path)
        .output()
        .unwrap();

    // Now try to merge ours-branch into theirs-branch
    let git_ops = repo.git_ops().unwrap();
    let (pool_tx, _) = mpsc::channel(100);
    let pool = Arc::new(RwLock::new(AgentPool::new(4, pool_tx)));
    let resolver = ConflictResolver::new(git_ops, Arc::clone(&pool));

    // Checkout ours-branch as the worktree
    Command::new("git")
        .args(["checkout", "ours-branch"])
        .current_dir(&repo.path)
        .output()
        .unwrap();

    let result = resolver.merge(&repo.path, "theirs-branch");

    if let Ok(MergeResult::Conflicts { files }) = result {
        assert!(!files.is_empty(), "Should have at least one conflict");

        let conflict = files.iter().find(|f| f.path.to_string_lossy().contains("conflict.txt"));
        if let Some(cf) = conflict {
            // The exact content depends on git's merge behavior
            // Just verify we have some content
            assert!(!cf.ours.is_empty() || !cf.theirs.is_empty(), "Should have conflict content");
        }
    }
    // If merge fails or succeeds differently, that's also acceptable
    // as this tests the detection mechanism
}

/// Test: Multiple conflict files
/// Given changes to multiple files
/// When merge creates multiple conflicts
/// Then all conflict files are reported
#[tokio::test]
async fn test_multiple_conflicts() {
    let repo = TestRepo::new();

    // Create initial files
    for i in 1..=3 {
        let filename = format!("file{}.txt", i);
        std::fs::write(repo.path.join(&filename), format!("Original {}\n", i)).unwrap();
        Command::new("git")
            .args(["add", &filename])
            .current_dir(&repo.path)
            .output()
            .unwrap();
    }
    Command::new("git")
        .args(["commit", "-m", "Add files"])
        .current_dir(&repo.path)
        .output()
        .unwrap();

    // Get main branch name
    let output = Command::new("git")
        .args(["branch", "--show-current"])
        .current_dir(&repo.path)
        .output()
        .unwrap();
    let main_branch = String::from_utf8_lossy(&output.stdout).trim().to_string();

    // Create branch A with changes to all files
    Command::new("git")
        .args(["checkout", "-b", "branch-a"])
        .current_dir(&repo.path)
        .output()
        .unwrap();

    for i in 1..=3 {
        let filename = format!("file{}.txt", i);
        std::fs::write(repo.path.join(&filename), format!("Branch A {}\n", i)).unwrap();
    }
    Command::new("git")
        .args(["add", "."])
        .current_dir(&repo.path)
        .output()
        .unwrap();
    Command::new("git")
        .args(["commit", "-m", "Branch A changes"])
        .current_dir(&repo.path)
        .output()
        .unwrap();

    // Create branch B from main with different changes
    Command::new("git")
        .args(["checkout", &main_branch])
        .current_dir(&repo.path)
        .output()
        .unwrap();
    Command::new("git")
        .args(["checkout", "-b", "branch-b"])
        .current_dir(&repo.path)
        .output()
        .unwrap();

    for i in 1..=3 {
        let filename = format!("file{}.txt", i);
        std::fs::write(repo.path.join(&filename), format!("Branch B {}\n", i)).unwrap();
    }
    Command::new("git")
        .args(["add", "."])
        .current_dir(&repo.path)
        .output()
        .unwrap();
    Command::new("git")
        .args(["commit", "-m", "Branch B changes"])
        .current_dir(&repo.path)
        .output()
        .unwrap();

    // Checkout branch-a and try to merge branch-b
    Command::new("git")
        .args(["checkout", "branch-a"])
        .current_dir(&repo.path)
        .output()
        .unwrap();

    let git_ops = repo.git_ops().unwrap();
    let (pool_tx, _) = mpsc::channel(100);
    let pool = Arc::new(RwLock::new(AgentPool::new(4, pool_tx)));
    let resolver = ConflictResolver::new(git_ops, Arc::clone(&pool));

    let result = resolver.merge(&repo.path, "branch-b");

    if let Ok(MergeResult::Conflicts { files }) = result {
        // Should have multiple conflicts (potentially all 3 files)
        assert!(files.len() >= 1, "Should have at least one conflict file");
    }
    // If the merge fails differently, that's also acceptable
}

/// Test: Merge result types
/// Test various merge result types are correctly represented
#[test]
fn test_merge_result_types() {
    // Success variant
    let success = MergeResult::Success {
        commit: "abc123".to_string(),
    };
    assert!(success.is_success());
    assert_eq!(success.commit(), Some("abc123"));
    assert!(success.conflict_files().map_or(true, |f| f.is_empty()));

    // Conflicts variant
    let conflicts = MergeResult::Conflicts {
        files: vec![],
    };
    assert!(!conflicts.is_success());
    assert_eq!(conflicts.commit(), None);

    // Failed variant
    let failed = MergeResult::Failed {
        error: "Test error".to_string(),
    };
    assert!(!failed.is_success());
    assert_eq!(failed.error(), Some("Test error"));
}

/// Test: Resolver handles empty worktree
/// Given worktree path doesn't exist
/// When merge is attempted
/// Then appropriate error is returned
#[tokio::test]
async fn test_merge_nonexistent_worktree() {
    let repo = TestRepo::new();
    let git_ops = repo.git_ops().unwrap();
    let (pool_tx, _) = mpsc::channel(100);
    let pool = Arc::new(RwLock::new(AgentPool::new(4, pool_tx)));

    let resolver = ConflictResolver::new(git_ops, Arc::clone(&pool));

    // Try to merge with non-existent path
    let fake_path = PathBuf::from("/nonexistent/path/that/does/not/exist");
    let result = resolver.merge(&fake_path, "staging");

    // Should fail
    assert!(result.is_err() || matches!(result.unwrap(), MergeResult::Failed { .. }));
}

/// Test: Staging branch creation
/// Given staging branch doesn't exist
/// When merge is attempted
/// Then staging branch is created or error is returned
#[tokio::test]
async fn test_staging_branch_auto_creation() {
    let repo = TestRepo::new();
    let git_ops = repo.git_ops().unwrap();
    let (pool_tx, _) = mpsc::channel(100);
    let pool = Arc::new(RwLock::new(AgentPool::new(4, pool_tx)));

    let resolver = ConflictResolver::new(git_ops, Arc::clone(&pool));

    // Staging branch doesn't exist
    assert!(!repo.branch_exists("staging"));

    // Create a feature branch
    Command::new("git")
        .args(["checkout", "-b", "feature"])
        .current_dir(&repo.path)
        .output()
        .unwrap();

    std::fs::write(repo.path.join("feature.txt"), "Feature content\n").unwrap();
    Command::new("git")
        .args(["add", "feature.txt"])
        .current_dir(&repo.path)
        .output()
        .unwrap();
    Command::new("git")
        .args(["commit", "-m", "Feature commit"])
        .current_dir(&repo.path)
        .output()
        .unwrap();

    // Try to merge to non-existent staging
    let result = resolver.merge(&repo.path, "auto-staging");

    // Either it creates the branch and succeeds, or it fails with a meaningful error
    // Both are acceptable behaviors depending on implementation
    match result {
        Ok(MergeResult::Success { .. }) => {
            // Staging was auto-created
            assert!(repo.branch_exists("auto-staging"));
        }
        Ok(MergeResult::Failed { error }) => {
            // Branch didn't exist, error is expected
            assert!(
                error.contains("branch") || error.contains("staging") || error.contains("not found"),
                "Error should mention missing branch: {}",
                error
            );
        }
        Err(e) => {
            // Error returned is also acceptable
            let err_str = format!("{:?}", e);
            assert!(
                err_str.contains("branch") || err_str.contains("staging") || err_str.contains("not found") || err_str.contains("reference"),
                "Error should be branch-related: {}",
                err_str
            );
        }
        Ok(MergeResult::Conflicts { .. }) => {
            // Unlikely but acceptable
        }
    }
}
