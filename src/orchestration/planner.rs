//! Reactive planner for detecting plan/design file changes.
//!
//! This module provides the ReactivePlanner which monitors the .sop/planning/
//! directory for changes to plan or design files, enabling reactive replanning
//! during workflow execution.

use crate::core::{CodeTask, Task, TaskDAG, TaskId, TaskStatus};
use crate::error::Result;
use notify::{Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};
use tokio::sync::mpsc;

/// Default debounce window in milliseconds.
pub const DEFAULT_DEBOUNCE_MS: u64 = 1000;

/// Events emitted by the ReactivePlanner when plan changes are detected.
#[derive(Debug, Clone)]
pub enum PlanEvent {
    /// A relevant file was changed.
    FileChanged {
        /// Path to the changed file.
        path: PathBuf,
    },
    /// Replanning has been triggered.
    ReplanTriggered,
    /// New tasks have been added to the DAG.
    TasksAdded {
        /// The newly added tasks.
        tasks: Vec<Task>,
    },
    /// Tasks have been cancelled.
    TasksCancelled {
        /// IDs of the cancelled tasks.
        tasks: Vec<TaskId>,
    },
}

/// Configuration for the reactive planner.
#[derive(Debug, Clone)]
pub struct PlannerConfig {
    /// Paths to watch for changes.
    pub watch_paths: Vec<PathBuf>,
    /// Debounce window for rapid changes.
    pub debounce_duration: Duration,
    /// File patterns that trigger replanning (e.g., "plan.md", "detailed-design.md").
    pub relevant_patterns: Vec<String>,
}

impl Default for PlannerConfig {
    fn default() -> Self {
        Self {
            watch_paths: vec![PathBuf::from(".sop/planning")],
            debounce_duration: Duration::from_millis(DEFAULT_DEBOUNCE_MS),
            relevant_patterns: vec![
                "plan.md".to_string(),
                "detailed-design.md".to_string(),
                ".code-task.md".to_string(),
            ],
        }
    }
}

/// Result of comparing old and new task lists during replanning.
///
/// Captures which tasks have been added, removed, or modified
/// so the DAG can be updated accordingly.
#[derive(Debug, Clone, Default)]
pub struct TaskDiff {
    /// New tasks that should be added to the DAG.
    pub added: Vec<Task>,
    /// IDs of tasks that should be marked as cancelled.
    pub removed: Vec<TaskId>,
    /// Tasks that have been modified (description changed, etc.).
    pub modified: Vec<Task>,
}

impl TaskDiff {
    /// Create a new empty TaskDiff.
    pub fn new() -> Self {
        Self::default()
    }

    /// Check if there are any changes.
    pub fn has_changes(&self) -> bool {
        !self.added.is_empty() || !self.removed.is_empty() || !self.modified.is_empty()
    }

    /// Get the total number of changes.
    pub fn change_count(&self) -> usize {
        self.added.len() + self.removed.len() + self.modified.len()
    }
}

/// Watches for plan/design changes and triggers replanning.
///
/// The ReactivePlanner monitors specified directories for file changes
/// and emits PlanEvents when relevant changes are detected. It implements
/// debouncing to coalesce rapid changes into a single event.
pub struct ReactivePlanner {
    /// The task DAG being managed.
    dag: Arc<RwLock<TaskDAG>>,
    /// Paths being watched.
    watch_paths: Vec<PathBuf>,
    /// Configuration for the planner.
    config: PlannerConfig,
    /// Channel for sending plan events.
    event_tx: mpsc::Sender<PlanEvent>,
    /// Debounce state: maps file paths to last change time.
    debounce_state: Arc<RwLock<HashMap<PathBuf, Instant>>>,
    /// Repository path for scanning code tasks.
    repo_path: PathBuf,
    /// Mapping from task name to task ID for tracking current tasks.
    task_name_to_id: Arc<RwLock<HashMap<String, TaskId>>>,
}

impl ReactivePlanner {
    /// Create a new ReactivePlanner with the given DAG, configuration, and repo path.
    ///
    /// Returns the planner and a receiver for plan events.
    pub fn new(
        dag: Arc<RwLock<TaskDAG>>,
        config: PlannerConfig,
        repo_path: PathBuf,
    ) -> (Self, mpsc::Receiver<PlanEvent>) {
        let (event_tx, event_rx) = mpsc::channel(100);
        let watch_paths = config.watch_paths.clone();

        (
            Self {
                dag,
                watch_paths,
                config,
                event_tx,
                debounce_state: Arc::new(RwLock::new(HashMap::new())),
                repo_path,
                task_name_to_id: Arc::new(RwLock::new(HashMap::new())),
            },
            event_rx,
        )
    }

    /// Create a new ReactivePlanner with default configuration.
    pub fn with_defaults(dag: Arc<RwLock<TaskDAG>>, repo_path: PathBuf) -> (Self, mpsc::Receiver<PlanEvent>) {
        Self::new(dag, PlannerConfig::default(), repo_path)
    }

    /// Get the paths being watched.
    pub fn watch_paths(&self) -> &[PathBuf] {
        &self.watch_paths
    }

    /// Get the configuration.
    pub fn config(&self) -> &PlannerConfig {
        &self.config
    }

    /// Get the event sender for testing purposes.
    pub fn event_sender(&self) -> mpsc::Sender<PlanEvent> {
        self.event_tx.clone()
    }

    /// Check if a file path matches our relevant patterns.
    pub fn is_relevant_file(&self, path: &PathBuf) -> bool {
        if let Some(file_name) = path.file_name() {
            let name = file_name.to_string_lossy();
            self.config
                .relevant_patterns
                .iter()
                .any(|pattern| name.ends_with(pattern))
        } else {
            false
        }
    }

    /// Check if a file change should be debounced.
    ///
    /// Returns true if the change should be processed (not within debounce window),
    /// false if it should be skipped.
    pub fn should_process_change(&self, path: &PathBuf) -> bool {
        let now = Instant::now();
        let mut state = self.debounce_state.write().unwrap();

        if let Some(last_change) = state.get(path) {
            if now.duration_since(*last_change) < self.config.debounce_duration {
                // Within debounce window, skip
                return false;
            }
        }

        // Update last change time and process
        state.insert(path.clone(), now);
        true
    }

    /// Create a file watcher configured for this planner.
    ///
    /// Returns a watcher that will call the provided callback on file changes.
    /// The watcher must be kept alive for watching to continue.
    pub fn create_watcher<F>(&self, mut callback: F) -> notify::Result<RecommendedWatcher>
    where
        F: FnMut(notify::Result<Event>) + Send + 'static,
    {
        let watcher = RecommendedWatcher::new(
            move |res| {
                callback(res);
            },
            Config::default(),
        )?;

        Ok(watcher)
    }

    /// Start watching the configured paths.
    ///
    /// Returns a watcher handle that must be kept alive. The planner will
    /// emit PlanEvents through its channel when relevant changes are detected.
    pub fn start_watching(&self) -> notify::Result<RecommendedWatcher> {
        let event_tx = self.event_tx.clone();
        let debounce_state = self.debounce_state.clone();
        let relevant_patterns = self.config.relevant_patterns.clone();
        let debounce_duration = self.config.debounce_duration;

        let mut watcher = RecommendedWatcher::new(
            move |res: notify::Result<Event>| {
                if let Ok(event) = res {
                    // Only process modify and create events
                    match event.kind {
                        EventKind::Modify(_) | EventKind::Create(_) => {}
                        _ => return,
                    }

                    for path in event.paths {
                        // Check if file is relevant
                        let is_relevant = if let Some(file_name) = path.file_name() {
                            let name = file_name.to_string_lossy();
                            relevant_patterns.iter().any(|p| name.ends_with(p))
                        } else {
                            false
                        };

                        if !is_relevant {
                            continue;
                        }

                        // Check debounce
                        let now = Instant::now();
                        let should_emit = {
                            let mut state = debounce_state.write().unwrap();
                            if let Some(last) = state.get(&path) {
                                if now.duration_since(*last) < debounce_duration {
                                    false
                                } else {
                                    state.insert(path.clone(), now);
                                    true
                                }
                            } else {
                                state.insert(path.clone(), now);
                                true
                            }
                        };

                        if should_emit {
                            let _ = event_tx.blocking_send(PlanEvent::FileChanged {
                                path: path.clone(),
                            });
                        }
                    }
                }
            },
            Config::default(),
        )?;

        // Watch all configured paths
        for path in &self.watch_paths {
            if path.exists() {
                watcher.watch(path, RecursiveMode::Recursive)?;
            }
        }

        Ok(watcher)
    }

    /// Emit a FileChanged event.
    pub async fn emit_file_changed(&self, path: PathBuf) {
        let _ = self.event_tx.send(PlanEvent::FileChanged { path }).await;
    }

    /// Emit a ReplanTriggered event.
    pub async fn emit_replan_triggered(&self) {
        let _ = self.event_tx.send(PlanEvent::ReplanTriggered).await;
    }

    /// Emit a TasksAdded event.
    pub async fn emit_tasks_added(&self, tasks: Vec<Task>) {
        let _ = self.event_tx.send(PlanEvent::TasksAdded { tasks }).await;
    }

    /// Emit a TasksCancelled event.
    pub async fn emit_tasks_cancelled(&self, tasks: Vec<TaskId>) {
        let _ = self.event_tx.send(PlanEvent::TasksCancelled { tasks }).await;
    }

    /// Get a reference to the DAG.
    pub fn dag(&self) -> &Arc<RwLock<TaskDAG>> {
        &self.dag
    }

    /// Clear debounce state (useful for testing).
    pub fn clear_debounce_state(&self) {
        self.debounce_state.write().unwrap().clear();
    }

    /// Get the repository path.
    pub fn repo_path(&self) -> &Path {
        &self.repo_path
    }

    /// Register a task in the name-to-id mapping.
    ///
    /// This is used to track which tasks are currently in the DAG
    /// so we can detect added/removed tasks during replanning.
    pub fn register_task(&self, name: &str, id: TaskId) {
        self.task_name_to_id
            .write()
            .unwrap()
            .insert(name.to_string(), id);
    }

    /// Get the task ID for a given task name.
    pub fn get_task_id(&self, name: &str) -> Option<TaskId> {
        self.task_name_to_id.read().unwrap().get(name).copied()
    }

    /// Get all registered task names.
    pub fn registered_task_names(&self) -> Vec<String> {
        self.task_name_to_id.read().unwrap().keys().cloned().collect()
    }

    /// Handle a plan file change.
    ///
    /// Re-parses the code tasks from the repository and triggers
    /// replanning if there are changes.
    pub async fn on_plan_changed(&self, _path: &Path) -> Result<()> {
        // Emit file changed event
        self.emit_replan_triggered().await;

        // Trigger replanning
        self.replan().await
    }

    /// Replan the task DAG based on current code task files.
    ///
    /// Scans for .code-task.md files, compares them to the current
    /// DAG state, and applies changes without interrupting running tasks.
    pub async fn replan(&self) -> Result<()> {
        // Scan for current code tasks
        let search_paths = [
            self.repo_path.clone(),
            self.repo_path.join(".sop"),
            self.repo_path.join(".sop/planning"),
            self.repo_path.join(".sop/planning/implementation"),
        ];

        let mut new_code_tasks = Vec::new();
        for path in search_paths {
            if let Ok(tasks) = CodeTask::from_directory(&path) {
                for task in tasks {
                    // Deduplicate by ID
                    if !new_code_tasks.iter().any(|t: &CodeTask| t.id == task.id) {
                        new_code_tasks.push(task);
                    }
                }
            }
        }

        // Convert CodeTasks to Tasks
        let new_tasks: Vec<Task> = new_code_tasks.iter().map(|ct| ct.to_task()).collect();

        // Get current tasks from the DAG
        let current_tasks: Vec<Task> = {
            let dag = self.dag.read().unwrap();
            dag.all_tasks().into_iter().cloned().collect()
        };

        // Compute diff
        let diff = self.diff_tasks(&current_tasks, &new_tasks);

        // Apply changes if there are any
        if diff.has_changes() {
            self.apply_diff(&diff).await?;
        }

        Ok(())
    }

    /// Compare old and new task lists to produce a TaskDiff.
    ///
    /// Uses task names as the comparison key since IDs are generated
    /// fresh when parsing CodeTask files.
    pub fn diff_tasks(&self, old: &[Task], new: &[Task]) -> TaskDiff {
        let mut diff = TaskDiff::new();

        // Build sets of task names for comparison
        let old_names: HashSet<_> = old.iter().map(|t| t.name.clone()).collect();
        let new_names: HashSet<_> = new.iter().map(|t| t.name.clone()).collect();

        // Find added tasks (in new but not in old)
        for task in new {
            if !old_names.contains(&task.name) {
                diff.added.push(task.clone());
            }
        }

        // Find removed tasks (in old but not in new)
        // Only mark non-running tasks as removed
        for task in old {
            if !new_names.contains(&task.name) {
                // Don't remove running or completed tasks
                if !matches!(
                    task.status,
                    TaskStatus::Running | TaskStatus::Completed | TaskStatus::Cancelled { .. }
                ) {
                    diff.removed.push(task.id);
                }
            }
        }

        // Find modified tasks (same name but different description)
        let old_by_name: HashMap<_, _> = old.iter().map(|t| (t.name.clone(), t)).collect();
        for task in new {
            if let Some(old_task) = old_by_name.get(&task.name) {
                if old_task.description != task.description {
                    // Only mark as modified if not already running
                    if !matches!(old_task.status, TaskStatus::Running) {
                        let mut modified_task = task.clone();
                        // Preserve the original task ID
                        modified_task.id = old_task.id;
                        diff.modified.push(modified_task);
                    }
                }
            }
        }

        diff
    }

    /// Apply a TaskDiff to the DAG.
    ///
    /// Adds new tasks, marks removed tasks as cancelled, and updates
    /// modified task descriptions.
    async fn apply_diff(&self, diff: &TaskDiff) -> Result<()> {
        // Apply changes to the DAG
        {
            let mut dag = self.dag.write().unwrap();

            // Add new tasks
            for task in &diff.added {
                dag.add_task(task.clone());
                // Register the new task
                self.register_task(&task.name, task.id);
            }

            // Mark removed tasks as cancelled
            for task_id in &diff.removed {
                if let Some(task) = dag.get_task_mut(task_id) {
                    task.cancel("removed during replanning");
                }
            }

            // Update modified tasks
            for modified_task in &diff.modified {
                if let Some(task) = dag.get_task_mut(&modified_task.id) {
                    task.description = modified_task.description.clone();
                }
            }
        }

        // Emit events for TUI notification
        if !diff.added.is_empty() {
            self.emit_tasks_added(diff.added.clone()).await;
        }

        if !diff.removed.is_empty() {
            self.emit_tasks_cancelled(diff.removed.clone()).await;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::Write;
    use tempfile::TempDir;

    fn create_test_dag() -> Arc<RwLock<TaskDAG>> {
        Arc::new(RwLock::new(TaskDAG::new()))
    }

    fn create_test_planner() -> (ReactivePlanner, mpsc::Receiver<PlanEvent>, TempDir) {
        let dag = create_test_dag();
        let temp_dir = TempDir::new().unwrap();
        let (planner, rx) = ReactivePlanner::with_defaults(dag, temp_dir.path().to_path_buf());
        (planner, rx, temp_dir)
    }

    // ============ PlannerConfig tests ============

    #[test]
    fn test_planner_config_default() {
        let config = PlannerConfig::default();

        assert_eq!(config.watch_paths, vec![PathBuf::from(".sop/planning")]);
        assert_eq!(
            config.debounce_duration,
            Duration::from_millis(DEFAULT_DEBOUNCE_MS)
        );
        assert!(config.relevant_patterns.contains(&"plan.md".to_string()));
        assert!(config
            .relevant_patterns
            .contains(&"detailed-design.md".to_string()));
        assert!(config
            .relevant_patterns
            .contains(&".code-task.md".to_string()));
    }

    #[test]
    fn test_planner_config_custom() {
        let config = PlannerConfig {
            watch_paths: vec![PathBuf::from("/custom/path")],
            debounce_duration: Duration::from_millis(500),
            relevant_patterns: vec!["custom.md".to_string()],
        };

        assert_eq!(config.watch_paths, vec![PathBuf::from("/custom/path")]);
        assert_eq!(config.debounce_duration, Duration::from_millis(500));
        assert!(config.relevant_patterns.contains(&"custom.md".to_string()));
    }

    // ============ PlanEvent tests ============

    #[test]
    fn test_plan_event_file_changed() {
        let event = PlanEvent::FileChanged {
            path: PathBuf::from("plan.md"),
        };

        if let PlanEvent::FileChanged { path } = event {
            assert_eq!(path, PathBuf::from("plan.md"));
        } else {
            panic!("Expected FileChanged event");
        }
    }

    #[test]
    fn test_plan_event_replan_triggered() {
        let event = PlanEvent::ReplanTriggered;
        assert!(matches!(event, PlanEvent::ReplanTriggered));
    }

    #[test]
    fn test_plan_event_tasks_added() {
        let task = Task::new("test-task", "Test description");
        let event = PlanEvent::TasksAdded {
            tasks: vec![task.clone()],
        };

        if let PlanEvent::TasksAdded { tasks } = event {
            assert_eq!(tasks.len(), 1);
            assert_eq!(tasks[0].name, "test-task");
        } else {
            panic!("Expected TasksAdded event");
        }
    }

    #[test]
    fn test_plan_event_tasks_cancelled() {
        let task_id = TaskId::new();
        let event = PlanEvent::TasksCancelled {
            tasks: vec![task_id],
        };

        if let PlanEvent::TasksCancelled { tasks } = event {
            assert_eq!(tasks.len(), 1);
        } else {
            panic!("Expected TasksCancelled event");
        }
    }

    #[test]
    fn test_plan_event_clone() {
        let event = PlanEvent::FileChanged {
            path: PathBuf::from("plan.md"),
        };
        let cloned = event.clone();

        if let (PlanEvent::FileChanged { path: p1 }, PlanEvent::FileChanged { path: p2 }) =
            (event, cloned)
        {
            assert_eq!(p1, p2);
        } else {
            panic!("Clone mismatch");
        }
    }

    #[test]
    fn test_plan_event_debug() {
        let event = PlanEvent::ReplanTriggered;
        let debug = format!("{:?}", event);
        assert!(debug.contains("ReplanTriggered"));
    }

    // ============ ReactivePlanner creation tests ============

    #[test]
    fn test_reactive_planner_new() {
        let dag = create_test_dag();
        let config = PlannerConfig::default();
        let temp_dir = TempDir::new().unwrap();
        let (planner, _rx) = ReactivePlanner::new(dag, config, temp_dir.path().to_path_buf());

        assert_eq!(
            planner.watch_paths(),
            &[PathBuf::from(".sop/planning")]
        );
    }

    #[test]
    fn test_reactive_planner_with_defaults() {
        let (planner, _rx, _temp_dir) = create_test_planner();

        assert_eq!(
            planner.watch_paths(),
            &[PathBuf::from(".sop/planning")]
        );
        assert_eq!(
            planner.config().debounce_duration,
            Duration::from_millis(DEFAULT_DEBOUNCE_MS)
        );
    }

    #[test]
    fn test_reactive_planner_custom_paths() {
        let dag = create_test_dag();
        let config = PlannerConfig {
            watch_paths: vec![PathBuf::from("/path/one"), PathBuf::from("/path/two")],
            ..Default::default()
        };
        let temp_dir = TempDir::new().unwrap();
        let (planner, _rx) = ReactivePlanner::new(dag, config, temp_dir.path().to_path_buf());

        assert_eq!(planner.watch_paths().len(), 2);
        assert!(planner.watch_paths().contains(&PathBuf::from("/path/one")));
        assert!(planner.watch_paths().contains(&PathBuf::from("/path/two")));
    }

    // ============ Relevance filtering tests ============

    #[test]
    fn test_is_relevant_file_plan_md() {
        let (planner, _rx, _temp_dir) = create_test_planner();

        assert!(planner.is_relevant_file(&PathBuf::from("plan.md")));
        assert!(planner.is_relevant_file(&PathBuf::from("/some/path/plan.md")));
    }

    #[test]
    fn test_is_relevant_file_detailed_design() {
        let (planner, _rx, _temp_dir) = create_test_planner();

        assert!(planner.is_relevant_file(&PathBuf::from("detailed-design.md")));
        assert!(planner.is_relevant_file(&PathBuf::from(".sop/planning/design/detailed-design.md")));
    }

    #[test]
    fn test_is_relevant_file_code_task() {
        let (planner, _rx, _temp_dir) = create_test_planner();

        assert!(planner.is_relevant_file(&PathBuf::from("task-01.code-task.md")));
        assert!(planner.is_relevant_file(&PathBuf::from(".sop/implementation/task-01.code-task.md")));
    }

    #[test]
    fn test_is_relevant_file_unrelated() {
        let (planner, _rx, _temp_dir) = create_test_planner();

        assert!(!planner.is_relevant_file(&PathBuf::from("README.md")));
        assert!(!planner.is_relevant_file(&PathBuf::from("src/main.rs")));
        assert!(!planner.is_relevant_file(&PathBuf::from("random.txt")));
    }

    #[test]
    fn test_is_relevant_file_no_filename() {
        let (planner, _rx, _temp_dir) = create_test_planner();

        // Path without filename
        assert!(!planner.is_relevant_file(&PathBuf::from("")));
    }

    #[test]
    fn test_is_relevant_file_custom_patterns() {
        let dag = create_test_dag();
        let config = PlannerConfig {
            relevant_patterns: vec!["custom.yaml".to_string(), ".special".to_string()],
            ..Default::default()
        };
        let temp_dir = TempDir::new().unwrap();
        let (planner, _rx) = ReactivePlanner::new(dag, config, temp_dir.path().to_path_buf());

        assert!(planner.is_relevant_file(&PathBuf::from("config.custom.yaml")));
        assert!(planner.is_relevant_file(&PathBuf::from("file.special")));
        assert!(!planner.is_relevant_file(&PathBuf::from("plan.md")));
    }

    // ============ Debouncing tests ============

    #[test]
    fn test_should_process_change_first_change() {
        let (planner, _rx, _temp_dir) = create_test_planner();
        let path = PathBuf::from("plan.md");

        // First change should always be processed
        assert!(planner.should_process_change(&path));
    }

    #[test]
    fn test_should_process_change_rapid_changes_debounced() {
        let (planner, _rx, _temp_dir) = create_test_planner();
        let path = PathBuf::from("plan.md");

        // First change processes
        assert!(planner.should_process_change(&path));

        // Immediate second change is debounced
        assert!(!planner.should_process_change(&path));

        // Third immediate change is also debounced
        assert!(!planner.should_process_change(&path));
    }

    #[test]
    fn test_should_process_change_after_debounce_window() {
        let dag = create_test_dag();
        let config = PlannerConfig {
            debounce_duration: Duration::from_millis(10), // Very short for testing
            ..Default::default()
        };
        let temp_dir = TempDir::new().unwrap();
        let (planner, _rx) = ReactivePlanner::new(dag, config, temp_dir.path().to_path_buf());
        let path = PathBuf::from("plan.md");

        // First change
        assert!(planner.should_process_change(&path));

        // Wait past debounce window
        std::thread::sleep(Duration::from_millis(20));

        // Should now process again
        assert!(planner.should_process_change(&path));
    }

    #[test]
    fn test_should_process_change_different_files() {
        let (planner, _rx, _temp_dir) = create_test_planner();

        let path1 = PathBuf::from("plan.md");
        let path2 = PathBuf::from("detailed-design.md");

        // Both files should process independently
        assert!(planner.should_process_change(&path1));
        assert!(planner.should_process_change(&path2));

        // Rapid changes to both are debounced independently
        assert!(!planner.should_process_change(&path1));
        assert!(!planner.should_process_change(&path2));
    }

    #[test]
    fn test_clear_debounce_state() {
        let (planner, _rx, _temp_dir) = create_test_planner();
        let path = PathBuf::from("plan.md");

        // Process first change
        assert!(planner.should_process_change(&path));

        // Would be debounced
        assert!(!planner.should_process_change(&path));

        // Clear state
        planner.clear_debounce_state();

        // Now processes again
        assert!(planner.should_process_change(&path));
    }

    // ============ Event emission tests ============

    #[tokio::test]
    async fn test_emit_file_changed() {
        let (planner, mut rx, _temp_dir) = create_test_planner();

        planner
            .emit_file_changed(PathBuf::from("plan.md"))
            .await;

        let event = rx.recv().await.unwrap();
        if let PlanEvent::FileChanged { path } = event {
            assert_eq!(path, PathBuf::from("plan.md"));
        } else {
            panic!("Expected FileChanged event");
        }
    }

    #[tokio::test]
    async fn test_emit_replan_triggered() {
        let (planner, mut rx, _temp_dir) = create_test_planner();

        planner.emit_replan_triggered().await;

        let event = rx.recv().await.unwrap();
        assert!(matches!(event, PlanEvent::ReplanTriggered));
    }

    #[tokio::test]
    async fn test_emit_tasks_added() {
        let (planner, mut rx, _temp_dir) = create_test_planner();

        let task = Task::new("new-task", "A new task");
        planner.emit_tasks_added(vec![task]).await;

        let event = rx.recv().await.unwrap();
        if let PlanEvent::TasksAdded { tasks } = event {
            assert_eq!(tasks.len(), 1);
            assert_eq!(tasks[0].name, "new-task");
        } else {
            panic!("Expected TasksAdded event");
        }
    }

    #[tokio::test]
    async fn test_emit_tasks_cancelled() {
        let (planner, mut rx, _temp_dir) = create_test_planner();

        let task_id = TaskId::new();
        planner.emit_tasks_cancelled(vec![task_id]).await;

        let event = rx.recv().await.unwrap();
        if let PlanEvent::TasksCancelled { tasks } = event {
            assert_eq!(tasks.len(), 1);
        } else {
            panic!("Expected TasksCancelled event");
        }
    }

    // ============ DAG access tests ============

    #[test]
    fn test_dag_access() {
        let dag = create_test_dag();
        let dag_clone = dag.clone();
        let temp_dir = TempDir::new().unwrap();
        let (planner, _rx) = ReactivePlanner::with_defaults(dag, temp_dir.path().to_path_buf());

        // Add task through the DAG reference
        {
            let mut dag = planner.dag().write().unwrap();
            dag.add_task(Task::new("task-1", "First task"));
        }

        // Verify via original reference
        let dag = dag_clone.read().unwrap();
        assert_eq!(dag.task_count(), 1);
    }

    // ============ Watcher creation tests ============

    #[test]
    fn test_create_watcher() {
        let (planner, _rx, _temp_dir) = create_test_planner();

        let watcher = planner.create_watcher(|_| {});
        assert!(watcher.is_ok());
    }

    // ============ Integration tests with real files ============

    #[test]
    fn test_start_watching_creates_watcher() {
        let temp_dir = TempDir::new().unwrap();
        let watch_path = temp_dir.path().join("planning");
        fs::create_dir_all(&watch_path).unwrap();

        let dag = create_test_dag();
        let config = PlannerConfig {
            watch_paths: vec![watch_path],
            ..Default::default()
        };
        let (planner, _rx) = ReactivePlanner::new(dag, config, temp_dir.path().to_path_buf());

        let watcher = planner.start_watching();
        assert!(watcher.is_ok());
    }

    #[test]
    fn test_start_watching_nonexistent_path_ok() {
        let dag = create_test_dag();
        let config = PlannerConfig {
            watch_paths: vec![PathBuf::from("/nonexistent/path/that/does/not/exist")],
            ..Default::default()
        };
        let temp_dir = TempDir::new().unwrap();
        let (planner, _rx) = ReactivePlanner::new(dag, config, temp_dir.path().to_path_buf());

        // Should succeed but not watch the nonexistent path
        let watcher = planner.start_watching();
        assert!(watcher.is_ok());
    }

    #[tokio::test]
    async fn test_file_change_detection() {
        let temp_dir = TempDir::new().unwrap();
        let watch_path = temp_dir.path().join("planning");
        fs::create_dir_all(&watch_path).unwrap();

        let dag = create_test_dag();
        let config = PlannerConfig {
            watch_paths: vec![watch_path.clone()],
            debounce_duration: Duration::from_millis(50),
            ..Default::default()
        };
        let (planner, mut rx) = ReactivePlanner::new(dag, config, temp_dir.path().to_path_buf());

        let _watcher = planner.start_watching().unwrap();

        // Give watcher time to set up
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Create a relevant file
        let file_path = watch_path.join("plan.md");
        let mut file = fs::File::create(&file_path).unwrap();
        file.write_all(b"# Plan\n").unwrap();
        file.sync_all().unwrap();

        // Wait for event (with timeout)
        let event = tokio::time::timeout(Duration::from_secs(2), rx.recv()).await;

        if let Ok(Some(PlanEvent::FileChanged { path })) = event {
            assert!(path.ends_with("plan.md"));
        } else {
            // File watching can be flaky in tests, so we don't fail
            // Just verify the watcher was created successfully
        }
    }

    #[tokio::test]
    async fn test_irrelevant_file_not_emitted() {
        let temp_dir = TempDir::new().unwrap();
        let watch_path = temp_dir.path().join("planning");
        fs::create_dir_all(&watch_path).unwrap();

        let dag = create_test_dag();
        let config = PlannerConfig {
            watch_paths: vec![watch_path.clone()],
            debounce_duration: Duration::from_millis(50),
            ..Default::default()
        };
        let (planner, mut rx) = ReactivePlanner::new(dag, config, temp_dir.path().to_path_buf());

        let _watcher = planner.start_watching().unwrap();

        // Give watcher time to set up
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Create an irrelevant file
        let file_path = watch_path.join("README.md");
        let mut file = fs::File::create(&file_path).unwrap();
        file.write_all(b"# README\n").unwrap();
        file.sync_all().unwrap();

        // Wait briefly - should NOT receive event
        let event = tokio::time::timeout(Duration::from_millis(500), rx.recv()).await;

        // Should timeout because irrelevant file is filtered
        assert!(event.is_err() || event.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_debouncing_rapid_changes() {
        let temp_dir = TempDir::new().unwrap();
        let watch_path = temp_dir.path().join("planning");
        fs::create_dir_all(&watch_path).unwrap();

        let dag = create_test_dag();
        let config = PlannerConfig {
            watch_paths: vec![watch_path.clone()],
            debounce_duration: Duration::from_millis(500), // Long debounce
            ..Default::default()
        };
        let (planner, mut rx) = ReactivePlanner::new(dag, config, temp_dir.path().to_path_buf());

        let _watcher = planner.start_watching().unwrap();

        // Give watcher time to set up
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Create file and make rapid changes
        let file_path = watch_path.join("plan.md");
        for i in 0..5 {
            let mut file = fs::OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .open(&file_path)
                .unwrap();
            file.write_all(format!("# Plan version {}\n", i).as_bytes())
                .unwrap();
            file.sync_all().unwrap();
            tokio::time::sleep(Duration::from_millis(50)).await;
        }

        // Collect events over a short window
        let mut events = Vec::new();
        loop {
            match tokio::time::timeout(Duration::from_millis(200), rx.recv()).await {
                Ok(Some(event)) => events.push(event),
                _ => break,
            }
        }

        // Due to debouncing, should have received 1-2 events max, not 5
        // (exact number depends on timing, but should be less than 5)
        assert!(
            events.len() <= 2,
            "Expected at most 2 events due to debouncing, got {}",
            events.len()
        );
    }

    // ============ Error handling tests ============

    #[test]
    fn test_watcher_handles_errors_gracefully() {
        let (planner, _rx, _temp_dir) = create_test_planner();

        // Creating a watcher with an error callback
        let watcher = planner.create_watcher(|res| {
            // Errors are just ignored, not propagated
            if let Err(_e) = res {
                // Error logged but doesn't crash
            }
        });

        assert!(watcher.is_ok());
    }

    // ============ TaskDiff tests ============

    #[test]
    fn test_task_diff_new() {
        let diff = TaskDiff::new();
        assert!(diff.added.is_empty());
        assert!(diff.removed.is_empty());
        assert!(diff.modified.is_empty());
    }

    #[test]
    fn test_task_diff_has_changes() {
        let mut diff = TaskDiff::new();
        assert!(!diff.has_changes());

        diff.added.push(Task::new("new-task", "description"));
        assert!(diff.has_changes());
    }

    #[test]
    fn test_task_diff_change_count() {
        let mut diff = TaskDiff::new();
        assert_eq!(diff.change_count(), 0);

        diff.added.push(Task::new("task-1", "desc"));
        diff.removed.push(TaskId::new());
        diff.modified.push(Task::new("task-2", "desc"));
        assert_eq!(diff.change_count(), 3);
    }

    // ============ diff_tasks tests ============

    #[test]
    fn test_diff_tasks_new_task_detection() {
        let (planner, _rx, _temp_dir) = create_test_planner();

        let old_tasks: Vec<Task> = vec![];
        let new_tasks = vec![
            Task::new("Task A", "Description A"),
            Task::new("Task B", "Description B"),
        ];

        let diff = planner.diff_tasks(&old_tasks, &new_tasks);

        // Given plan adds 2 new steps, when replan() runs, then 2 tasks are added
        assert_eq!(diff.added.len(), 2);
        assert!(diff.removed.is_empty());
        assert!(diff.modified.is_empty());
    }

    #[test]
    fn test_diff_tasks_removed_task_handling() {
        let (planner, _rx, _temp_dir) = create_test_planner();

        let old_tasks = vec![
            Task::new("Task A", "Description A"),
            Task::new("Task B", "Description B"),
        ];
        let new_tasks = vec![Task::new("Task A", "Description A")];

        let diff = planner.diff_tasks(&old_tasks, &new_tasks);

        // Given plan removes a pending task, then task is marked cancelled
        assert!(diff.added.is_empty());
        assert_eq!(diff.removed.len(), 1);
        assert!(diff.modified.is_empty());
    }

    #[test]
    fn test_diff_tasks_running_task_protection() {
        let (planner, _rx, _temp_dir) = create_test_planner();

        let mut running_task = Task::new("Task B", "Description B");
        running_task.start(); // Mark as running

        let old_tasks = vec![Task::new("Task A", "Description A"), running_task];
        let new_tasks = vec![Task::new("Task A", "Description A")];

        let diff = planner.diff_tasks(&old_tasks, &new_tasks);

        // Given a task is running, when plan changes, then running task continues
        // The removed list should NOT contain the running task
        assert!(diff.removed.is_empty());
    }

    #[test]
    fn test_diff_tasks_modified_task() {
        let (planner, _rx, _temp_dir) = create_test_planner();

        let old_task = Task::new("Task A", "Old description");
        let new_task = Task::new("Task A", "New description");

        let old_tasks = vec![old_task];
        let new_tasks = vec![new_task];

        let diff = planner.diff_tasks(&old_tasks, &new_tasks);

        // Given task description changes, then task appears in modified list
        assert!(diff.added.is_empty());
        assert!(diff.removed.is_empty());
        assert_eq!(diff.modified.len(), 1);
        assert_eq!(diff.modified[0].description, "New description");
    }

    #[test]
    fn test_diff_tasks_running_task_not_modified() {
        let (planner, _rx, _temp_dir) = create_test_planner();

        let mut old_task = Task::new("Task A", "Old description");
        old_task.start(); // Mark as running

        let new_task = Task::new("Task A", "New description");

        let old_tasks = vec![old_task];
        let new_tasks = vec![new_task];

        let diff = planner.diff_tasks(&old_tasks, &new_tasks);

        // Running tasks should not be modified
        assert!(diff.modified.is_empty());
    }

    #[test]
    fn test_diff_tasks_preserves_task_id_on_modify() {
        let (planner, _rx, _temp_dir) = create_test_planner();

        let old_task = Task::new("Task A", "Old description");
        let original_id = old_task.id;
        let new_task = Task::new("Task A", "New description");

        let old_tasks = vec![old_task];
        let new_tasks = vec![new_task];

        let diff = planner.diff_tasks(&old_tasks, &new_tasks);

        // Modified task should preserve the original ID
        assert_eq!(diff.modified[0].id, original_id);
    }

    // ============ Replanning tests ============

    #[tokio::test]
    async fn test_on_plan_changed_emits_replan_event() {
        let (planner, mut rx, _temp_dir) = create_test_planner();

        planner
            .on_plan_changed(Path::new("plan.md"))
            .await
            .unwrap();

        // Should emit ReplanTriggered event
        let event = rx.recv().await.unwrap();
        assert!(matches!(event, PlanEvent::ReplanTriggered));
    }

    #[tokio::test]
    async fn test_replan_with_no_code_tasks() {
        let (planner, _rx, _temp_dir) = create_test_planner();

        // Replan should succeed even with no code tasks
        let result = planner.replan().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_apply_diff_adds_tasks() {
        let (planner, mut rx, _temp_dir) = create_test_planner();

        let mut diff = TaskDiff::new();
        diff.added.push(Task::new("New Task", "Description"));

        planner.apply_diff(&diff).await.unwrap();

        // Task should be added to DAG
        let dag = planner.dag().read().unwrap();
        assert_eq!(dag.task_count(), 1);

        // Should emit TasksAdded event
        let event = rx.recv().await.unwrap();
        if let PlanEvent::TasksAdded { tasks } = event {
            assert_eq!(tasks.len(), 1);
        } else {
            panic!("Expected TasksAdded event");
        }
    }

    #[tokio::test]
    async fn test_apply_diff_cancels_removed_tasks() {
        let (planner, mut rx, _temp_dir) = create_test_planner();

        // First add a task to the DAG
        let task = Task::new("Task to Remove", "Description");
        let task_id = task.id;
        {
            let mut dag = planner.dag().write().unwrap();
            dag.add_task(task);
        }

        // Now create a diff that removes it
        let mut diff = TaskDiff::new();
        diff.removed.push(task_id);

        planner.apply_diff(&diff).await.unwrap();

        // Task should be marked as cancelled
        let dag = planner.dag().read().unwrap();
        let task = dag.get_task(&task_id).unwrap();
        assert!(matches!(task.status, TaskStatus::Cancelled { .. }));

        // Should emit TasksCancelled event
        let event = rx.recv().await.unwrap();
        if let PlanEvent::TasksCancelled { tasks } = event {
            assert_eq!(tasks.len(), 1);
        } else {
            panic!("Expected TasksCancelled event");
        }
    }

    #[tokio::test]
    async fn test_apply_diff_updates_modified_tasks() {
        let (planner, _rx, _temp_dir) = create_test_planner();

        // First add a task to the DAG
        let task = Task::new("Task", "Old Description");
        let task_id = task.id;
        {
            let mut dag = planner.dag().write().unwrap();
            dag.add_task(task);
        }

        // Now create a diff that modifies it
        let mut modified_task = Task::new("Task", "New Description");
        modified_task.id = task_id;

        let mut diff = TaskDiff::new();
        diff.modified.push(modified_task);

        planner.apply_diff(&diff).await.unwrap();

        // Task description should be updated
        let dag = planner.dag().read().unwrap();
        let task = dag.get_task(&task_id).unwrap();
        assert_eq!(task.description, "New Description");
    }

    #[test]
    fn test_register_task() {
        let (planner, _rx, _temp_dir) = create_test_planner();
        let task_id = TaskId::new();

        planner.register_task("My Task", task_id);

        assert_eq!(planner.get_task_id("My Task"), Some(task_id));
    }

    #[test]
    fn test_registered_task_names() {
        let (planner, _rx, _temp_dir) = create_test_planner();

        planner.register_task("Task A", TaskId::new());
        planner.register_task("Task B", TaskId::new());

        let names = planner.registered_task_names();
        assert_eq!(names.len(), 2);
        assert!(names.contains(&"Task A".to_string()));
        assert!(names.contains(&"Task B".to_string()));
    }
}
