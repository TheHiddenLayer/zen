//! State migration for transitioning from JSON to git-native storage.
//!
//! This module provides migration utilities to ensure backward compatibility
//! for existing Zen users. The migration is automatic and idempotent.

use std::path::Path;

use crate::config::Config;
use crate::{zlog, zlog_debug, Result};

use super::GitStateManager;

/// The ref name used to mark that migration has been completed.
/// Stored at `refs/zen/migrated`.
const MIGRATION_MARKER_REF: &str = "migrated";

impl GitStateManager {
    /// Check if migration is needed.
    ///
    /// Migration is needed when:
    /// - The migration marker ref does NOT exist, AND
    /// - The JSON state file exists (indicating an existing installation)
    ///
    /// Fresh installations (no state.json) don't need migration.
    pub fn needs_migration(&self) -> Result<bool> {
        zlog_debug!("GitStateManager::needs_migration");

        // If marker exists, migration is already done
        if self.refs().ref_exists(MIGRATION_MARKER_REF)? {
            zlog_debug!("Migration marker exists, no migration needed");
            return Ok(false);
        }

        // Check if state.json exists
        let state_path = Config::state_path()?;
        if !state_path.exists() {
            zlog_debug!(
                "No state.json found at {}, fresh install - no migration needed",
                state_path.display()
            );
            return Ok(false);
        }

        zlog_debug!(
            "Migration needed: state.json exists at {}, no marker ref",
            state_path.display()
        );
        Ok(true)
    }

    /// Check if migration is needed using an explicit state path.
    ///
    /// This variant is useful for testing where we control the state file location.
    pub fn needs_migration_with_path(&self, state_path: &Path) -> Result<bool> {
        zlog_debug!(
            "GitStateManager::needs_migration_with_path path={}",
            state_path.display()
        );

        // If marker exists, migration is already done
        if self.refs().ref_exists(MIGRATION_MARKER_REF)? {
            zlog_debug!("Migration marker exists, no migration needed");
            return Ok(false);
        }

        // Check if state file exists at the given path
        if !state_path.exists() {
            zlog_debug!(
                "No state file found at {}, fresh install - no migration needed",
                state_path.display()
            );
            return Ok(false);
        }

        zlog_debug!(
            "Migration needed: state file exists at {}, no marker ref",
            state_path.display()
        );
        Ok(true)
    }

    /// Perform the migration.
    ///
    /// This creates the migration marker ref pointing to the current HEAD commit.
    /// Sessions continue to use the JSON state file for backward compatibility.
    ///
    /// # Note
    /// This method is idempotent in the sense that calling it multiple times
    /// will update the marker ref to point to the current HEAD.
    pub fn migrate(&self) -> Result<()> {
        zlog!("GitStateManager::migrate - performing state migration");

        let head_sha = self.ops().head_commit()?;

        // Create or update the marker ref
        if self.refs().ref_exists(MIGRATION_MARKER_REF)? {
            self.refs().update_ref(MIGRATION_MARKER_REF, &head_sha)?;
            zlog_debug!("Updated migration marker ref to {}", head_sha);
        } else {
            self.refs().create_ref(MIGRATION_MARKER_REF, &head_sha)?;
            zlog_debug!("Created migration marker ref at {}", head_sha);
        }

        zlog!(
            "Migration complete: marker ref created at refs/zen/{}",
            MIGRATION_MARKER_REF
        );
        Ok(())
    }

    /// Check if migration is needed and perform it if so.
    ///
    /// This is the primary entry point for migration. It combines
    /// `needs_migration()` and `migrate()` into a single operation.
    ///
    /// Returns `true` if migration was performed, `false` if it was skipped.
    pub fn migrate_if_needed(&self) -> Result<bool> {
        zlog_debug!("GitStateManager::migrate_if_needed");

        if self.needs_migration()? {
            self.migrate()?;
            Ok(true)
        } else {
            zlog_debug!("Migration not needed, skipping");
            Ok(false)
        }
    }

    /// Check if migration is needed with an explicit path and perform it if so.
    ///
    /// This variant is useful for testing where we control the state file location.
    ///
    /// Returns `true` if migration was performed, `false` if it was skipped.
    pub fn migrate_if_needed_with_path(&self, state_path: &Path) -> Result<bool> {
        zlog_debug!(
            "GitStateManager::migrate_if_needed_with_path path={}",
            state_path.display()
        );

        if self.needs_migration_with_path(state_path)? {
            self.migrate()?;
            Ok(true)
        } else {
            zlog_debug!("Migration not needed, skipping");
            Ok(false)
        }
    }

    /// Check if the migration marker ref exists.
    ///
    /// This can be used to verify that migration has completed.
    pub fn is_migrated(&self) -> Result<bool> {
        self.refs().ref_exists(MIGRATION_MARKER_REF)
    }

    /// Get the commit SHA that the migration marker points to.
    ///
    /// Returns `None` if migration has not been performed.
    pub fn migration_marker_commit(&self) -> Result<Option<String>> {
        self.refs().read_ref(MIGRATION_MARKER_REF)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use git2::{Repository, Signature};
    use std::fs;
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

    /// Create a mock state.json file in the given directory.
    fn create_mock_state_file(dir: &Path) -> std::path::PathBuf {
        let state_path = dir.join("state.json");
        let state_content = r#"{
            "version": 1,
            "sessions": []
        }"#;
        fs::write(&state_path, state_content).expect("Failed to create mock state file");
        state_path
    }

    #[test]
    fn test_needs_migration_fresh_install() {
        let (temp_dir, _) = setup_test_repo();
        let manager = GitStateManager::new(temp_dir.path()).unwrap();

        // No state file, no marker - fresh install
        let state_path = temp_dir.path().join("nonexistent_state.json");
        let needs = manager.needs_migration_with_path(&state_path).unwrap();
        assert!(!needs, "Fresh install should not need migration");
    }

    #[test]
    fn test_needs_migration_existing_state_no_marker() {
        let (temp_dir, _) = setup_test_repo();
        let manager = GitStateManager::new(temp_dir.path()).unwrap();

        // Create state file
        let state_path = create_mock_state_file(temp_dir.path());

        let needs = manager.needs_migration_with_path(&state_path).unwrap();
        assert!(needs, "Existing state without marker should need migration");
    }

    #[test]
    fn test_needs_migration_already_migrated() {
        let (temp_dir, _) = setup_test_repo();
        let manager = GitStateManager::new(temp_dir.path()).unwrap();

        // Create state file and perform migration
        let state_path = create_mock_state_file(temp_dir.path());
        manager.migrate().unwrap();

        let needs = manager.needs_migration_with_path(&state_path).unwrap();
        assert!(!needs, "Already migrated should not need migration");
    }

    #[test]
    fn test_migrate_creates_marker() {
        let (temp_dir, _) = setup_test_repo();
        let manager = GitStateManager::new(temp_dir.path()).unwrap();

        // Before migration, marker should not exist
        assert!(!manager.is_migrated().unwrap());

        // Perform migration
        manager.migrate().unwrap();

        // After migration, marker should exist
        assert!(manager.is_migrated().unwrap());
    }

    #[test]
    fn test_migrate_idempotent() {
        let (temp_dir, _) = setup_test_repo();
        let manager = GitStateManager::new(temp_dir.path()).unwrap();

        // Create state file
        let state_path = create_mock_state_file(temp_dir.path());

        // First migration
        let migrated1 = manager.migrate_if_needed_with_path(&state_path).unwrap();
        assert!(migrated1, "First migration should happen");

        // Second migration should be skipped
        let migrated2 = manager.migrate_if_needed_with_path(&state_path).unwrap();
        assert!(!migrated2, "Second migration should be skipped");

        // Marker should still exist
        assert!(manager.is_migrated().unwrap());
    }

    #[test]
    fn test_migrate_without_state_file() {
        let (temp_dir, _) = setup_test_repo();
        let manager = GitStateManager::new(temp_dir.path()).unwrap();

        // No state file - fresh install case
        let state_path = temp_dir.path().join("nonexistent_state.json");

        // migrate_if_needed should return false (no migration needed)
        let migrated = manager.migrate_if_needed_with_path(&state_path).unwrap();
        assert!(!migrated, "Fresh install should not trigger migration");

        // Marker should not exist for fresh installs
        assert!(!manager.is_migrated().unwrap());
    }

    #[test]
    fn test_marker_ref_points_to_head() {
        let (temp_dir, commit_sha) = setup_test_repo();
        let manager = GitStateManager::new(temp_dir.path()).unwrap();

        // Perform migration
        manager.migrate().unwrap();

        // Marker should point to HEAD
        let marker_sha = manager.migration_marker_commit().unwrap();
        assert_eq!(marker_sha, Some(commit_sha));
    }

    #[test]
    fn test_migration_marker_commit_before_migration() {
        let (temp_dir, _) = setup_test_repo();
        let manager = GitStateManager::new(temp_dir.path()).unwrap();

        // Before migration, marker commit should be None
        let marker_sha = manager.migration_marker_commit().unwrap();
        assert!(marker_sha.is_none());
    }

    #[test]
    fn test_migrate_can_be_called_directly() {
        let (temp_dir, _) = setup_test_repo();
        let manager = GitStateManager::new(temp_dir.path()).unwrap();

        // Calling migrate directly should work even without state file
        let result = manager.migrate();
        assert!(result.is_ok());

        // Marker should exist
        assert!(manager.is_migrated().unwrap());
    }

    #[test]
    fn test_migrate_updates_marker_on_repeated_calls() {
        let (temp_dir, _) = setup_test_repo();
        let manager = GitStateManager::new(temp_dir.path()).unwrap();

        // First migration
        manager.migrate().unwrap();
        let first_sha = manager.migration_marker_commit().unwrap().unwrap();

        // Create a new commit
        let repo = Repository::open(temp_dir.path()).unwrap();
        let sig = Signature::now("Test", "test@example.com").unwrap();
        let tree_id = repo.index().unwrap().write_tree().unwrap();
        let tree = repo.find_tree(tree_id).unwrap();
        let parent = repo.head().unwrap().peel_to_commit().unwrap();
        let new_commit_id = repo
            .commit(Some("HEAD"), &sig, &sig, "Second commit", &tree, &[&parent])
            .unwrap();
        let new_sha = new_commit_id.to_string();

        // Second direct migrate call should update marker
        manager.migrate().unwrap();
        let second_sha = manager.migration_marker_commit().unwrap().unwrap();

        assert_ne!(first_sha, second_sha);
        assert_eq!(second_sha, new_sha);
    }
}
