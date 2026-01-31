//! Test fixtures for integration tests.
//!
//! Provides helpers for:
//! - Creating temporary git repositories
//! - Mock Claude responses
//! - Predefined task sets

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use tempfile::TempDir;
use tokio::sync::{mpsc, RwLock};

use zen::agent::AgentId;
use zen::core::dag::{DependencyType, TaskDAG};
use zen::core::task::{Task, TaskId};
use zen::orchestration::{
    AgentEvent, AgentPool, HealthConfig, HealthMonitor,
    ConflictResolver, Scheduler, SchedulerEvent,
};
use zen::git::GitOps;

/// A test repository with a temporary directory and initialized git.
pub struct TestRepo {
    /// The temporary directory containing the repo.
    pub temp_dir: TempDir,
    /// Path to the repository root.
    pub path: PathBuf,
}

impl TestRepo {
    /// Create a new test repository with an initial commit.
    pub fn new() -> Self {
        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        let path = temp_dir.path().to_path_buf();

        // Initialize git
        Command::new("git")
            .args(["init"])
            .current_dir(&path)
            .output()
            .expect("Failed to init git");

        // Configure git user
        Command::new("git")
            .args(["config", "user.email", "test@test.com"])
            .current_dir(&path)
            .output()
            .expect("Failed to set user.email");

        Command::new("git")
            .args(["config", "user.name", "Test User"])
            .current_dir(&path)
            .output()
            .expect("Failed to set user.name");

        // Create initial commit
        std::fs::write(path.join("README.md"), "# Test Repository\n")
            .expect("Failed to write README");

        Command::new("git")
            .args(["add", "."])
            .current_dir(&path)
            .output()
            .expect("Failed to git add");

        Command::new("git")
            .args(["commit", "-m", "Initial commit"])
            .current_dir(&path)
            .output()
            .expect("Failed to git commit");

        Self { temp_dir, path }
    }

    /// Create a new branch in the repository.
    pub fn create_branch(&self, name: &str) -> std::io::Result<()> {
        let output = Command::new("git")
            .args(["branch", name])
            .current_dir(&self.path)
            .output()?;

        if !output.status.success() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                String::from_utf8_lossy(&output.stderr).to_string(),
            ));
        }
        Ok(())
    }

    /// Create a file and commit it.
    pub fn create_and_commit(&self, filename: &str, content: &str, message: &str) -> std::io::Result<String> {
        // Write file
        let file_path = self.path.join(filename);
        if let Some(parent) = file_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&file_path, content)?;

        // Stage and commit
        Command::new("git")
            .args(["add", filename])
            .current_dir(&self.path)
            .output()?;

        Command::new("git")
            .args(["commit", "-m", message])
            .current_dir(&self.path)
            .output()?;

        // Get commit hash
        let output = Command::new("git")
            .args(["rev-parse", "HEAD"])
            .current_dir(&self.path)
            .output()?;

        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }

    /// Get the current branch name.
    pub fn current_branch(&self) -> std::io::Result<String> {
        let output = Command::new("git")
            .args(["branch", "--show-current"])
            .current_dir(&self.path)
            .output()?;

        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }

    /// Checkout a branch.
    pub fn checkout(&self, branch: &str) -> std::io::Result<()> {
        let output = Command::new("git")
            .args(["checkout", branch])
            .current_dir(&self.path)
            .output()?;

        if !output.status.success() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                String::from_utf8_lossy(&output.stderr).to_string(),
            ));
        }
        Ok(())
    }

    /// Check if a branch exists.
    pub fn branch_exists(&self, name: &str) -> bool {
        let output = Command::new("git")
            .args(["branch", "--list", name])
            .current_dir(&self.path)
            .output()
            .expect("Failed to list branches");

        !String::from_utf8_lossy(&output.stdout).trim().is_empty()
    }

    /// Get GitOps for this repository.
    pub fn git_ops(&self) -> zen::Result<GitOps> {
        GitOps::new(&self.path)
    }
}

impl Default for TestRepo {
    fn default() -> Self {
        Self::new()
    }
}

/// Create a test task with the given name.
pub fn test_task(name: &str) -> Task {
    Task::new(name, &format!("{} description", name))
}

/// Create a test task with a specific ID.
pub fn test_task_with_id(name: &str, id: TaskId) -> Task {
    let mut task = Task::new(name, &format!("{} description", name));
    // Note: Task::new generates a new ID, so we need to update it
    // This is a workaround since Task doesn't have a constructor that takes an ID
    task
}

/// Create a set of predefined independent tasks.
pub fn independent_tasks(count: usize) -> Vec<Task> {
    (0..count)
        .map(|i| test_task(&format!("task-{}", i)))
        .collect()
}

/// Create a diamond-shaped task DAG: A, B -> C
///
/// ```text
///   A
///    \
///     C
///    /
///   B
/// ```
pub fn diamond_dag() -> (TaskDAG, TaskId, TaskId, TaskId) {
    let mut dag = TaskDAG::new();

    let task_a = test_task("task-a");
    let task_b = test_task("task-b");
    let task_c = test_task("task-c");

    let id_a = task_a.id;
    let id_b = task_b.id;
    let id_c = task_c.id;

    dag.add_task(task_a);
    dag.add_task(task_b);
    dag.add_task(task_c);

    dag.add_dependency(&id_a, &id_c, DependencyType::DataDependency)
        .expect("Failed to add dependency A->C");
    dag.add_dependency(&id_b, &id_c, DependencyType::DataDependency)
        .expect("Failed to add dependency B->C");

    (dag, id_a, id_b, id_c)
}

/// Create a chain DAG: A -> B -> C
pub fn chain_dag() -> (TaskDAG, TaskId, TaskId, TaskId) {
    let mut dag = TaskDAG::new();

    let task_a = test_task("task-a");
    let task_b = test_task("task-b");
    let task_c = test_task("task-c");

    let id_a = task_a.id;
    let id_b = task_b.id;
    let id_c = task_c.id;

    dag.add_task(task_a);
    dag.add_task(task_b);
    dag.add_task(task_c);

    dag.add_dependency(&id_a, &id_b, DependencyType::DataDependency)
        .expect("Failed to add dependency A->B");
    dag.add_dependency(&id_b, &id_c, DependencyType::DataDependency)
        .expect("Failed to add dependency B->C");

    (dag, id_a, id_b, id_c)
}

/// Test harness for scheduler tests.
pub struct SchedulerHarness {
    pub scheduler: Scheduler,
    pub dag: Arc<RwLock<TaskDAG>>,
    pub pool: Arc<RwLock<AgentPool>>,
    pub event_rx: mpsc::Receiver<SchedulerEvent>,
    pub agent_rx: mpsc::Receiver<AgentEvent>,
    pub repo: TestRepo,
}

impl SchedulerHarness {
    /// Create a new scheduler harness with a test repository.
    pub fn new(max_agents: usize) -> Self {
        let repo = TestRepo::new();
        let dag = Arc::new(RwLock::new(TaskDAG::new()));
        let (pool_tx, agent_rx) = mpsc::channel(100);
        let pool = Arc::new(RwLock::new(AgentPool::new(max_agents, pool_tx)));
        let (event_tx, event_rx) = mpsc::channel(100);

        let scheduler = Scheduler::new(
            Arc::clone(&dag),
            Arc::clone(&pool),
            event_tx,
            repo.path.clone(),
        );

        Self {
            scheduler,
            dag,
            pool,
            event_rx,
            agent_rx,
            repo,
        }
    }

    /// Add a task to the DAG.
    pub async fn add_task(&self, task: Task) {
        let mut dag = self.dag.write().await;
        dag.add_task(task);
    }

    /// Add a dependency between tasks.
    pub async fn add_dependency(&self, from: &TaskId, to: &TaskId) -> zen::Result<()> {
        let mut dag = self.dag.write().await;
        dag.add_dependency(from, to, DependencyType::DataDependency)
    }

    /// Get the next scheduler event.
    pub async fn next_event(&mut self) -> Option<SchedulerEvent> {
        self.event_rx.recv().await
    }

    /// Get the next scheduler event with timeout.
    pub async fn next_event_timeout(&mut self, timeout_ms: u64) -> Option<SchedulerEvent> {
        tokio::time::timeout(
            std::time::Duration::from_millis(timeout_ms),
            self.event_rx.recv(),
        )
        .await
        .ok()
        .flatten()
    }
}

/// Test harness for health monitor tests.
pub struct HealthMonitorHarness {
    pub monitor: HealthMonitor,
    pub pool: Arc<RwLock<AgentPool>>,
    pub event_rx: mpsc::Receiver<zen::orchestration::HealthEvent>,
}

impl HealthMonitorHarness {
    /// Create a new health monitor harness.
    pub fn new(max_agents: usize, config: Option<HealthConfig>) -> Self {
        let (pool_tx, _agent_rx) = mpsc::channel(100);
        let pool = Arc::new(RwLock::new(AgentPool::new(max_agents, pool_tx)));
        let (event_tx, event_rx) = mpsc::channel(100);

        let config = config.unwrap_or_default();
        let monitor = HealthMonitor::new(config, Arc::clone(&pool), event_tx);

        Self {
            monitor,
            pool,
            event_rx,
        }
    }
}

/// Test harness for conflict resolver tests.
pub struct ConflictResolverHarness {
    pub resolver: ConflictResolver,
    pub pool: Arc<RwLock<AgentPool>>,
    pub repo: TestRepo,
}

impl ConflictResolverHarness {
    /// Create a new conflict resolver harness.
    pub fn new(max_agents: usize) -> zen::Result<Self> {
        let repo = TestRepo::new();
        let git_ops = repo.git_ops()?;
        let (pool_tx, _agent_rx) = mpsc::channel(100);
        let pool = Arc::new(RwLock::new(AgentPool::new(max_agents, pool_tx)));

        let resolver = ConflictResolver::new(git_ops, Arc::clone(&pool));

        Ok(Self {
            resolver,
            pool,
            repo,
        })
    }

    /// Create a branch with a file modification to set up a conflict.
    pub fn create_conflict_branch(
        &self,
        branch_name: &str,
        filename: &str,
        content: &str,
    ) -> std::io::Result<()> {
        // Create and checkout the branch
        self.repo.create_branch(branch_name)?;
        self.repo.checkout(branch_name)?;

        // Modify the file
        let file_path = self.repo.path.join(filename);
        std::fs::write(&file_path, content)?;

        // Commit the change
        Command::new("git")
            .args(["add", filename])
            .current_dir(&self.repo.path)
            .output()?;

        Command::new("git")
            .args(["commit", "-m", &format!("Modify {} on {}", filename, branch_name)])
            .current_dir(&self.repo.path)
            .output()?;

        Ok(())
    }
}

/// Mock Claude responder that returns predefined responses.
///
/// This allows tests to run without making actual API calls.
pub struct MockClaudeResponder {
    responses: Vec<MockResponse>,
    current_index: usize,
}

/// A mock response from Claude.
pub struct MockResponse {
    /// Whether this response indicates success.
    pub success: bool,
    /// The output text.
    pub output: String,
    /// Simulated duration in milliseconds.
    pub duration_ms: u64,
}

impl MockClaudeResponder {
    /// Create a new mock responder with default success responses.
    pub fn new() -> Self {
        Self {
            responses: vec![MockResponse::success("Task completed successfully.")],
            current_index: 0,
        }
    }

    /// Create a responder with custom responses.
    pub fn with_responses(responses: Vec<MockResponse>) -> Self {
        Self {
            responses,
            current_index: 0,
        }
    }

    /// Get the next response.
    pub fn next_response(&mut self) -> &MockResponse {
        let response = &self.responses[self.current_index % self.responses.len()];
        self.current_index += 1;
        response
    }
}

impl Default for MockClaudeResponder {
    fn default() -> Self {
        Self::new()
    }
}

impl MockResponse {
    /// Create a successful response.
    pub fn success(output: &str) -> Self {
        Self {
            success: true,
            output: output.to_string(),
            duration_ms: 100,
        }
    }

    /// Create an error response.
    pub fn error(output: &str) -> Self {
        Self {
            success: false,
            output: output.to_string(),
            duration_ms: 50,
        }
    }

    /// Create a response with a question.
    pub fn question(question: &str) -> Self {
        Self {
            success: true,
            output: format!("? {}", question),
            duration_ms: 100,
        }
    }
}

/// Helper to clean up worktrees created during tests.
pub fn cleanup_test_worktrees(task_ids: &[TaskId]) {
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    let worktrees_dir = home.join(".zen").join("worktrees");

    for task_id in task_ids {
        let worktree_path = worktrees_dir.join(task_id.short());
        if worktree_path.exists() {
            let _ = std::fs::remove_dir_all(&worktree_path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_test_repo_creation() {
        let repo = TestRepo::new();
        assert!(repo.path.exists());
        assert!(repo.path.join(".git").exists());
        assert!(repo.path.join("README.md").exists());
    }

    #[test]
    fn test_test_repo_create_branch() {
        let repo = TestRepo::new();
        repo.create_branch("feature").unwrap();
        assert!(repo.branch_exists("feature"));
    }

    #[test]
    fn test_test_repo_create_and_commit() {
        let repo = TestRepo::new();
        let commit = repo.create_and_commit("test.txt", "hello", "Add test file").unwrap();
        assert!(!commit.is_empty());
        assert!(repo.path.join("test.txt").exists());
    }

    #[test]
    fn test_diamond_dag() {
        let (dag, id_a, id_b, id_c) = diamond_dag();

        // A and B should be ready
        let completed = HashSet::new();
        let ready = dag.ready_tasks(&completed);
        assert_eq!(ready.len(), 2);

        // C should not be ready
        let ready_ids: Vec<_> = ready.iter().map(|t| t.id).collect();
        assert!(!ready_ids.contains(&id_c));
    }

    #[test]
    fn test_chain_dag() {
        let (dag, id_a, id_b, id_c) = chain_dag();

        // Only A should be ready
        let completed = HashSet::new();
        let ready = dag.ready_tasks(&completed);
        assert_eq!(ready.len(), 1);
        assert_eq!(ready[0].id, id_a);
    }

    #[test]
    fn test_mock_response_success() {
        let response = MockResponse::success("Done!");
        assert!(response.success);
        assert_eq!(response.output, "Done!");
    }

    #[test]
    fn test_mock_response_error() {
        let response = MockResponse::error("Failed!");
        assert!(!response.success);
        assert_eq!(response.output, "Failed!");
    }

    #[test]
    fn test_mock_claude_responder() {
        let mut responder = MockClaudeResponder::with_responses(vec![
            MockResponse::success("First"),
            MockResponse::success("Second"),
        ]);

        assert_eq!(responder.next_response().output, "First");
        assert_eq!(responder.next_response().output, "Second");
        // Wraps around
        assert_eq!(responder.next_response().output, "First");
    }

    #[tokio::test]
    async fn test_scheduler_harness() {
        let harness = SchedulerHarness::new(4);

        let task = test_task("test");
        harness.add_task(task).await;

        let dag = harness.dag.read().await;
        assert_eq!(dag.task_count(), 1);
    }
}
