use crate::agent::{AgentId, AgentStatus};
use crate::core::task::TaskStatus;
use crate::session::{SessionId, SessionStatus};
use crate::tea::{Mode, Notification};
use crate::workflow::{WorkflowId, WorkflowPhase, WorkflowStatus};
use chrono::{DateTime, Utc};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct SessionView {
    pub id: SessionId,
    pub name: String,
    pub project: String,
    pub branch: String,
    pub base_branch: String,
    pub base_commit: String,
    pub agent: String,
    pub status: SessionStatus,
    pub last_active: DateTime<Utc>,
    pub is_active: Option<bool>,
}

/// View struct for workflow display in TUI.
///
/// Provides a snapshot of workflow state for rendering, following the
/// same pattern as SessionView.
#[derive(Debug, Clone)]
pub struct WorkflowView {
    /// Unique workflow identifier.
    pub id: WorkflowId,
    /// Human-readable workflow name.
    pub name: String,
    /// Current workflow phase.
    pub phase: WorkflowPhase,
    /// Current workflow status.
    pub status: WorkflowStatus,
    /// Progress as (completed_phases, total_phases).
    pub phase_progress: (usize, usize),
}

impl WorkflowView {
    /// Total number of workflow phases (excluding Complete).
    pub const TOTAL_PHASES: usize = 5;

    /// Create a new WorkflowView from workflow data.
    pub fn new(
        id: WorkflowId,
        name: String,
        phase: WorkflowPhase,
        status: WorkflowStatus,
    ) -> Self {
        let completed = Self::completed_phases(&phase);
        Self {
            id,
            name,
            phase,
            status,
            phase_progress: (completed, Self::TOTAL_PHASES),
        }
    }

    /// Calculate number of completed phases based on current phase.
    fn completed_phases(phase: &WorkflowPhase) -> usize {
        match phase {
            WorkflowPhase::Planning => 0,
            WorkflowPhase::TaskGeneration => 1,
            WorkflowPhase::Implementation => 2,
            WorkflowPhase::Merging => 3,
            WorkflowPhase::Documentation => 4,
            WorkflowPhase::Complete => 5,
        }
    }

    /// Calculate progress percentage (0-100).
    pub fn progress_percentage(&self) -> u16 {
        if self.phase_progress.1 == 0 {
            return 0;
        }
        ((self.phase_progress.0 * 100) / self.phase_progress.1) as u16
    }

    /// Get all phase names in order.
    pub fn phase_names() -> [&'static str; 5] {
        ["Planning", "TaskGen", "Impl", "Merge", "Docs"]
    }

    /// Get the index of the current phase (0-based).
    pub fn current_phase_index(&self) -> usize {
        Self::completed_phases(&self.phase)
    }
}

/// View struct for agent display in TUI grid.
///
/// Provides a snapshot of agent state for rendering in the multi-agent grid.
/// Each agent shows its status, current task, elapsed time, and output preview.
#[derive(Debug, Clone)]
pub struct AgentView {
    /// Unique agent identifier.
    pub id: AgentId,
    /// Name of the task the agent is working on.
    pub task_name: String,
    /// Current agent status.
    pub status: AgentStatus,
    /// How long the agent has been running.
    pub elapsed: Duration,
    /// Last few lines of output for preview.
    pub output_preview: String,
}

impl AgentView {
    /// Create a new AgentView.
    pub fn new(
        id: AgentId,
        task_name: String,
        status: AgentStatus,
        elapsed: Duration,
        output_preview: String,
    ) -> Self {
        Self {
            id,
            task_name,
            status,
            elapsed,
            output_preview,
        }
    }

    /// Get a short label for the agent status.
    pub fn status_label(&self) -> &'static str {
        match &self.status {
            AgentStatus::Idle => "Idle",
            AgentStatus::Running { .. } => "Running",
            AgentStatus::Stuck { .. } => "Stuck",
            AgentStatus::Failed { .. } => "Failed",
            AgentStatus::Terminated => "Done",
        }
    }

    /// Format elapsed duration as human-readable string (e.g., "2m 30s").
    pub fn format_elapsed(&self) -> String {
        let total_secs = self.elapsed.as_secs();
        let hours = total_secs / 3600;
        let minutes = (total_secs % 3600) / 60;
        let seconds = total_secs % 60;

        if hours > 0 {
            format!("{}h {}m", hours, minutes)
        } else if minutes > 0 {
            format!("{}m {}s", minutes, seconds)
        } else {
            format!("{}s", seconds)
        }
    }

    /// Get last N lines of output preview.
    pub fn output_lines(&self, max_lines: usize) -> Vec<&str> {
        self.output_preview
            .lines()
            .rev()
            .take(max_lines)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect()
    }
}

/// View struct for task display in DAG visualization.
///
/// Represents a single task node in the DAG view with name and status
/// for rendering ASCII boxes with appropriate colors.
#[derive(Debug, Clone)]
pub struct TaskView {
    /// Task name for display in the box.
    pub name: String,
    /// Current task status for color coding.
    pub status: TaskStatus,
}

impl TaskView {
    /// Create a new TaskView.
    pub fn new(name: String, status: TaskStatus) -> Self {
        Self { name, status }
    }

    /// Get a short label for the task status.
    pub fn status_label(&self) -> &'static str {
        match &self.status {
            TaskStatus::Pending => "pending",
            TaskStatus::Ready => "ready",
            TaskStatus::Running => "running",
            TaskStatus::Completed => "done",
            TaskStatus::Failed { .. } => "failed",
            TaskStatus::Blocked { .. } => "blocked",
            TaskStatus::Cancelled { .. } => "cancelled",
        }
    }
}

/// View struct for DAG visualization in TUI.
///
/// Contains the task nodes and their dependency edges for rendering
/// an ASCII-based dependency graph.
#[derive(Debug, Clone)]
pub struct TaskDAGView {
    /// Tasks to display as boxes.
    pub tasks: Vec<TaskView>,
    /// Dependency edges as (from_idx, to_idx) pairs.
    /// from_idx must complete before to_idx can start.
    pub edges: Vec<(usize, usize)>,
}

impl TaskDAGView {
    /// Create a new empty TaskDAGView.
    pub fn new() -> Self {
        Self {
            tasks: Vec::new(),
            edges: Vec::new(),
        }
    }

    /// Add a task to the DAG view.
    pub fn add_task(&mut self, task: TaskView) -> usize {
        let idx = self.tasks.len();
        self.tasks.push(task);
        idx
    }

    /// Add a dependency edge (from_idx -> to_idx).
    pub fn add_edge(&mut self, from_idx: usize, to_idx: usize) {
        self.edges.push((from_idx, to_idx));
    }

    /// Get incoming edges for a task (dependencies).
    pub fn incoming_edges(&self, task_idx: usize) -> Vec<usize> {
        self.edges
            .iter()
            .filter(|(_, to)| *to == task_idx)
            .map(|(from, _)| *from)
            .collect()
    }

    /// Get outgoing edges from a task (dependents).
    pub fn outgoing_edges(&self, task_idx: usize) -> Vec<usize> {
        self.edges
            .iter()
            .filter(|(from, _)| *from == task_idx)
            .map(|(_, to)| *to)
            .collect()
    }

    /// Compute topological layers for layout.
    /// Returns tasks grouped by layer (tasks with no deps in layer 0, etc).
    pub fn compute_layers(&self) -> Vec<Vec<usize>> {
        let n = self.tasks.len();
        if n == 0 {
            return Vec::new();
        }

        // Calculate in-degree for each task
        let mut in_degree: Vec<usize> = vec![0; n];
        for (_, to) in &self.edges {
            in_degree[*to] += 1;
        }

        let mut layers: Vec<Vec<usize>> = Vec::new();
        let mut remaining: Vec<bool> = vec![true; n];
        let mut processed = 0;

        while processed < n {
            // Find all tasks with in_degree 0 among remaining
            let mut layer: Vec<usize> = Vec::new();
            for (i, &r) in remaining.iter().enumerate() {
                if r && in_degree[i] == 0 {
                    layer.push(i);
                }
            }

            if layer.is_empty() {
                // Cycle detected or empty - add remaining tasks
                for (i, &r) in remaining.iter().enumerate() {
                    if r {
                        layer.push(i);
                    }
                }
            }

            // Remove these tasks and update in_degrees
            for &idx in &layer {
                remaining[idx] = false;
                processed += 1;
                for to in self.outgoing_edges(idx) {
                    if in_degree[to] > 0 {
                        in_degree[to] -= 1;
                    }
                }
            }

            layers.push(layer);
        }

        layers
    }
}

impl Default for TaskDAGView {
    fn default() -> Self {
        Self::new()
    }
}

static VERSION_COUNTER: AtomicU64 = AtomicU64::new(0);

pub fn next_version() -> u64 {
    VERSION_COUNTER.fetch_add(1, Ordering::Relaxed)
}

#[derive(Debug, Clone)]
pub struct RenderState {
    pub version: u64,
    pub sessions: Vec<SessionView>,
    pub selected: usize,
    pub mode: Mode,
    pub preview: Option<String>,
    pub input_buffer: String,
    pub notification: Option<Notification>,
    /// Whether the keymap legend is expanded (toggled by '?')
    pub show_keymap: bool,
    /// Trust mode indicator - shown in UI when enabled
    pub trust_enabled: bool,
    /// Active workflow view for display (None if no workflow running).
    pub workflow: Option<WorkflowView>,
    /// Active agents for grid display during parallel execution.
    pub agents: Vec<AgentView>,
    /// Currently selected agent in the grid (for keyboard navigation).
    pub selected_agent: usize,
    /// Task DAG view for dependency visualization.
    pub dag: Option<TaskDAGView>,
    /// Whether to show the DAG visualization (toggled by 'd').
    pub show_dag: bool,
}

impl Default for RenderState {
    fn default() -> Self {
        Self {
            version: 0,
            sessions: Vec::new(),
            selected: 0,
            mode: Mode::List,
            preview: None,
            input_buffer: String::new(),
            notification: None,
            show_keymap: false,
            trust_enabled: false,
            workflow: None,
            agents: Vec::new(),
            selected_agent: 0,
            dag: None,
            show_dag: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_counter_increments() {
        let v1 = next_version();
        let v2 = next_version();
        let v3 = next_version();
        assert!(v2 > v1, "Version should increment");
        assert!(v3 > v2, "Version should increment monotonically");
    }

    #[test]
    fn test_render_state_default_version() {
        let state = RenderState::default();
        assert_eq!(state.version, 0);
    }

    #[test]
    fn test_render_state_default_workflow_is_none() {
        let state = RenderState::default();
        assert!(state.workflow.is_none());
    }

    // WorkflowView tests

    #[test]
    fn test_workflow_view_new_planning_phase() {
        let view = WorkflowView::new(
            WorkflowId::new(),
            "build-auth".to_string(),
            WorkflowPhase::Planning,
            WorkflowStatus::Running,
        );
        assert_eq!(view.name, "build-auth");
        assert_eq!(view.phase, WorkflowPhase::Planning);
        assert_eq!(view.status, WorkflowStatus::Running);
        assert_eq!(view.phase_progress, (0, 5));
    }

    #[test]
    fn test_workflow_view_new_implementation_phase() {
        let view = WorkflowView::new(
            WorkflowId::new(),
            "build-auth".to_string(),
            WorkflowPhase::Implementation,
            WorkflowStatus::Running,
        );
        assert_eq!(view.phase_progress, (2, 5));
    }

    #[test]
    fn test_workflow_view_new_complete_phase() {
        let view = WorkflowView::new(
            WorkflowId::new(),
            "build-auth".to_string(),
            WorkflowPhase::Complete,
            WorkflowStatus::Completed,
        );
        assert_eq!(view.phase_progress, (5, 5));
    }

    #[test]
    fn test_workflow_view_progress_percentage_zero() {
        let view = WorkflowView::new(
            WorkflowId::new(),
            "test".to_string(),
            WorkflowPhase::Planning,
            WorkflowStatus::Running,
        );
        assert_eq!(view.progress_percentage(), 0);
    }

    #[test]
    fn test_workflow_view_progress_percentage_60() {
        // 3 of 5 phases = 60%
        let view = WorkflowView::new(
            WorkflowId::new(),
            "test".to_string(),
            WorkflowPhase::Merging,
            WorkflowStatus::Running,
        );
        assert_eq!(view.progress_percentage(), 60);
    }

    #[test]
    fn test_workflow_view_progress_percentage_100() {
        let view = WorkflowView::new(
            WorkflowId::new(),
            "test".to_string(),
            WorkflowPhase::Complete,
            WorkflowStatus::Completed,
        );
        assert_eq!(view.progress_percentage(), 100);
    }

    #[test]
    fn test_workflow_view_phase_names() {
        let names = WorkflowView::phase_names();
        assert_eq!(names.len(), 5);
        assert_eq!(names[0], "Planning");
        assert_eq!(names[1], "TaskGen");
        assert_eq!(names[2], "Impl");
        assert_eq!(names[3], "Merge");
        assert_eq!(names[4], "Docs");
    }

    #[test]
    fn test_workflow_view_current_phase_index() {
        let planning = WorkflowView::new(
            WorkflowId::new(),
            "test".to_string(),
            WorkflowPhase::Planning,
            WorkflowStatus::Running,
        );
        assert_eq!(planning.current_phase_index(), 0);

        let impl_phase = WorkflowView::new(
            WorkflowId::new(),
            "test".to_string(),
            WorkflowPhase::Implementation,
            WorkflowStatus::Running,
        );
        assert_eq!(impl_phase.current_phase_index(), 2);
    }

    #[test]
    fn test_workflow_view_all_phases_progress() {
        let phases = [
            (WorkflowPhase::Planning, 0),
            (WorkflowPhase::TaskGeneration, 1),
            (WorkflowPhase::Implementation, 2),
            (WorkflowPhase::Merging, 3),
            (WorkflowPhase::Documentation, 4),
            (WorkflowPhase::Complete, 5),
        ];

        for (phase, expected_completed) in phases {
            let view = WorkflowView::new(
                WorkflowId::new(),
                "test".to_string(),
                phase,
                WorkflowStatus::Running,
            );
            assert_eq!(
                view.phase_progress.0, expected_completed,
                "Phase {:?} should have {} completed phases",
                phase, expected_completed
            );
        }
    }

    // AgentView tests

    use crate::workflow::TaskId;

    fn create_test_agent_view() -> AgentView {
        AgentView::new(
            AgentId::new(),
            "create-user-model".to_string(),
            AgentStatus::Running { task_id: TaskId::new() },
            Duration::from_secs(150), // 2m 30s
            "Building model...\nRunning tests...\nAll tests passed".to_string(),
        )
    }

    #[test]
    fn test_agent_view_new() {
        let agent = create_test_agent_view();
        assert_eq!(agent.task_name, "create-user-model");
        assert!(matches!(agent.status, AgentStatus::Running { .. }));
        assert_eq!(agent.elapsed.as_secs(), 150);
    }

    #[test]
    fn test_agent_view_status_label_running() {
        let agent = create_test_agent_view();
        assert_eq!(agent.status_label(), "Running");
    }

    #[test]
    fn test_agent_view_status_label_idle() {
        let agent = AgentView::new(
            AgentId::new(),
            "task".to_string(),
            AgentStatus::Idle,
            Duration::from_secs(0),
            String::new(),
        );
        assert_eq!(agent.status_label(), "Idle");
    }

    #[test]
    fn test_agent_view_status_label_stuck() {
        let agent = AgentView::new(
            AgentId::new(),
            "task".to_string(),
            AgentStatus::Stuck {
                since: std::time::Instant::now(),
                reason: "timeout".to_string(),
            },
            Duration::from_secs(0),
            String::new(),
        );
        assert_eq!(agent.status_label(), "Stuck");
    }

    #[test]
    fn test_agent_view_status_label_failed() {
        let agent = AgentView::new(
            AgentId::new(),
            "task".to_string(),
            AgentStatus::Failed { error: "error".to_string() },
            Duration::from_secs(0),
            String::new(),
        );
        assert_eq!(agent.status_label(), "Failed");
    }

    #[test]
    fn test_agent_view_status_label_terminated() {
        let agent = AgentView::new(
            AgentId::new(),
            "task".to_string(),
            AgentStatus::Terminated,
            Duration::from_secs(0),
            String::new(),
        );
        assert_eq!(agent.status_label(), "Done");
    }

    #[test]
    fn test_agent_view_format_elapsed_seconds() {
        let agent = AgentView::new(
            AgentId::new(),
            "task".to_string(),
            AgentStatus::Idle,
            Duration::from_secs(45),
            String::new(),
        );
        assert_eq!(agent.format_elapsed(), "45s");
    }

    #[test]
    fn test_agent_view_format_elapsed_minutes_seconds() {
        // Given agent running for 2 minutes 30 seconds
        let agent = AgentView::new(
            AgentId::new(),
            "task".to_string(),
            AgentStatus::Idle,
            Duration::from_secs(150), // 2m 30s
            String::new(),
        );
        // Then "2m 30s" is displayed
        assert_eq!(agent.format_elapsed(), "2m 30s");
    }

    #[test]
    fn test_agent_view_format_elapsed_hours() {
        let agent = AgentView::new(
            AgentId::new(),
            "task".to_string(),
            AgentStatus::Idle,
            Duration::from_secs(3750), // 1h 2m 30s
            String::new(),
        );
        assert_eq!(agent.format_elapsed(), "1h 2m");
    }

    #[test]
    fn test_agent_view_output_lines() {
        // Given agent with recent output
        let agent = AgentView::new(
            AgentId::new(),
            "task".to_string(),
            AgentStatus::Idle,
            Duration::from_secs(0),
            "Line 1\nLine 2\nLine 3\nLine 4\nLine 5".to_string(),
        );
        // When cell renders
        // Then last 3 lines of output are shown
        let lines = agent.output_lines(3);
        assert_eq!(lines.len(), 3);
        assert_eq!(lines[0], "Line 3");
        assert_eq!(lines[1], "Line 4");
        assert_eq!(lines[2], "Line 5");
    }

    #[test]
    fn test_agent_view_output_lines_empty() {
        let agent = AgentView::new(
            AgentId::new(),
            "task".to_string(),
            AgentStatus::Idle,
            Duration::from_secs(0),
            String::new(),
        );
        let lines = agent.output_lines(3);
        assert!(lines.is_empty());
    }

    #[test]
    fn test_agent_view_output_lines_less_than_max() {
        let agent = AgentView::new(
            AgentId::new(),
            "task".to_string(),
            AgentStatus::Idle,
            Duration::from_secs(0),
            "Only one line".to_string(),
        );
        let lines = agent.output_lines(3);
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0], "Only one line");
    }

    // TaskView and TaskDAGView tests

    #[test]
    fn test_task_view_new() {
        let view = TaskView::new("create-user".to_string(), TaskStatus::Running);
        assert_eq!(view.name, "create-user");
        assert_eq!(view.status, TaskStatus::Running);
    }

    #[test]
    fn test_task_view_status_labels() {
        let test_cases = [
            (TaskStatus::Pending, "pending"),
            (TaskStatus::Ready, "ready"),
            (TaskStatus::Running, "running"),
            (TaskStatus::Completed, "done"),
            (TaskStatus::Failed { error: "err".to_string() }, "failed"),
            (TaskStatus::Blocked { reason: "dep".to_string() }, "blocked"),
            (TaskStatus::Cancelled { reason: "replanned".to_string() }, "cancelled"),
        ];

        for (status, expected_label) in test_cases {
            let view = TaskView::new("task".to_string(), status);
            assert_eq!(view.status_label(), expected_label);
        }
    }

    #[test]
    fn test_task_dag_view_new() {
        let dag = TaskDAGView::new();
        assert!(dag.tasks.is_empty());
        assert!(dag.edges.is_empty());
    }

    #[test]
    fn test_task_dag_view_default() {
        let dag = TaskDAGView::default();
        assert!(dag.tasks.is_empty());
        assert!(dag.edges.is_empty());
    }

    #[test]
    fn test_task_dag_view_add_task() {
        let mut dag = TaskDAGView::new();
        let idx = dag.add_task(TaskView::new("task-a".to_string(), TaskStatus::Pending));
        assert_eq!(idx, 0);
        assert_eq!(dag.tasks.len(), 1);
        assert_eq!(dag.tasks[0].name, "task-a");
    }

    #[test]
    fn test_task_dag_view_add_edge() {
        let mut dag = TaskDAGView::new();
        dag.add_task(TaskView::new("A".to_string(), TaskStatus::Completed));
        dag.add_task(TaskView::new("B".to_string(), TaskStatus::Running));
        dag.add_edge(0, 1);

        assert_eq!(dag.edges.len(), 1);
        assert_eq!(dag.edges[0], (0, 1));
    }

    #[test]
    fn test_task_dag_view_incoming_edges() {
        let mut dag = TaskDAGView::new();
        dag.add_task(TaskView::new("A".to_string(), TaskStatus::Completed));
        dag.add_task(TaskView::new("B".to_string(), TaskStatus::Completed));
        dag.add_task(TaskView::new("C".to_string(), TaskStatus::Running));
        // A -> C and B -> C
        dag.add_edge(0, 2);
        dag.add_edge(1, 2);

        let incoming = dag.incoming_edges(2);
        assert_eq!(incoming.len(), 2);
        assert!(incoming.contains(&0));
        assert!(incoming.contains(&1));

        // A has no incoming edges
        assert!(dag.incoming_edges(0).is_empty());
    }

    #[test]
    fn test_task_dag_view_outgoing_edges() {
        let mut dag = TaskDAGView::new();
        dag.add_task(TaskView::new("A".to_string(), TaskStatus::Completed));
        dag.add_task(TaskView::new("B".to_string(), TaskStatus::Running));
        dag.add_task(TaskView::new("C".to_string(), TaskStatus::Pending));
        // A -> B and A -> C
        dag.add_edge(0, 1);
        dag.add_edge(0, 2);

        let outgoing = dag.outgoing_edges(0);
        assert_eq!(outgoing.len(), 2);
        assert!(outgoing.contains(&1));
        assert!(outgoing.contains(&2));

        // C has no outgoing edges
        assert!(dag.outgoing_edges(2).is_empty());
    }

    #[test]
    fn test_task_dag_view_compute_layers_empty() {
        let dag = TaskDAGView::new();
        let layers = dag.compute_layers();
        assert!(layers.is_empty());
    }

    #[test]
    fn test_task_dag_view_compute_layers_single() {
        let mut dag = TaskDAGView::new();
        dag.add_task(TaskView::new("A".to_string(), TaskStatus::Pending));
        let layers = dag.compute_layers();
        assert_eq!(layers.len(), 1);
        assert_eq!(layers[0], vec![0]);
    }

    #[test]
    fn test_task_dag_view_compute_layers_linear_chain() {
        // A -> B -> C
        let mut dag = TaskDAGView::new();
        dag.add_task(TaskView::new("A".to_string(), TaskStatus::Completed));
        dag.add_task(TaskView::new("B".to_string(), TaskStatus::Running));
        dag.add_task(TaskView::new("C".to_string(), TaskStatus::Pending));
        dag.add_edge(0, 1);
        dag.add_edge(1, 2);

        let layers = dag.compute_layers();
        assert_eq!(layers.len(), 3);
        assert_eq!(layers[0], vec![0]); // A
        assert_eq!(layers[1], vec![1]); // B
        assert_eq!(layers[2], vec![2]); // C
    }

    #[test]
    fn test_task_dag_view_compute_layers_diamond() {
        // A -> B, A -> C, B -> D, C -> D
        let mut dag = TaskDAGView::new();
        dag.add_task(TaskView::new("A".to_string(), TaskStatus::Completed));
        dag.add_task(TaskView::new("B".to_string(), TaskStatus::Running));
        dag.add_task(TaskView::new("C".to_string(), TaskStatus::Running));
        dag.add_task(TaskView::new("D".to_string(), TaskStatus::Pending));
        dag.add_edge(0, 1);
        dag.add_edge(0, 2);
        dag.add_edge(1, 3);
        dag.add_edge(2, 3);

        let layers = dag.compute_layers();
        assert_eq!(layers.len(), 3);
        assert_eq!(layers[0], vec![0]); // A
        assert!(layers[1].contains(&1) && layers[1].contains(&2)); // B, C
        assert_eq!(layers[2], vec![3]); // D
    }

    #[test]
    fn test_task_dag_view_compute_layers_parallel() {
        // A, B, C all independent
        let mut dag = TaskDAGView::new();
        dag.add_task(TaskView::new("A".to_string(), TaskStatus::Running));
        dag.add_task(TaskView::new("B".to_string(), TaskStatus::Running));
        dag.add_task(TaskView::new("C".to_string(), TaskStatus::Running));

        let layers = dag.compute_layers();
        assert_eq!(layers.len(), 1);
        assert_eq!(layers[0].len(), 3);
    }

    #[test]
    fn test_task_dag_view_5_tasks_rendered() {
        // Given 5 tasks in DAG
        let mut dag = TaskDAGView::new();
        dag.add_task(TaskView::new("task-1".to_string(), TaskStatus::Completed));
        dag.add_task(TaskView::new("task-2".to_string(), TaskStatus::Completed));
        dag.add_task(TaskView::new("task-3".to_string(), TaskStatus::Running));
        dag.add_task(TaskView::new("task-4".to_string(), TaskStatus::Pending));
        dag.add_task(TaskView::new("task-5".to_string(), TaskStatus::Pending));

        // When checking
        // Then 5 tasks are available
        assert_eq!(dag.tasks.len(), 5);
    }

    #[test]
    fn test_task_dag_view_dependency_arrow() {
        // Given A->C dependency
        let mut dag = TaskDAGView::new();
        let a_idx = dag.add_task(TaskView::new("A".to_string(), TaskStatus::Completed));
        dag.add_task(TaskView::new("B".to_string(), TaskStatus::Running));
        let c_idx = dag.add_task(TaskView::new("C".to_string(), TaskStatus::Pending));
        dag.add_edge(a_idx, c_idx);

        // When checking
        // Then arrow connects A (idx 0) to C (idx 2)
        let incoming_c = dag.incoming_edges(c_idx);
        assert!(incoming_c.contains(&a_idx));
    }

    #[test]
    fn test_task_dag_view_completed_task_status() {
        // Given completed task A
        let view = TaskView::new("A".to_string(), TaskStatus::Completed);

        // When checking status
        // Then status label is "done"
        assert_eq!(view.status_label(), "done");
    }

    #[test]
    fn test_render_state_default_dag_is_none() {
        let state = RenderState::default();
        assert!(state.dag.is_none());
        assert!(!state.show_dag);
    }
}
