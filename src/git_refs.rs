//! Git refs management under the `refs/zen/` namespace.
//!
//! This module provides low-level primitives for creating, reading, updating,
//! and deleting git refs that will be used by the GitStateManager.

use std::path::{Path, PathBuf};

use git2::{ErrorCode, Oid, Repository};

use crate::{zlog_debug, Error, Result};

/// The namespace prefix for all zen refs.
const ZEN_REFS_PREFIX: &str = "refs/zen/";

/// Manages git refs under the `refs/zen/` namespace.
pub struct GitRefs {
    repo_path: PathBuf,
}

impl GitRefs {
    /// Create a new GitRefs instance for the given repository path.
    ///
    /// # Errors
    /// Returns an error if the path is not a valid git repository.
    pub fn new(repo_path: &Path) -> Result<Self> {
        zlog_debug!("GitRefs::new path={}", repo_path.display());
        let _ = Repository::discover(repo_path)?;
        Ok(Self {
            repo_path: repo_path.to_path_buf(),
        })
    }

    /// Get a fresh Repository handle.
    fn repo(&self) -> Result<Repository> {
        Ok(Repository::discover(&self.repo_path)?)
    }

    /// Build the full ref name from the short name.
    fn full_ref_name(name: &str) -> String {
        format!("{}{}", ZEN_REFS_PREFIX, name)
    }

    /// Check if a ref exists under `refs/zen/{name}`.
    pub fn ref_exists(&self, name: &str) -> Result<bool> {
        let repo = self.repo()?;
        let refname = Self::full_ref_name(name);
        let result = match repo.find_reference(&refname) {
            Ok(_) => Ok(true),
            Err(e) if e.code() == ErrorCode::NotFound => Ok(false),
            Err(e) => Err(e.into()),
        };
        result
    }

    /// Create a ref at `refs/zen/{name}` pointing to the given target commit SHA.
    ///
    /// # Errors
    /// Returns `Error::RefExists` if the ref already exists.
    /// Returns `Error::Git` if the target is not a valid commit SHA.
    pub fn create_ref(&self, name: &str, target: &str) -> Result<()> {
        zlog_debug!("GitRefs::create_ref name={} target={}", name, target);
        let repo = self.repo()?;
        let refname = Self::full_ref_name(name);

        // Check if ref already exists
        if self.ref_exists(name)? {
            return Err(Error::RefExists(refname));
        }

        // Parse the target as an OID
        let oid = Oid::from_str(target)?;

        // Create the reference
        repo.reference(&refname, oid, false, "zen: create ref")?;
        zlog_debug!("Created ref {} -> {}", refname, target);
        Ok(())
    }

    /// Read the target (commit SHA) of `refs/zen/{name}`.
    ///
    /// Returns `None` if the ref doesn't exist.
    pub fn read_ref(&self, name: &str) -> Result<Option<String>> {
        let repo = self.repo()?;
        let refname = Self::full_ref_name(name);

        let result = match repo.find_reference(&refname) {
            Ok(reference) => {
                let target = reference.target().map(|oid| oid.to_string());
                zlog_debug!("Read ref {} -> {:?}", refname, target);
                Ok(target)
            }
            Err(e) if e.code() == ErrorCode::NotFound => {
                zlog_debug!("Ref {} not found", refname);
                Ok(None)
            }
            Err(e) => Err(e.into()),
        };
        result
    }

    /// Update an existing ref to point to a new target.
    ///
    /// # Errors
    /// Returns `Error::RefNotFound` if the ref doesn't exist.
    pub fn update_ref(&self, name: &str, target: &str) -> Result<()> {
        zlog_debug!("GitRefs::update_ref name={} target={}", name, target);
        let repo = self.repo()?;
        let refname = Self::full_ref_name(name);

        // Check if ref exists
        if !self.ref_exists(name)? {
            return Err(Error::RefNotFound(refname));
        }

        // Parse the target as an OID
        let oid = Oid::from_str(target)?;

        // Update the reference (force=true to overwrite)
        repo.reference(&refname, oid, true, "zen: update ref")?;
        zlog_debug!("Updated ref {} -> {}", refname, target);
        Ok(())
    }

    /// Delete `refs/zen/{name}`.
    ///
    /// This is idempotent - no error if the ref doesn't exist.
    pub fn delete_ref(&self, name: &str) -> Result<()> {
        zlog_debug!("GitRefs::delete_ref name={}", name);
        let repo = self.repo()?;
        let refname = Self::full_ref_name(name);

        match repo.find_reference(&refname) {
            Ok(mut reference) => {
                reference.delete()?;
                zlog_debug!("Deleted ref {}", refname);
            }
            Err(e) if e.code() == ErrorCode::NotFound => {
                zlog_debug!("Ref {} not found (already deleted?)", refname);
            }
            Err(e) => return Err(e.into()),
        }
        Ok(())
    }

    /// List all refs under `refs/zen/`.
    ///
    /// If `prefix` is provided, only refs matching `refs/zen/{prefix}*` are returned.
    /// Returns ref names without the `refs/zen/` prefix.
    pub fn list_refs(&self, prefix: Option<&str>) -> Result<Vec<String>> {
        let repo = self.repo()?;
        let mut refs = Vec::new();

        let search_prefix = match prefix {
            Some(p) => format!("{}{}", ZEN_REFS_PREFIX, p),
            None => ZEN_REFS_PREFIX.to_string(),
        };

        repo.references_glob(&format!("{}*", search_prefix))?
            .filter_map(|r| r.ok())
            .filter_map(|r| r.name().map(String::from))
            .for_each(|name| {
                // Strip the refs/zen/ prefix
                if let Some(short_name) = name.strip_prefix(ZEN_REFS_PREFIX) {
                    refs.push(short_name.to_string());
                }
            });

        zlog_debug!("Listed {} refs with prefix {:?}", refs.len(), prefix);
        Ok(refs)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use git2::Signature;
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
        let result = GitRefs::new(temp_dir.path());
        assert!(result.is_ok());
    }

    #[test]
    fn test_new_with_invalid_path() {
        let result = GitRefs::new(Path::new("/nonexistent/path"));
        assert!(result.is_err());
    }

    #[test]
    fn test_create_and_read_ref() {
        let (temp_dir, commit_sha) = setup_test_repo();
        let refs = GitRefs::new(temp_dir.path()).unwrap();

        // Create a ref
        refs.create_ref("test/myref", &commit_sha).unwrap();

        // Read it back
        let target = refs.read_ref("test/myref").unwrap();
        assert_eq!(target, Some(commit_sha));
    }

    #[test]
    fn test_read_nonexistent_ref() {
        let (temp_dir, _) = setup_test_repo();
        let refs = GitRefs::new(temp_dir.path()).unwrap();

        let target = refs.read_ref("nonexistent").unwrap();
        assert_eq!(target, None);
    }

    #[test]
    fn test_create_duplicate_ref_fails() {
        let (temp_dir, commit_sha) = setup_test_repo();
        let refs = GitRefs::new(temp_dir.path()).unwrap();

        refs.create_ref("duplicate", &commit_sha).unwrap();
        let result = refs.create_ref("duplicate", &commit_sha);

        assert!(matches!(result, Err(Error::RefExists(_))));
    }

    #[test]
    fn test_update_ref() {
        let (temp_dir, commit_sha) = setup_test_repo();
        let refs = GitRefs::new(temp_dir.path()).unwrap();

        // Create initial ref
        refs.create_ref("updateme", &commit_sha).unwrap();

        // Create a second commit for a new target
        let repo = Repository::open(temp_dir.path()).unwrap();
        let sig = Signature::now("Test", "test@example.com").unwrap();
        let tree_id = repo.index().unwrap().write_tree().unwrap();
        let tree = repo.find_tree(tree_id).unwrap();
        let parent = repo.head().unwrap().peel_to_commit().unwrap();
        let new_commit_id = repo
            .commit(Some("HEAD"), &sig, &sig, "Second commit", &tree, &[&parent])
            .unwrap();
        let new_sha = new_commit_id.to_string();

        // Update the ref
        refs.update_ref("updateme", &new_sha).unwrap();

        // Verify update
        let target = refs.read_ref("updateme").unwrap();
        assert_eq!(target, Some(new_sha));
    }

    #[test]
    fn test_update_nonexistent_ref_fails() {
        let (temp_dir, commit_sha) = setup_test_repo();
        let refs = GitRefs::new(temp_dir.path()).unwrap();

        let result = refs.update_ref("nonexistent", &commit_sha);
        assert!(matches!(result, Err(Error::RefNotFound(_))));
    }

    #[test]
    fn test_delete_ref() {
        let (temp_dir, commit_sha) = setup_test_repo();
        let refs = GitRefs::new(temp_dir.path()).unwrap();

        refs.create_ref("deleteme", &commit_sha).unwrap();
        assert!(refs.ref_exists("deleteme").unwrap());

        refs.delete_ref("deleteme").unwrap();
        assert!(!refs.ref_exists("deleteme").unwrap());
    }

    #[test]
    fn test_delete_nonexistent_ref_succeeds() {
        let (temp_dir, _) = setup_test_repo();
        let refs = GitRefs::new(temp_dir.path()).unwrap();

        // Should not error
        let result = refs.delete_ref("nonexistent");
        assert!(result.is_ok());
    }

    #[test]
    fn test_ref_exists() {
        let (temp_dir, commit_sha) = setup_test_repo();
        let refs = GitRefs::new(temp_dir.path()).unwrap();

        assert!(!refs.ref_exists("myref").unwrap());
        refs.create_ref("myref", &commit_sha).unwrap();
        assert!(refs.ref_exists("myref").unwrap());
    }

    #[test]
    fn test_list_refs_empty() {
        let (temp_dir, _) = setup_test_repo();
        let refs = GitRefs::new(temp_dir.path()).unwrap();

        let list = refs.list_refs(None).unwrap();
        assert!(list.is_empty());
    }

    #[test]
    fn test_list_refs() {
        let (temp_dir, commit_sha) = setup_test_repo();
        let refs = GitRefs::new(temp_dir.path()).unwrap();

        refs.create_ref("workflows/w1", &commit_sha).unwrap();
        refs.create_ref("workflows/w2", &commit_sha).unwrap();
        refs.create_ref("tasks/t1", &commit_sha).unwrap();

        let all = refs.list_refs(None).unwrap();
        assert_eq!(all.len(), 3);

        let workflows = refs.list_refs(Some("workflows/")).unwrap();
        assert_eq!(workflows.len(), 2);
        assert!(workflows.contains(&"workflows/w1".to_string()));
        assert!(workflows.contains(&"workflows/w2".to_string()));

        let tasks = refs.list_refs(Some("tasks/")).unwrap();
        assert_eq!(tasks.len(), 1);
        assert!(tasks.contains(&"tasks/t1".to_string()));
    }

    #[test]
    fn test_list_refs_with_nonmatching_prefix() {
        let (temp_dir, commit_sha) = setup_test_repo();
        let refs = GitRefs::new(temp_dir.path()).unwrap();

        refs.create_ref("workflows/w1", &commit_sha).unwrap();

        let sessions = refs.list_refs(Some("sessions/")).unwrap();
        assert!(sessions.is_empty());
    }
}
