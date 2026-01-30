use std::path::{Path, PathBuf};

use git2::{Oid, Repository, Signature};
use serde::{de::DeserializeOwned, Serialize};

use crate::Result;

/// The notes reference used for zen state storage.
const NOTES_REF: &str = "refs/notes/zen";

/// Git notes operations for storing JSON data attached to commits.
///
/// This module provides a way to store arbitrary JSON data as git notes
/// under the `refs/notes/zen` namespace, keeping state in git itself
/// rather than external JSON files.
pub struct GitNotes {
    repo_path: PathBuf,
}

impl GitNotes {
    /// Create a new GitNotes instance for the given repository path.
    pub fn new(repo_path: &Path) -> Result<Self> {
        let _ = Repository::discover(repo_path)?;
        Ok(Self {
            repo_path: repo_path.to_path_buf(),
        })
    }

    fn repo(&self) -> Result<Repository> {
        Ok(Repository::discover(&self.repo_path)?)
    }

    fn signature(&self) -> Result<Signature<'static>> {
        let repo = self.repo()?;
        Ok(repo
            .signature()
            .or_else(|_| Signature::now("Zen", "zen@localhost"))?)
    }

    /// Add a JSON note to a commit under refs/notes/zen/.
    ///
    /// If a note already exists for this commit, it will be overwritten.
    pub fn add<T: Serialize>(&self, commit_oid: &str, data: &T) -> Result<()> {
        let repo = self.repo()?;
        let oid = Oid::from_str(commit_oid)?;
        let json = serde_json::to_string_pretty(data)?;
        let sig = self.signature()?;

        repo.note(&sig, &sig, Some(NOTES_REF), oid, &json, true)?;
        Ok(())
    }

    /// Read a JSON note from a commit.
    ///
    /// Returns `Ok(None)` if no note exists for this commit.
    pub fn read<T: DeserializeOwned>(&self, commit_oid: &str) -> Result<Option<T>> {
        let repo = self.repo()?;
        let oid = Oid::from_str(commit_oid)?;

        let message = match repo.find_note(Some(NOTES_REF), oid) {
            Ok(note) => note.message().unwrap_or("").to_string(),
            Err(e) if e.code() == git2::ErrorCode::NotFound => return Ok(None),
            Err(e) => return Err(e.into()),
        };

        let data: T = serde_json::from_str(&message)?;
        Ok(Some(data))
    }

    /// Remove a note from a commit.
    ///
    /// Succeeds without error if no note exists for this commit.
    pub fn remove(&self, commit_oid: &str) -> Result<()> {
        let repo = self.repo()?;
        let oid = Oid::from_str(commit_oid)?;
        let sig = self.signature()?;

        match repo.note_delete(oid, Some(NOTES_REF), &sig, &sig) {
            Ok(_) => Ok(()),
            Err(e) if e.code() == git2::ErrorCode::NotFound => Ok(()),
            Err(e) => Err(e.into()),
        }
    }

    /// List all commit OIDs that have notes in refs/notes/zen/.
    pub fn list(&self) -> Result<Vec<String>> {
        let repo = self.repo()?;
        let mut oids = Vec::new();

        // notes_foreach requires a callback and iterates through all notes
        match repo.notes(Some(NOTES_REF)) {
            Ok(notes) => {
                for note_result in notes {
                    if let Ok((_, annotated_oid)) = note_result {
                        oids.push(annotated_oid.to_string());
                    }
                }
            }
            Err(e) if e.code() == git2::ErrorCode::NotFound => {
                // No notes ref exists yet, return empty list
            }
            Err(e) => return Err(e.into()),
        }

        Ok(oids)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};
    use std::fs;

    /// Helper to create a temporary git repository with an initial commit.
    fn setup_test_repo() -> (tempfile::TempDir, String) {
        let temp_dir = tempfile::tempdir().unwrap();
        let repo = Repository::init(temp_dir.path()).unwrap();

        // Create an initial commit
        let sig = Signature::now("Test", "test@test.com").unwrap();
        let tree_id = repo.index().unwrap().write_tree().unwrap();
        let tree = repo.find_tree(tree_id).unwrap();
        let commit_oid = repo
            .commit(Some("HEAD"), &sig, &sig, "Initial commit", &tree, &[])
            .unwrap();

        (temp_dir, commit_oid.to_string())
    }

    /// Helper to create additional commits for testing.
    fn create_commit(repo_path: &Path, message: &str) -> String {
        let repo = Repository::open(repo_path).unwrap();
        let sig = Signature::now("Test", "test@test.com").unwrap();

        // Create a file to have something to commit
        let file_path = repo_path.join(format!("{}.txt", message.replace(' ', "_")));
        fs::write(&file_path, message).unwrap();

        let mut index = repo.index().unwrap();
        index
            .add_path(Path::new(file_path.file_name().unwrap()))
            .unwrap();
        index.write().unwrap();
        let tree_id = index.write_tree().unwrap();
        let tree = repo.find_tree(tree_id).unwrap();

        let head = repo.head().unwrap().peel_to_commit().unwrap();
        let commit_oid = repo
            .commit(Some("HEAD"), &sig, &sig, message, &tree, &[&head])
            .unwrap();

        commit_oid.to_string()
    }

    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    struct TestData {
        name: String,
        value: i32,
    }

    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    struct ComplexData {
        id: String,
        items: Vec<String>,
        nested: NestedData,
    }

    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    struct NestedData {
        flag: bool,
        count: u64,
    }

    // T1: Add a JSON note to a commit - verify it can be read back
    #[test]
    fn test_add_and_read_note() {
        let (temp_dir, commit_oid) = setup_test_repo();
        let notes = GitNotes::new(temp_dir.path()).unwrap();

        let data = TestData {
            name: "test".to_string(),
            value: 42,
        };

        notes.add(&commit_oid, &data).unwrap();
        let read_data: Option<TestData> = notes.read(&commit_oid).unwrap();

        assert_eq!(read_data, Some(data));
    }

    // T2: Read a note from a commit that has no note - returns None
    #[test]
    fn test_read_nonexistent_note() {
        let (temp_dir, commit_oid) = setup_test_repo();
        let notes = GitNotes::new(temp_dir.path()).unwrap();

        let read_data: Option<TestData> = notes.read(&commit_oid).unwrap();

        assert_eq!(read_data, None);
    }

    // T3: Remove a note from a commit - verify it's gone
    #[test]
    fn test_remove_note() {
        let (temp_dir, commit_oid) = setup_test_repo();
        let notes = GitNotes::new(temp_dir.path()).unwrap();

        let data = TestData {
            name: "to_delete".to_string(),
            value: 99,
        };
        notes.add(&commit_oid, &data).unwrap();

        // Verify note exists
        let read_data: Option<TestData> = notes.read(&commit_oid).unwrap();
        assert!(read_data.is_some());

        // Remove and verify gone
        notes.remove(&commit_oid).unwrap();
        let read_data: Option<TestData> = notes.read(&commit_oid).unwrap();
        assert_eq!(read_data, None);
    }

    // T4: Remove a note from a commit that has no note - succeeds without error
    #[test]
    fn test_remove_nonexistent_note() {
        let (temp_dir, commit_oid) = setup_test_repo();
        let notes = GitNotes::new(temp_dir.path()).unwrap();

        // Should not error
        let result = notes.remove(&commit_oid);
        assert!(result.is_ok());
    }

    // T5: Add a complex struct as JSON note - verify round-trip serialization
    #[test]
    fn test_complex_struct_roundtrip() {
        let (temp_dir, commit_oid) = setup_test_repo();
        let notes = GitNotes::new(temp_dir.path()).unwrap();

        let data = ComplexData {
            id: "abc-123".to_string(),
            items: vec!["one".to_string(), "two".to_string(), "three".to_string()],
            nested: NestedData {
                flag: true,
                count: 9999,
            },
        };

        notes.add(&commit_oid, &data).unwrap();
        let read_data: Option<ComplexData> = notes.read(&commit_oid).unwrap();

        assert_eq!(read_data, Some(data));
    }

    // T6: Overwrite an existing note
    #[test]
    fn test_overwrite_note() {
        let (temp_dir, commit_oid) = setup_test_repo();
        let notes = GitNotes::new(temp_dir.path()).unwrap();

        let data1 = TestData {
            name: "first".to_string(),
            value: 1,
        };
        let data2 = TestData {
            name: "second".to_string(),
            value: 2,
        };

        notes.add(&commit_oid, &data1).unwrap();
        notes.add(&commit_oid, &data2).unwrap();

        let read_data: Option<TestData> = notes.read(&commit_oid).unwrap();
        assert_eq!(read_data, Some(data2));
    }

    // T7: List notes when none exist - returns empty vec
    #[test]
    fn test_list_empty() {
        let (temp_dir, _commit_oid) = setup_test_repo();
        let notes = GitNotes::new(temp_dir.path()).unwrap();

        let list = notes.list().unwrap();
        assert!(list.is_empty());
    }

    // T8: List notes after adding several - returns all commit OIDs
    #[test]
    fn test_list_multiple_notes() {
        let (temp_dir, commit_oid1) = setup_test_repo();
        let commit_oid2 = create_commit(temp_dir.path(), "second commit");
        let commit_oid3 = create_commit(temp_dir.path(), "third commit");

        let notes = GitNotes::new(temp_dir.path()).unwrap();

        // Add notes to commits 1 and 3 (not 2)
        notes
            .add(
                &commit_oid1,
                &TestData {
                    name: "one".to_string(),
                    value: 1,
                },
            )
            .unwrap();
        notes
            .add(
                &commit_oid3,
                &TestData {
                    name: "three".to_string(),
                    value: 3,
                },
            )
            .unwrap();

        let list = notes.list().unwrap();
        assert_eq!(list.len(), 2);
        assert!(list.contains(&commit_oid1));
        assert!(list.contains(&commit_oid3));
        assert!(!list.contains(&commit_oid2));
    }

    // T9: Operations with invalid commit OID - returns appropriate error
    #[test]
    fn test_invalid_oid() {
        let (temp_dir, _commit_oid) = setup_test_repo();
        let notes = GitNotes::new(temp_dir.path()).unwrap();

        let result = notes.add(
            "not-a-valid-oid",
            &TestData {
                name: "test".to_string(),
                value: 0,
            },
        );
        assert!(result.is_err());

        let result: Result<Option<TestData>> = notes.read("not-a-valid-oid");
        assert!(result.is_err());
    }

    // T10: Note namespace isolation - notes under refs/notes/zen/ don't affect refs/notes/commits
    #[test]
    fn test_namespace_isolation() {
        let (temp_dir, commit_oid) = setup_test_repo();
        let repo = Repository::open(temp_dir.path()).unwrap();
        let notes = GitNotes::new(temp_dir.path()).unwrap();

        // Add a note via GitNotes (uses refs/notes/zen)
        let zen_data = TestData {
            name: "zen".to_string(),
            value: 100,
        };
        notes.add(&commit_oid, &zen_data).unwrap();

        // Add a note directly to default namespace (refs/notes/commits)
        let sig = Signature::now("Test", "test@test.com").unwrap();
        let oid = Oid::from_str(&commit_oid).unwrap();
        repo.note(&sig, &sig, None, oid, "default note content", false)
            .unwrap();

        // Verify zen note is independent
        let read_zen: Option<TestData> = notes.read(&commit_oid).unwrap();
        assert_eq!(read_zen, Some(zen_data));

        // Verify default note exists separately
        let default_note = repo.find_note(None, oid).unwrap();
        assert_eq!(default_note.message(), Some("default note content"));
    }
}
