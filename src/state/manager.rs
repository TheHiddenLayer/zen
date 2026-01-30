//! GitStateManager - unified interface for git-native state persistence.

use std::path::{Path, PathBuf};

use crate::git::GitOps;
use crate::git_notes::GitNotes;
use crate::git_refs::GitRefs;
use crate::workflow::{Workflow, WorkflowId};
use crate::{zlog_debug, Result};

/// The prefix for workflow refs and notes namespaces.
const WORKFLOWS_PREFIX: &str = "workflows";

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

    // -------------------------------------------------------------------------
    // Workflow Persistence Methods
    // -------------------------------------------------------------------------

    /// Build the ref name for a workflow ID.
    fn workflow_ref_name(id: &WorkflowId) -> String {
        format!("{}/{}", WORKFLOWS_PREFIX, id)
    }

    /// Build the notes namespace for a workflow ID.
    /// Each workflow gets its own namespace to avoid collisions when multiple
    /// workflows share the same commit.
    fn workflow_notes_namespace(id: &WorkflowId) -> String {
        format!("{}/{}", WORKFLOWS_PREFIX, id)
    }

    /// Save a workflow to git-native storage.
    ///
    /// Creates or updates:
    /// - A ref at `refs/zen/workflows/{id}` pointing to the current HEAD commit
    /// - A note on the target commit containing the workflow JSON
    ///
    /// # Errors
    /// Returns an error if git operations fail.
    pub fn save_workflow(&self, workflow: &Workflow) -> Result<()> {
        zlog_debug!("GitStateManager::save_workflow id={}", workflow.id);

        let ref_name = Self::workflow_ref_name(&workflow.id);
        let head_sha = self.ops.head_commit()?;

        // Create or update the ref
        if self.refs.ref_exists(&ref_name)? {
            self.refs.update_ref(&ref_name, &head_sha)?;
        } else {
            self.refs.create_ref(&ref_name, &head_sha)?;
        }

        // Attach workflow as a note on the commit (using per-workflow namespace)
        let notes_ns = Self::workflow_notes_namespace(&workflow.id);
        self.notes.set_note(&head_sha, &notes_ns, workflow)?;

        zlog_debug!("Saved workflow {} to ref {} with note on {}", workflow.id, ref_name, head_sha);
        Ok(())
    }

    /// Load a workflow from git-native storage.
    ///
    /// Returns `None` if the workflow doesn't exist.
    ///
    /// # Errors
    /// Returns an error if git operations or deserialization fail.
    pub fn load_workflow(&self, id: &WorkflowId) -> Result<Option<Workflow>> {
        zlog_debug!("GitStateManager::load_workflow id={}", id);

        let ref_name = Self::workflow_ref_name(id);

        // Read the ref to get the commit SHA
        let commit_sha = match self.refs.read_ref(&ref_name)? {
            Some(sha) => sha,
            None => {
                zlog_debug!("Workflow {} not found (no ref)", id);
                return Ok(None);
            }
        };

        // Get the note from that commit (using per-workflow namespace)
        let notes_ns = Self::workflow_notes_namespace(id);
        let workflow: Option<Workflow> = self.notes.get_note(&commit_sha, &notes_ns)?;
        zlog_debug!("Loaded workflow {} from commit {}: {:?}", id, commit_sha, workflow.is_some());
        Ok(workflow)
    }

    /// List all saved workflows.
    ///
    /// Returns an empty vector if no workflows exist.
    ///
    /// # Errors
    /// Returns an error if git operations or deserialization fail.
    pub fn list_workflows(&self) -> Result<Vec<Workflow>> {
        zlog_debug!("GitStateManager::list_workflows");

        let ref_names = self.refs.list_refs(Some(&format!("{}/", WORKFLOWS_PREFIX)))?;
        let mut workflows = Vec::new();

        for ref_name in ref_names {
            // Extract workflow ID from ref name (workflows/{id})
            if let Some(id_str) = ref_name.strip_prefix(&format!("{}/", WORKFLOWS_PREFIX)) {
                if let Ok(id) = id_str.parse::<WorkflowId>() {
                    if let Some(workflow) = self.load_workflow(&id)? {
                        workflows.push(workflow);
                    }
                }
            }
        }

        zlog_debug!("Listed {} workflows", workflows.len());
        Ok(workflows)
    }

    /// Delete a workflow from git-native storage.
    ///
    /// This is idempotent - no error if the workflow doesn't exist.
    ///
    /// # Errors
    /// Returns an error if git operations fail.
    pub fn delete_workflow(&self, id: &WorkflowId) -> Result<()> {
        zlog_debug!("GitStateManager::delete_workflow id={}", id);

        let ref_name = Self::workflow_ref_name(id);

        // Get the commit SHA before deleting the ref
        if let Some(commit_sha) = self.refs.read_ref(&ref_name)? {
            // Delete the note first (using per-workflow namespace)
            let notes_ns = Self::workflow_notes_namespace(id);
            self.notes.delete_note(&commit_sha, &notes_ns)?;
        }

        // Delete the ref (idempotent)
        self.refs.delete_ref(&ref_name)?;

        zlog_debug!("Deleted workflow {}", id);
        Ok(())
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

    // -------------------------------------------------------------------------
    // Workflow Persistence Tests
    // -------------------------------------------------------------------------

    use crate::workflow::{Workflow, WorkflowConfig, WorkflowId, WorkflowPhase, WorkflowStatus};

    #[test]
    fn test_save_and_load_workflow_roundtrip() {
        let (temp_dir, _) = setup_test_repo();
        let manager = GitStateManager::new(temp_dir.path()).unwrap();

        let config = WorkflowConfig::default();
        let workflow = Workflow::new("build user authentication", config);
        let id = workflow.id;

        // Save the workflow
        manager.save_workflow(&workflow).unwrap();

        // Load it back
        let loaded = manager.load_workflow(&id).unwrap();
        assert!(loaded.is_some());
        let loaded = loaded.unwrap();

        // Verify all fields match
        assert_eq!(loaded.id, workflow.id);
        assert_eq!(loaded.name, workflow.name);
        assert_eq!(loaded.prompt, workflow.prompt);
        assert_eq!(loaded.phase, workflow.phase);
        assert_eq!(loaded.status, workflow.status);
        assert_eq!(loaded.config.update_docs, workflow.config.update_docs);
        assert_eq!(loaded.config.max_parallel_agents, workflow.config.max_parallel_agents);
        assert_eq!(loaded.config.staging_branch_prefix, workflow.config.staging_branch_prefix);
    }

    #[test]
    fn test_save_workflow_overwrites_existing() {
        let (temp_dir, _) = setup_test_repo();
        let manager = GitStateManager::new(temp_dir.path()).unwrap();

        let config = WorkflowConfig::default();
        let mut workflow = Workflow::new("initial prompt", config);
        let id = workflow.id;

        // Save initial version
        manager.save_workflow(&workflow).unwrap();

        // Modify and save again
        workflow.start();
        assert_eq!(workflow.status, WorkflowStatus::Running);
        manager.save_workflow(&workflow).unwrap();

        // Load and verify updated version
        let loaded = manager.load_workflow(&id).unwrap().unwrap();
        assert_eq!(loaded.status, WorkflowStatus::Running);
        assert!(loaded.started_at.is_some());
    }

    #[test]
    fn test_list_multiple_workflows() {
        let (temp_dir, _) = setup_test_repo();
        let manager = GitStateManager::new(temp_dir.path()).unwrap();

        let config = WorkflowConfig::default();
        let workflow1 = Workflow::new("first workflow", config.clone());
        let workflow2 = Workflow::new("second workflow", config.clone());
        let workflow3 = Workflow::new("third workflow", config);

        let id1 = workflow1.id;
        let id2 = workflow2.id;
        let id3 = workflow3.id;

        // Save all three
        manager.save_workflow(&workflow1).unwrap();
        manager.save_workflow(&workflow2).unwrap();
        manager.save_workflow(&workflow3).unwrap();

        // List all
        let workflows = manager.list_workflows().unwrap();
        assert_eq!(workflows.len(), 3);

        // Verify all IDs are present
        let ids: Vec<WorkflowId> = workflows.iter().map(|w| w.id).collect();
        assert!(ids.contains(&id1));
        assert!(ids.contains(&id2));
        assert!(ids.contains(&id3));
    }

    #[test]
    fn test_load_nonexistent_workflow_returns_none() {
        let (temp_dir, _) = setup_test_repo();
        let manager = GitStateManager::new(temp_dir.path()).unwrap();

        let nonexistent_id = WorkflowId::new();
        let result = manager.load_workflow(&nonexistent_id).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_delete_workflow() {
        let (temp_dir, _) = setup_test_repo();
        let manager = GitStateManager::new(temp_dir.path()).unwrap();

        let config = WorkflowConfig::default();
        let workflow = Workflow::new("delete me", config);
        let id = workflow.id;

        // Save the workflow
        manager.save_workflow(&workflow).unwrap();

        // Verify it exists
        assert!(manager.load_workflow(&id).unwrap().is_some());

        // Delete it
        manager.delete_workflow(&id).unwrap();

        // Verify it's gone
        assert!(manager.load_workflow(&id).unwrap().is_none());
    }

    #[test]
    fn test_delete_nonexistent_workflow_is_idempotent() {
        let (temp_dir, _) = setup_test_repo();
        let manager = GitStateManager::new(temp_dir.path()).unwrap();

        let nonexistent_id = WorkflowId::new();

        // Should not error
        let result = manager.delete_workflow(&nonexistent_id);
        assert!(result.is_ok());
    }

    #[test]
    fn test_list_workflows_empty() {
        let (temp_dir, _) = setup_test_repo();
        let manager = GitStateManager::new(temp_dir.path()).unwrap();

        let workflows = manager.list_workflows().unwrap();
        assert!(workflows.is_empty());
    }

    #[test]
    fn test_workflow_with_tasks_roundtrip() {
        use crate::workflow::TaskId;

        let (temp_dir, _) = setup_test_repo();
        let manager = GitStateManager::new(temp_dir.path()).unwrap();

        let config = WorkflowConfig::default();
        let mut workflow = Workflow::new("workflow with tasks", config);
        workflow.task_ids.push(TaskId::new());
        workflow.task_ids.push(TaskId::new());
        let id = workflow.id;

        manager.save_workflow(&workflow).unwrap();

        let loaded = manager.load_workflow(&id).unwrap().unwrap();
        assert_eq!(loaded.task_ids.len(), 2);
    }

    #[test]
    fn test_workflow_with_all_phases_roundtrip() {
        let (temp_dir, _) = setup_test_repo();
        let manager = GitStateManager::new(temp_dir.path()).unwrap();

        let config = WorkflowConfig::default();
        let mut workflow = Workflow::new("complete workflow", config);
        workflow.start();
        workflow.complete();
        let id = workflow.id;

        manager.save_workflow(&workflow).unwrap();

        let loaded = manager.load_workflow(&id).unwrap().unwrap();
        assert_eq!(loaded.status, WorkflowStatus::Completed);
        assert_eq!(loaded.phase, WorkflowPhase::Complete);
        assert!(loaded.started_at.is_some());
        assert!(loaded.completed_at.is_some());
    }
}
