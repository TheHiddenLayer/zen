//! Git notes management under the `refs/notes/zen/` namespace.
//!
//! This module provides primitives for attaching JSON-serialized data
//! to commits as git notes, which will be used by the GitStateManager.

use std::path::{Path, PathBuf};

use git2::{ErrorCode, Oid, Repository, Signature};
use serde::{de::DeserializeOwned, Serialize};

use crate::{zlog_debug, Result};

/// The namespace prefix for all zen notes refs.
const ZEN_NOTES_PREFIX: &str = "refs/notes/zen/";

/// Manages git notes under the `refs/notes/zen/` namespace.
pub struct GitNotes {
    repo_path: PathBuf,
}

impl GitNotes {
    /// Create a new GitNotes instance for the given repository path.
    ///
    /// # Errors
    /// Returns an error if the path is not a valid git repository.
    pub fn new(repo_path: &Path) -> Result<Self> {
        zlog_debug!("GitNotes::new path={}", repo_path.display());
        let _ = Repository::discover(repo_path)?;
        Ok(Self {
            repo_path: repo_path.to_path_buf(),
        })
    }

    /// Get a fresh Repository handle.
    fn repo(&self) -> Result<Repository> {
        Ok(Repository::discover(&self.repo_path)?)
    }

    /// Build the full notes ref name from the namespace.
    fn notes_ref(namespace: &str) -> String {
        format!("{}{}", ZEN_NOTES_PREFIX, namespace)
    }

    /// Get a default signature for note operations.
    fn signature() -> Result<Signature<'static>> {
        Ok(Signature::now("zen", "zen@localhost")?)
    }

    /// Check if a note exists for the commit in the given namespace.
    pub fn note_exists(&self, commit: &str, namespace: &str) -> Result<bool> {
        let repo = self.repo()?;
        let notes_ref = Self::notes_ref(namespace);
        let oid = Oid::from_str(commit)?;

        let result = match repo.find_note(Some(&notes_ref), oid) {
            Ok(_) => Ok(true),
            Err(e) if e.code() == ErrorCode::NotFound => Ok(false),
            Err(e) => Err(e.into()),
        };
        result
    }

    /// Attach JSON-serialized data as a note to a commit.
    ///
    /// Notes are stored under `refs/notes/zen/{namespace}`.
    /// Overwrites existing note if present.
    pub fn set_note<T: Serialize>(&self, commit: &str, namespace: &str, data: &T) -> Result<()> {
        zlog_debug!("GitNotes::set_note commit={} namespace={}", commit, namespace);
        let repo = self.repo()?;
        let notes_ref = Self::notes_ref(namespace);
        let oid = Oid::from_str(commit)?;
        let sig = Self::signature()?;

        let json = serde_json::to_string(data)?;
        repo.note(&sig, &sig, Some(&notes_ref), oid, &json, true)?;

        zlog_debug!("Set note on {} in {}", commit, notes_ref);
        Ok(())
    }

    /// Read and deserialize a JSON note from a commit.
    ///
    /// Returns `None` if no note exists.
    pub fn get_note<T: DeserializeOwned>(&self, commit: &str, namespace: &str) -> Result<Option<T>> {
        let repo = self.repo()?;
        let notes_ref = Self::notes_ref(namespace);
        let oid = Oid::from_str(commit)?;

        let result = match repo.find_note(Some(&notes_ref), oid) {
            Ok(note) => {
                let message = note.message().unwrap_or("");
                zlog_debug!("Read note from {} in {}: {}", commit, notes_ref, message);
                let data: T = serde_json::from_str(message)?;
                Ok(Some(data))
            }
            Err(e) if e.code() == ErrorCode::NotFound => {
                zlog_debug!("No note found for {} in {}", commit, notes_ref);
                Ok(None)
            }
            Err(e) => Err(e.into()),
        };
        result
    }

    /// Remove a note from a commit under the specified namespace.
    ///
    /// This is idempotent - no error if note doesn't exist.
    pub fn delete_note(&self, commit: &str, namespace: &str) -> Result<()> {
        zlog_debug!("GitNotes::delete_note commit={} namespace={}", commit, namespace);
        let repo = self.repo()?;
        let notes_ref = Self::notes_ref(namespace);
        let oid = Oid::from_str(commit)?;
        let sig = Self::signature()?;

        match repo.note_delete(oid, Some(&notes_ref), &sig, &sig) {
            Ok(_) => {
                zlog_debug!("Deleted note from {} in {}", commit, notes_ref);
                Ok(())
            }
            Err(e) if e.code() == ErrorCode::NotFound => {
                zlog_debug!("No note to delete for {} in {}", commit, notes_ref);
                Ok(())
            }
            Err(e) => Err(e.into()),
        }
    }

    /// List all commit SHAs that have notes in the namespace.
    pub fn list_notes(&self, namespace: &str) -> Result<Vec<String>> {
        let repo = self.repo()?;
        let notes_ref = Self::notes_ref(namespace);
        let mut commits = Vec::new();

        // Try to find the notes ref - if it doesn't exist, return empty list
        match repo.find_reference(&notes_ref) {
            Ok(_) => {
                repo.notes(Some(&notes_ref))?.for_each(|note_result| {
                    if let Ok((_, annotated_oid)) = note_result {
                        commits.push(annotated_oid.to_string());
                    }
                });
            }
            Err(e) if e.code() == ErrorCode::NotFound => {
                // No notes ref yet, return empty list
            }
            Err(e) => return Err(e.into()),
        }

        zlog_debug!("Listed {} notes in {}", commits.len(), notes_ref);
        Ok(commits)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};
    use tempfile::TempDir;

    #[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
    struct TestData {
        name: String,
        count: u32,
    }

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

    /// Create a second commit for testing multiple commits.
    fn create_second_commit(repo_path: &Path) -> String {
        let repo = Repository::open(repo_path).unwrap();
        let sig = Signature::now("Test", "test@example.com").unwrap();
        let tree_id = repo.index().unwrap().write_tree().unwrap();
        let tree = repo.find_tree(tree_id).unwrap();
        let parent = repo.head().unwrap().peel_to_commit().unwrap();
        let commit_id = repo
            .commit(Some("HEAD"), &sig, &sig, "Second commit", &tree, &[&parent])
            .unwrap();
        commit_id.to_string()
    }

    #[test]
    fn test_new_with_valid_repo() {
        let (temp_dir, _) = setup_test_repo();
        let result = GitNotes::new(temp_dir.path());
        assert!(result.is_ok());
    }

    #[test]
    fn test_new_with_invalid_path() {
        let result = GitNotes::new(Path::new("/nonexistent/path"));
        assert!(result.is_err());
    }

    #[test]
    fn test_set_and_get_note() {
        let (temp_dir, commit_sha) = setup_test_repo();
        let notes = GitNotes::new(temp_dir.path()).unwrap();

        let data = TestData {
            name: "test".to_string(),
            count: 42,
        };

        // Set note
        notes.set_note(&commit_sha, "workflow", &data).unwrap();

        // Get it back
        let retrieved: Option<TestData> = notes.get_note(&commit_sha, "workflow").unwrap();
        assert_eq!(retrieved, Some(data));
    }

    #[test]
    fn test_get_note_nonexistent() {
        let (temp_dir, commit_sha) = setup_test_repo();
        let notes = GitNotes::new(temp_dir.path()).unwrap();

        let retrieved: Option<TestData> = notes.get_note(&commit_sha, "workflow").unwrap();
        assert_eq!(retrieved, None);
    }

    #[test]
    fn test_set_note_overwrites() {
        let (temp_dir, commit_sha) = setup_test_repo();
        let notes = GitNotes::new(temp_dir.path()).unwrap();

        let data1 = TestData {
            name: "first".to_string(),
            count: 1,
        };
        let data2 = TestData {
            name: "second".to_string(),
            count: 2,
        };

        // Set first note
        notes.set_note(&commit_sha, "workflow", &data1).unwrap();

        // Overwrite with second note
        notes.set_note(&commit_sha, "workflow", &data2).unwrap();

        // Should get the second one
        let retrieved: Option<TestData> = notes.get_note(&commit_sha, "workflow").unwrap();
        assert_eq!(retrieved, Some(data2));
    }

    #[test]
    fn test_delete_note() {
        let (temp_dir, commit_sha) = setup_test_repo();
        let notes = GitNotes::new(temp_dir.path()).unwrap();

        let data = TestData {
            name: "test".to_string(),
            count: 42,
        };

        // Set note
        notes.set_note(&commit_sha, "workflow", &data).unwrap();
        assert!(notes.note_exists(&commit_sha, "workflow").unwrap());

        // Delete it
        notes.delete_note(&commit_sha, "workflow").unwrap();
        assert!(!notes.note_exists(&commit_sha, "workflow").unwrap());

        // Verify it's gone
        let retrieved: Option<TestData> = notes.get_note(&commit_sha, "workflow").unwrap();
        assert_eq!(retrieved, None);
    }

    #[test]
    fn test_delete_nonexistent_note_succeeds() {
        let (temp_dir, commit_sha) = setup_test_repo();
        let notes = GitNotes::new(temp_dir.path()).unwrap();

        // Should not error
        let result = notes.delete_note(&commit_sha, "workflow");
        assert!(result.is_ok());
    }

    #[test]
    fn test_list_notes_empty() {
        let (temp_dir, _) = setup_test_repo();
        let notes = GitNotes::new(temp_dir.path()).unwrap();

        let list = notes.list_notes("workflow").unwrap();
        assert!(list.is_empty());
    }

    #[test]
    fn test_list_notes() {
        let (temp_dir, commit1) = setup_test_repo();
        let commit2 = create_second_commit(temp_dir.path());
        let notes = GitNotes::new(temp_dir.path()).unwrap();

        let data = TestData {
            name: "test".to_string(),
            count: 42,
        };

        // Add notes to both commits
        notes.set_note(&commit1, "workflow", &data).unwrap();
        notes.set_note(&commit2, "workflow", &data).unwrap();

        let list = notes.list_notes("workflow").unwrap();
        assert_eq!(list.len(), 2);
        assert!(list.contains(&commit1));
        assert!(list.contains(&commit2));
    }

    #[test]
    fn test_note_exists() {
        let (temp_dir, commit_sha) = setup_test_repo();
        let notes = GitNotes::new(temp_dir.path()).unwrap();

        assert!(!notes.note_exists(&commit_sha, "workflow").unwrap());

        let data = TestData {
            name: "test".to_string(),
            count: 42,
        };
        notes.set_note(&commit_sha, "workflow", &data).unwrap();

        assert!(notes.note_exists(&commit_sha, "workflow").unwrap());
    }

    #[test]
    fn test_different_namespaces() {
        let (temp_dir, commit_sha) = setup_test_repo();
        let notes = GitNotes::new(temp_dir.path()).unwrap();

        let data1 = TestData {
            name: "workflow".to_string(),
            count: 1,
        };
        let data2 = TestData {
            name: "task".to_string(),
            count: 2,
        };

        // Set notes in different namespaces
        notes.set_note(&commit_sha, "workflows", &data1).unwrap();
        notes.set_note(&commit_sha, "tasks", &data2).unwrap();

        // Each namespace should have its own note
        let retrieved1: Option<TestData> = notes.get_note(&commit_sha, "workflows").unwrap();
        let retrieved2: Option<TestData> = notes.get_note(&commit_sha, "tasks").unwrap();

        assert_eq!(retrieved1, Some(data1));
        assert_eq!(retrieved2, Some(data2));

        // Lists should be separate
        let workflow_list = notes.list_notes("workflows").unwrap();
        let task_list = notes.list_notes("tasks").unwrap();

        assert_eq!(workflow_list.len(), 1);
        assert_eq!(task_list.len(), 1);
    }
}
