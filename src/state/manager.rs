//! GitStateManager - unified interface for git-native state persistence.

use std::path::{Path, PathBuf};

use crate::git::GitOps;
use crate::git_notes::GitNotes;
use crate::git_refs::GitRefs;
use crate::{zlog_debug, Result};

/// Unified manager for git-native state persistence.
///
/// Composes `GitRefs`, `GitNotes`, and `GitOps` to provide a single
/// interface for all git-based state operations.
pub struct GitStateManager {
    refs: GitRefs,
    notes: GitNotes,
    ops: GitOps,
    repo_path: PathBuf,
}

impl GitStateManager {
    /// Create a new GitStateManager for the given repository path.
    ///
    /// # Errors
    /// Returns an error if the path is not a valid git repository.
    pub fn new(repo_path: &Path) -> Result<Self> {
        zlog_debug!("GitStateManager::new path={}", repo_path.display());

        let refs = GitRefs::new(repo_path)?;
        let notes = GitNotes::new(repo_path)?;
        let ops = GitOps::new(repo_path)?;

        Ok(Self {
            refs,
            notes,
            ops,
            repo_path: repo_path.to_path_buf(),
        })
    }

    /// Access the GitRefs component for ref operations.
    pub fn refs(&self) -> &GitRefs {
        &self.refs
    }

    /// Access the GitNotes component for note operations.
    pub fn notes(&self) -> &GitNotes {
        &self.notes
    }

    /// Access the GitOps component for general git operations.
    pub fn ops(&self) -> &GitOps {
        &self.ops
    }

    /// Get the repository path this manager operates on.
    pub fn repo_path(&self) -> &Path {
        &self.repo_path
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use git2::{Repository, Signature};
    use serde::{Deserialize, Serialize};
    use tempfile::TempDir;

    /// Create a temporary git repository with an initial commit.
    fn setup_test_repo() -> (TempDir, String) {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let repo = Repository::init(temp_dir.path()).expect("Failed to init repo");

        // Create an initial commit
        let sig = Signature::now("Test", "test@example.com").unwrap();
        let tree_id = repo.index().unwrap().write_tree().unwrap();
        let tree = repo.find_tree(tree_id).unwrap();
        let commit_id = repo
            .commit(Some("HEAD"), &sig, &sig, "Initial commit", &tree, &[])
            .unwrap();

        (temp_dir, commit_id.to_string())
    }

    #[test]
    fn test_new_with_valid_repo() {
        let (temp_dir, _) = setup_test_repo();
        let result = GitStateManager::new(temp_dir.path());
        assert!(result.is_ok());
    }

    #[test]
    fn test_new_with_invalid_path() {
        let result = GitStateManager::new(Path::new("/nonexistent/path"));
        assert!(result.is_err());
    }

    #[test]
    fn test_new_with_non_git_dir() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        // Don't initialize git repo
        let result = GitStateManager::new(temp_dir.path());
        assert!(result.is_err());
    }

    #[test]
    fn test_repo_path_accessor() {
        let (temp_dir, _) = setup_test_repo();
        let manager = GitStateManager::new(temp_dir.path()).unwrap();
        assert_eq!(manager.repo_path(), temp_dir.path());
    }

    #[test]
    fn test_refs_accessible() {
        let (temp_dir, commit_sha) = setup_test_repo();
        let manager = GitStateManager::new(temp_dir.path()).unwrap();

        // Use refs to create and read a ref
        manager
            .refs()
            .create_ref("test/myref", &commit_sha)
            .unwrap();
        let target = manager.refs().read_ref("test/myref").unwrap();
        assert_eq!(target, Some(commit_sha));
    }

    #[derive(Serialize, Deserialize, PartialEq, Debug)]
    struct TestData {
        name: String,
        value: u32,
    }

    #[test]
    fn test_notes_accessible() {
        let (temp_dir, commit_sha) = setup_test_repo();
        let manager = GitStateManager::new(temp_dir.path()).unwrap();

        let data = TestData {
            name: "test".to_string(),
            value: 42,
        };

        // Use notes to set and get a note
        manager
            .notes()
            .set_note(&commit_sha, "test", &data)
            .unwrap();
        let retrieved: Option<TestData> = manager.notes().get_note(&commit_sha, "test").unwrap();
        assert_eq!(retrieved, Some(data));
    }

    #[test]
    fn test_ops_accessible() {
        let (temp_dir, _) = setup_test_repo();
        let manager = GitStateManager::new(temp_dir.path()).unwrap();

        // Use ops to get current head
        let head = manager.ops().current_head().unwrap();
        assert!(!head.is_empty());
    }
}
