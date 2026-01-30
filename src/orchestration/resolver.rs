//! Conflict resolution for merging task worktrees.
//!
//! This module handles merging completed task worktrees into a staging branch
//! and resolving any merge conflicts that arise.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use git2::{MergeOptions, Repository, Signature};
use tokio::sync::RwLock;

use crate::git::GitOps;
use crate::Result;

use super::AgentPool;

/// Handles merge conflicts between agent worktrees.
///
/// After the implementation phase completes, each task's worktree needs to be
/// merged into a staging branch. The ConflictResolver manages this process,
/// detecting conflicts and coordinating AI-assisted resolution when needed.
pub struct ConflictResolver {
    /// Git operations for merge commands
    git_ops: GitOps,
    /// Agent pool for spawning resolver agents
    agent_pool: Arc<RwLock<AgentPool>>,
}

impl ConflictResolver {
    /// Create a new ConflictResolver.
    ///
    /// # Arguments
    /// * `git_ops` - GitOps instance for executing git commands
    /// * `agent_pool` - Shared agent pool for spawning resolver agents
    ///
    /// # Example
    /// ```ignore
    /// let git_ops = GitOps::new("/path/to/repo")?;
    /// let agent_pool = Arc::new(RwLock::new(AgentPool::new(4)));
    /// let resolver = ConflictResolver::new(git_ops, agent_pool);
    /// ```
    pub fn new(git_ops: GitOps, agent_pool: Arc<RwLock<AgentPool>>) -> Self {
        Self { git_ops, agent_pool }
    }

    /// Get a reference to the GitOps instance.
    pub fn git_ops(&self) -> &GitOps {
        &self.git_ops
    }

    /// Get a reference to the agent pool.
    pub fn agent_pool(&self) -> &Arc<RwLock<AgentPool>> {
        &self.agent_pool
    }

    /// Attempt to merge a task worktree into the staging branch.
    ///
    /// This method:
    /// 1. Opens the main repository
    /// 2. Checks out the staging branch
    /// 3. Gets the worktree's branch commit
    /// 4. Attempts to merge the worktree's changes
    /// 5. Returns Success with commit hash, Conflicts with file list, or Failed
    ///
    /// # Arguments
    /// * `worktree` - Path to the task worktree directory
    /// * `staging_branch` - Name of the staging branch to merge into
    ///
    /// # Returns
    /// * `MergeResult::Success` - Merge completed cleanly with commit hash
    /// * `MergeResult::Conflicts` - Merge has conflicts that need resolution
    /// * `MergeResult::Failed` - Merge failed due to an error
    pub fn merge(&self, worktree: &Path, staging_branch: &str) -> Result<MergeResult> {
        // Open the main repository
        let repo = Repository::discover(self.git_ops.repo_path())?;

        // Get the worktree's branch name by opening the worktree repo
        let worktree_repo = Repository::open(worktree)?;
        let worktree_head = worktree_repo.head()?;
        let worktree_commit = worktree_head.peel_to_commit()?;

        // Find or create the staging branch
        let staging_branch_ref = match repo.find_branch(staging_branch, git2::BranchType::Local) {
            Ok(branch) => branch,
            Err(e) if e.code() == git2::ErrorCode::NotFound => {
                // Create staging branch from HEAD if it doesn't exist
                let head = repo.head()?;
                let head_commit = head.peel_to_commit()?;
                repo.branch(staging_branch, &head_commit, false)?
            }
            Err(e) => return Ok(MergeResult::failed(format!("Failed to find staging branch: {}", e))),
        };

        // Checkout the staging branch
        let staging_ref = staging_branch_ref.into_reference();
        let staging_commit = staging_ref.peel_to_commit()?;
        repo.checkout_tree(staging_commit.as_object(), None)?;
        repo.set_head(staging_ref.name().unwrap_or(&format!("refs/heads/{}", staging_branch)))?;

        // Get the annotated commit for the worktree's HEAD
        let their_commit = repo.find_commit(worktree_commit.id())?;
        let their_annotated = repo.find_annotated_commit(their_commit.id())?;

        // Perform merge analysis
        let (analysis, _preference) = repo.merge_analysis(&[&their_annotated])?;

        if analysis.is_up_to_date() {
            // Nothing to merge - already up to date
            return Ok(MergeResult::success(staging_commit.id().to_string()));
        }

        if analysis.is_fast_forward() {
            // Fast-forward merge
            let refname = format!("refs/heads/{}", staging_branch);
            repo.reference(
                &refname,
                their_commit.id(),
                true,
                &format!("Fast-forward merge from {}", worktree.display()),
            )?;
            repo.checkout_head(Some(git2::build::CheckoutBuilder::default().force()))?;
            return Ok(MergeResult::success(their_commit.id().to_string()));
        }

        // Normal merge required
        let mut merge_opts = MergeOptions::new();
        repo.merge(&[&their_annotated], Some(&mut merge_opts), None)?;

        // Check for conflicts
        let index = repo.index()?;
        if index.has_conflicts() {
            // Extract conflict information
            let conflicts = self.extract_conflicts(&repo)?;
            // Clean up the merge state
            let _ = repo.cleanup_state();
            return Ok(MergeResult::conflicts(conflicts));
        }

        // No conflicts - create merge commit
        let sig = repo
            .signature()
            .or_else(|_| Signature::now("Zen", "zen@localhost"))?;

        let mut index = repo.index()?;
        let tree_id = index.write_tree()?;
        let tree = repo.find_tree(tree_id)?;

        let message = format!(
            "Merge task worktree from {}",
            worktree.file_name().and_then(|n| n.to_str()).unwrap_or("unknown")
        );

        let commit_id = repo.commit(
            Some("HEAD"),
            &sig,
            &sig,
            &message,
            &tree,
            &[&staging_commit, &their_commit],
        )?;

        // Clean up merge state
        repo.cleanup_state()?;

        Ok(MergeResult::success(commit_id.to_string()))
    }

    /// Extract conflict information from the repository index.
    fn extract_conflicts(&self, repo: &Repository) -> Result<Vec<ConflictFile>> {
        let index = repo.index()?;
        let mut conflicts = Vec::new();

        for conflict in index.conflicts()? {
            let conflict = conflict?;

            // Get the path from whichever entry exists
            let path = conflict
                .our
                .as_ref()
                .or(conflict.their.as_ref())
                .or(conflict.ancestor.as_ref())
                .map(|e| {
                    String::from_utf8_lossy(&e.path).to_string()
                })
                .unwrap_or_default();

            // Extract content from each side
            let ours = self.read_blob_content(repo, conflict.our.as_ref().map(|e| e.id))?;
            let theirs = self.read_blob_content(repo, conflict.their.as_ref().map(|e| e.id))?;
            let base = self.read_blob_content(repo, conflict.ancestor.as_ref().map(|e| e.id))?;

            conflicts.push(ConflictFile::new(
                path,
                ours.unwrap_or_default(),
                theirs.unwrap_or_default(),
                base,
            ));
        }

        Ok(conflicts)
    }

    /// Read blob content from the repository by OID.
    fn read_blob_content(&self, repo: &Repository, oid: Option<git2::Oid>) -> Result<Option<String>> {
        match oid {
            Some(id) if !id.is_zero() => {
                let blob = repo.find_blob(id)?;
                let content = String::from_utf8_lossy(blob.content()).to_string();
                Ok(Some(content))
            }
            _ => Ok(None),
        }
    }
}

/// Result of attempting to merge a worktree into the staging branch.
#[derive(Debug, Clone)]
pub enum MergeResult {
    /// Merge succeeded without conflicts.
    Success {
        /// The commit hash of the merge commit.
        commit: String,
    },
    /// Merge encountered conflicts that need resolution.
    Conflicts {
        /// List of files with conflicts.
        files: Vec<ConflictFile>,
    },
    /// Merge failed due to an error (not conflicts).
    Failed {
        /// Error message describing the failure.
        error: String,
    },
}

impl MergeResult {
    /// Create a successful merge result.
    pub fn success(commit: impl Into<String>) -> Self {
        Self::Success {
            commit: commit.into(),
        }
    }

    /// Create a conflicts result.
    pub fn conflicts(files: Vec<ConflictFile>) -> Self {
        Self::Conflicts { files }
    }

    /// Create a failed result.
    pub fn failed(error: impl Into<String>) -> Self {
        Self::Failed {
            error: error.into(),
        }
    }

    /// Check if the merge was successful.
    pub fn is_success(&self) -> bool {
        matches!(self, Self::Success { .. })
    }

    /// Check if the merge had conflicts.
    pub fn is_conflicts(&self) -> bool {
        matches!(self, Self::Conflicts { .. })
    }

    /// Check if the merge failed.
    pub fn is_failed(&self) -> bool {
        matches!(self, Self::Failed { .. })
    }

    /// Get the commit hash if successful.
    pub fn commit(&self) -> Option<&str> {
        match self {
            Self::Success { commit } => Some(commit),
            _ => None,
        }
    }

    /// Get the conflict files if there were conflicts.
    pub fn conflict_files(&self) -> Option<&[ConflictFile]> {
        match self {
            Self::Conflicts { files } => Some(files),
            _ => None,
        }
    }

    /// Get the error message if failed.
    pub fn error(&self) -> Option<&str> {
        match self {
            Self::Failed { error } => Some(error),
            _ => None,
        }
    }
}

/// A file with merge conflicts.
///
/// Captures the content from both sides of a merge conflict plus the
/// common ancestor (base) if available for three-way merge resolution.
#[derive(Debug, Clone)]
pub struct ConflictFile {
    /// Path to the conflicting file relative to repo root.
    pub path: PathBuf,
    /// Content from "ours" side (the staging branch).
    pub ours: String,
    /// Content from "theirs" side (the task worktree).
    pub theirs: String,
    /// Content from the common ancestor (if available).
    pub base: Option<String>,
}

impl ConflictFile {
    /// Create a new ConflictFile.
    ///
    /// # Arguments
    /// * `path` - Path to the conflicting file
    /// * `ours` - Content from the staging branch ("ours")
    /// * `theirs` - Content from the task worktree ("theirs")
    /// * `base` - Optional content from the common ancestor
    pub fn new(
        path: impl Into<PathBuf>,
        ours: impl Into<String>,
        theirs: impl Into<String>,
        base: Option<String>,
    ) -> Self {
        Self {
            path: path.into(),
            ours: ours.into(),
            theirs: theirs.into(),
            base,
        }
    }

    /// Create a ConflictFile without base content.
    pub fn without_base(
        path: impl Into<PathBuf>,
        ours: impl Into<String>,
        theirs: impl Into<String>,
    ) -> Self {
        Self::new(path, ours, theirs, None)
    }

    /// Check if base content is available for three-way merge.
    pub fn has_base(&self) -> bool {
        self.base.is_some()
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;

    use git2::{IndexAddOption, Repository, Signature};
    use tempfile::TempDir;
    use tokio::sync::mpsc;

    use super::*;
    use crate::orchestration::{AgentEvent, AgentPool};

    fn create_test_resolver() -> ConflictResolver {
        // Create a minimal GitOps for testing (current directory is a git repo)
        let git_ops = GitOps::new(Path::new(".")).unwrap();
        let (event_tx, _event_rx) = mpsc::channel::<AgentEvent>(100);
        let agent_pool = Arc::new(RwLock::new(AgentPool::new(4, event_tx)));
        ConflictResolver::new(git_ops, agent_pool)
    }

    /// Create a test repository with an initial commit.
    fn create_test_repo() -> (TempDir, Repository) {
        let temp_dir = TempDir::new().unwrap();
        let repo = Repository::init(temp_dir.path()).unwrap();

        // Create initial file and commit
        let file_path = temp_dir.path().join("file.txt");
        fs::write(&file_path, "initial content\n").unwrap();

        {
            let mut index = repo.index().unwrap();
            index.add_path(Path::new("file.txt")).unwrap();
            index.write().unwrap();

            let tree_id = index.write_tree().unwrap();
            let tree = repo.find_tree(tree_id).unwrap();
            let sig = Signature::now("Test", "test@test.com").unwrap();

            repo.commit(Some("HEAD"), &sig, &sig, "Initial commit", &tree, &[])
                .unwrap();
        }

        (temp_dir, repo)
    }

    /// Create a ConflictResolver for a test repository.
    fn create_resolver_for_repo(temp_dir: &TempDir) -> ConflictResolver {
        let git_ops = GitOps::new(temp_dir.path()).unwrap();
        let (event_tx, _event_rx) = mpsc::channel::<AgentEvent>(100);
        let agent_pool = Arc::new(RwLock::new(AgentPool::new(4, event_tx)));
        ConflictResolver::new(git_ops, agent_pool)
    }

    /// Create a worktree with a new branch.
    fn create_worktree(repo: &Repository, temp_dir: &TempDir, branch_name: &str) -> PathBuf {
        let head = repo.head().unwrap();
        let commit = head.peel_to_commit().unwrap();

        // Create branch
        let branch = repo.branch(branch_name, &commit, false).unwrap();
        let branch_ref = branch.into_reference();

        // Create worktree
        let worktree_path = temp_dir.path().join(format!("worktree_{}", branch_name));
        let mut opts = git2::WorktreeAddOptions::new();
        opts.reference(Some(&branch_ref));
        repo.worktree(branch_name, &worktree_path, Some(&opts))
            .unwrap();

        worktree_path
    }

    /// Commit changes in a worktree.
    fn commit_in_worktree(worktree_path: &Path, message: &str) {
        let repo = Repository::open(worktree_path).unwrap();
        let mut index = repo.index().unwrap();
        index
            .add_all(["."].iter(), IndexAddOption::DEFAULT, None)
            .unwrap();
        index.write().unwrap();

        let tree_id = index.write_tree().unwrap();
        let tree = repo.find_tree(tree_id).unwrap();
        let sig = Signature::now("Test", "test@test.com").unwrap();

        let parent = repo.head().unwrap().peel_to_commit().unwrap();
        repo.commit(Some("HEAD"), &sig, &sig, message, &tree, &[&parent])
            .unwrap();
    }

    // ========================================
    // ConflictResolver struct tests
    // ========================================

    #[test]
    fn test_conflict_resolver_new() {
        let resolver = create_test_resolver();
        // Verify resolver was created with access to both git_ops and agent_pool
        assert!(resolver.git_ops().repo_path().exists());
    }

    #[tokio::test]
    async fn test_conflict_resolver_agent_pool_access() {
        let resolver = create_test_resolver();
        let pool = resolver.agent_pool().read().await;
        assert_eq!(pool.max_concurrent(), 4);
    }

    // ========================================
    // MergeResult type tests
    // ========================================

    #[test]
    fn test_merge_result_success() {
        let result = MergeResult::success("abc123");
        assert!(result.is_success());
        assert!(!result.is_conflicts());
        assert!(!result.is_failed());
        assert_eq!(result.commit(), Some("abc123"));
        assert!(result.conflict_files().is_none());
        assert!(result.error().is_none());
    }

    #[test]
    fn test_merge_result_conflicts() {
        let files = vec![ConflictFile::new(
            "src/main.rs",
            "ours content",
            "theirs content",
            Some("base content".to_string()),
        )];
        let result = MergeResult::conflicts(files);
        assert!(!result.is_success());
        assert!(result.is_conflicts());
        assert!(!result.is_failed());
        assert!(result.commit().is_none());
        assert_eq!(result.conflict_files().unwrap().len(), 1);
        assert!(result.error().is_none());
    }

    #[test]
    fn test_merge_result_failed() {
        let result = MergeResult::failed("merge error");
        assert!(!result.is_success());
        assert!(!result.is_conflicts());
        assert!(result.is_failed());
        assert!(result.commit().is_none());
        assert!(result.conflict_files().is_none());
        assert_eq!(result.error(), Some("merge error"));
    }

    #[test]
    fn test_merge_result_success_variant() {
        let result = MergeResult::Success {
            commit: "def456".to_string(),
        };
        match result {
            MergeResult::Success { commit } => assert_eq!(commit, "def456"),
            _ => panic!("Expected Success variant"),
        }
    }

    #[test]
    fn test_merge_result_conflicts_variant() {
        let result = MergeResult::Conflicts { files: vec![] };
        match result {
            MergeResult::Conflicts { files } => assert!(files.is_empty()),
            _ => panic!("Expected Conflicts variant"),
        }
    }

    #[test]
    fn test_merge_result_failed_variant() {
        let result = MergeResult::Failed {
            error: "test error".to_string(),
        };
        match result {
            MergeResult::Failed { error } => assert_eq!(error, "test error"),
            _ => panic!("Expected Failed variant"),
        }
    }

    // ========================================
    // ConflictFile tests
    // ========================================

    #[test]
    fn test_conflict_file_new() {
        let conflict = ConflictFile::new(
            "src/lib.rs",
            "ours",
            "theirs",
            Some("base".to_string()),
        );
        assert_eq!(conflict.path, PathBuf::from("src/lib.rs"));
        assert_eq!(conflict.ours, "ours");
        assert_eq!(conflict.theirs, "theirs");
        assert_eq!(conflict.base, Some("base".to_string()));
        assert!(conflict.has_base());
    }

    #[test]
    fn test_conflict_file_without_base() {
        let conflict = ConflictFile::without_base("README.md", "ours", "theirs");
        assert_eq!(conflict.path, PathBuf::from("README.md"));
        assert_eq!(conflict.ours, "ours");
        assert_eq!(conflict.theirs, "theirs");
        assert!(conflict.base.is_none());
        assert!(!conflict.has_base());
    }

    #[test]
    fn test_conflict_file_path_types() {
        // Test with PathBuf
        let conflict1 = ConflictFile::new(
            PathBuf::from("test.rs"),
            "a",
            "b",
            None,
        );
        assert_eq!(conflict1.path, PathBuf::from("test.rs"));

        // Test with &str
        let conflict2 = ConflictFile::new("test.rs", "a", "b", None);
        assert_eq!(conflict2.path, PathBuf::from("test.rs"));
    }

    #[test]
    fn test_conflict_file_content_capture() {
        let ours_content = r#"
fn main() {
    println!("Hello from ours");
}
"#;
        let theirs_content = r#"
fn main() {
    println!("Hello from theirs");
}
"#;
        let base_content = r#"
fn main() {
    println!("Hello");
}
"#;
        let conflict = ConflictFile::new(
            "main.rs",
            ours_content,
            theirs_content,
            Some(base_content.to_string()),
        );

        assert!(conflict.ours.contains("Hello from ours"));
        assert!(conflict.theirs.contains("Hello from theirs"));
        assert!(conflict.base.as_ref().unwrap().contains("Hello"));
    }

    #[test]
    fn test_conflict_file_clone() {
        let conflict = ConflictFile::new("test.rs", "ours", "theirs", Some("base".to_string()));
        let cloned = conflict.clone();
        assert_eq!(conflict.path, cloned.path);
        assert_eq!(conflict.ours, cloned.ours);
        assert_eq!(conflict.theirs, cloned.theirs);
        assert_eq!(conflict.base, cloned.base);
    }

    #[test]
    fn test_merge_result_clone() {
        let result = MergeResult::success("abc123");
        let cloned = result.clone();
        assert!(cloned.is_success());
        assert_eq!(cloned.commit(), Some("abc123"));
    }

    #[test]
    fn test_merge_result_debug() {
        let result = MergeResult::success("abc123");
        let debug_str = format!("{:?}", result);
        assert!(debug_str.contains("Success"));
        assert!(debug_str.contains("abc123"));
    }

    #[test]
    fn test_conflict_file_debug() {
        let conflict = ConflictFile::new("test.rs", "ours", "theirs", None);
        let debug_str = format!("{:?}", conflict);
        assert!(debug_str.contains("ConflictFile"));
        assert!(debug_str.contains("test.rs"));
    }

    // ========================================
    // merge() method tests
    // ========================================

    #[test]
    fn test_merge_clean_merge() {
        // Create repo with initial commit
        let (temp_dir, repo) = create_test_repo();
        let resolver = create_resolver_for_repo(&temp_dir);

        // Create staging branch from HEAD
        let head = repo.head().unwrap();
        let head_commit = head.peel_to_commit().unwrap();
        repo.branch("staging", &head_commit, false).unwrap();

        // Create worktree with a new file (non-conflicting change)
        let worktree_path = create_worktree(&repo, &temp_dir, "feature");

        // Add a new file in the worktree
        let new_file = worktree_path.join("new_file.txt");
        fs::write(&new_file, "new file content\n").unwrap();
        commit_in_worktree(&worktree_path, "Add new file");

        // Merge should succeed
        let result = resolver.merge(&worktree_path, "staging").unwrap();
        assert!(result.is_success(), "Expected Success, got {:?}", result);
        assert!(result.commit().is_some());
    }

    #[test]
    fn test_merge_fast_forward() {
        // Create repo with initial commit
        let (temp_dir, repo) = create_test_repo();
        let resolver = create_resolver_for_repo(&temp_dir);

        // Create staging branch from HEAD
        let head = repo.head().unwrap();
        let head_commit = head.peel_to_commit().unwrap();
        repo.branch("staging", &head_commit, false).unwrap();

        // Create worktree and make a change
        let worktree_path = create_worktree(&repo, &temp_dir, "feature");
        let new_file = worktree_path.join("new_file.txt");
        fs::write(&new_file, "new content\n").unwrap();
        commit_in_worktree(&worktree_path, "Add new file");

        // Since staging hasn't changed, this should fast-forward
        let result = resolver.merge(&worktree_path, "staging").unwrap();
        assert!(result.is_success(), "Expected Success, got {:?}", result);
    }

    #[test]
    fn test_merge_conflict_detection() {
        // Create repo with initial commit
        let (temp_dir, repo) = create_test_repo();
        let resolver = create_resolver_for_repo(&temp_dir);

        // Create staging branch and modify file.txt
        let head = repo.head().unwrap();
        let head_commit = head.peel_to_commit().unwrap();
        repo.branch("staging", &head_commit, false).unwrap();

        // First, create worktree for feature
        let worktree_path = create_worktree(&repo, &temp_dir, "feature");

        // Modify file.txt in main repo (which staging points to)
        let main_file = temp_dir.path().join("file.txt");
        fs::write(&main_file, "staging content - modified\n").unwrap();

        // Commit in main repo
        {
            let mut index = repo.index().unwrap();
            index.add_path(Path::new("file.txt")).unwrap();
            index.write().unwrap();
            let tree_id = index.write_tree().unwrap();
            let tree = repo.find_tree(tree_id).unwrap();
            let sig = Signature::now("Test", "test@test.com").unwrap();
            let parent = repo.head().unwrap().peel_to_commit().unwrap();
            repo.commit(Some("HEAD"), &sig, &sig, "Modify in main", &tree, &[&parent])
                .unwrap();
        }

        // Update staging branch to point to new commit
        {
            let head = repo.head().unwrap();
            let head_commit = head.peel_to_commit().unwrap();
            let mut staging = repo.find_branch("staging", git2::BranchType::Local).unwrap();
            staging.get_mut().set_target(head_commit.id(), "Update staging").unwrap();
        }

        // Modify same file in worktree with different content
        let worktree_file = worktree_path.join("file.txt");
        fs::write(&worktree_file, "feature content - different\n").unwrap();
        commit_in_worktree(&worktree_path, "Modify in feature");

        // Merge should detect conflict
        let result = resolver.merge(&worktree_path, "staging").unwrap();
        assert!(result.is_conflicts(), "Expected Conflicts, got {:?}", result);

        let files = result.conflict_files().unwrap();
        assert!(!files.is_empty(), "Should have at least one conflict file");
        assert_eq!(files[0].path.to_str().unwrap(), "file.txt");
    }

    #[test]
    fn test_merge_conflict_content_extraction() {
        // Create repo with initial commit
        let (temp_dir, repo) = create_test_repo();
        let resolver = create_resolver_for_repo(&temp_dir);

        // Create staging branch
        let head = repo.head().unwrap();
        let head_commit = head.peel_to_commit().unwrap();
        repo.branch("staging", &head_commit, false).unwrap();

        // Create worktree
        let worktree_path = create_worktree(&repo, &temp_dir, "feature");

        // Modify file in main repo
        let main_file = temp_dir.path().join("file.txt");
        fs::write(&main_file, "ours content\n").unwrap();
        {
            let mut index = repo.index().unwrap();
            index.add_path(Path::new("file.txt")).unwrap();
            index.write().unwrap();
            let tree_id = index.write_tree().unwrap();
            let tree = repo.find_tree(tree_id).unwrap();
            let sig = Signature::now("Test", "test@test.com").unwrap();
            let parent = repo.head().unwrap().peel_to_commit().unwrap();
            repo.commit(Some("HEAD"), &sig, &sig, "Ours change", &tree, &[&parent])
                .unwrap();
        }

        // Update staging to point to new commit
        {
            let head = repo.head().unwrap();
            let head_commit = head.peel_to_commit().unwrap();
            let mut staging = repo.find_branch("staging", git2::BranchType::Local).unwrap();
            staging.get_mut().set_target(head_commit.id(), "Update staging").unwrap();
        }

        // Modify in worktree with different content
        let worktree_file = worktree_path.join("file.txt");
        fs::write(&worktree_file, "theirs content\n").unwrap();
        commit_in_worktree(&worktree_path, "Theirs change");

        // Merge should detect conflict
        let result = resolver.merge(&worktree_path, "staging").unwrap();
        assert!(result.is_conflicts());

        let files = result.conflict_files().unwrap();
        assert_eq!(files.len(), 1);

        let conflict = &files[0];
        assert!(conflict.ours.contains("ours content"));
        assert!(conflict.theirs.contains("theirs content"));
        // Base should contain original content
        assert!(conflict.has_base());
        assert!(conflict.base.as_ref().unwrap().contains("initial content"));
    }

    #[test]
    fn test_merge_failure_invalid_worktree() {
        let (temp_dir, _repo) = create_test_repo();
        let resolver = create_resolver_for_repo(&temp_dir);

        // Try to merge from non-existent worktree
        let invalid_path = temp_dir.path().join("non_existent_worktree");
        let result = resolver.merge(&invalid_path, "staging");

        // Should return an error (not MergeResult::Failed, but actual error)
        assert!(result.is_err());
    }

    #[test]
    fn test_merge_multiple_conflicts() {
        // Create repo with 3 initial files
        let temp_dir = TempDir::new().unwrap();
        let repo = Repository::init(temp_dir.path()).unwrap();

        // Create 3 files
        for i in 1..=3 {
            let file_path = temp_dir.path().join(format!("file{}.txt", i));
            fs::write(&file_path, format!("initial content {}\n", i)).unwrap();
        }

        // Initial commit
        {
            let mut index = repo.index().unwrap();
            for i in 1..=3 {
                index.add_path(Path::new(&format!("file{}.txt", i))).unwrap();
            }
            index.write().unwrap();
            let tree_id = index.write_tree().unwrap();
            let tree = repo.find_tree(tree_id).unwrap();
            let sig = Signature::now("Test", "test@test.com").unwrap();
            repo.commit(Some("HEAD"), &sig, &sig, "Initial commit", &tree, &[])
                .unwrap();
        }

        let resolver = create_resolver_for_repo(&temp_dir);

        // Create staging branch
        let head = repo.head().unwrap();
        let head_commit = head.peel_to_commit().unwrap();
        repo.branch("staging", &head_commit, false).unwrap();

        // Create worktree
        let worktree_path = create_worktree(&repo, &temp_dir, "feature");

        // Modify all 3 files in main repo
        for i in 1..=3 {
            let file_path = temp_dir.path().join(format!("file{}.txt", i));
            fs::write(&file_path, format!("staging content {}\n", i)).unwrap();
        }
        {
            let mut index = repo.index().unwrap();
            for i in 1..=3 {
                index.add_path(Path::new(&format!("file{}.txt", i))).unwrap();
            }
            index.write().unwrap();
            let tree_id = index.write_tree().unwrap();
            let tree = repo.find_tree(tree_id).unwrap();
            let sig = Signature::now("Test", "test@test.com").unwrap();
            let parent = repo.head().unwrap().peel_to_commit().unwrap();
            repo.commit(Some("HEAD"), &sig, &sig, "Staging changes", &tree, &[&parent])
                .unwrap();
        }

        // Update staging branch
        {
            let head = repo.head().unwrap();
            let head_commit = head.peel_to_commit().unwrap();
            let mut staging = repo.find_branch("staging", git2::BranchType::Local).unwrap();
            staging.get_mut().set_target(head_commit.id(), "Update staging").unwrap();
        }

        // Modify all 3 files in worktree with different content
        for i in 1..=3 {
            let file_path = worktree_path.join(format!("file{}.txt", i));
            fs::write(&file_path, format!("feature content {}\n", i)).unwrap();
        }
        commit_in_worktree(&worktree_path, "Feature changes");

        // Merge should detect all 3 conflicts
        let result = resolver.merge(&worktree_path, "staging").unwrap();
        assert!(result.is_conflicts(), "Expected Conflicts, got {:?}", result);

        let files = result.conflict_files().unwrap();
        assert_eq!(files.len(), 3, "Expected 3 conflict files, got {}", files.len());

        // Verify all files are present
        let paths: Vec<_> = files.iter().map(|f| f.path.to_str().unwrap()).collect();
        assert!(paths.contains(&"file1.txt"));
        assert!(paths.contains(&"file2.txt"));
        assert!(paths.contains(&"file3.txt"));
    }

    #[test]
    fn test_merge_creates_staging_branch_if_missing() {
        let (temp_dir, repo) = create_test_repo();
        let resolver = create_resolver_for_repo(&temp_dir);

        // Create worktree with a change
        let worktree_path = create_worktree(&repo, &temp_dir, "feature");
        let new_file = worktree_path.join("new_file.txt");
        fs::write(&new_file, "new content\n").unwrap();
        commit_in_worktree(&worktree_path, "Add new file");

        // Merge should create staging branch if it doesn't exist
        let result = resolver.merge(&worktree_path, "auto_staging").unwrap();
        assert!(result.is_success(), "Expected Success, got {:?}", result);

        // Verify staging branch was created
        let branch = repo.find_branch("auto_staging", git2::BranchType::Local);
        assert!(branch.is_ok(), "Staging branch should have been created");
    }

    #[test]
    fn test_merge_up_to_date() {
        let (temp_dir, repo) = create_test_repo();
        let resolver = create_resolver_for_repo(&temp_dir);

        // Create worktree with no changes
        let worktree_path = create_worktree(&repo, &temp_dir, "feature");

        // Create staging branch at the same commit
        let head = repo.head().unwrap();
        let head_commit = head.peel_to_commit().unwrap();
        repo.branch("staging", &head_commit, false).unwrap();

        // Merge should report up-to-date (as success)
        let result = resolver.merge(&worktree_path, "staging").unwrap();
        assert!(result.is_success(), "Expected Success for up-to-date, got {:?}", result);
    }
}
