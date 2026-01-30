use crate::session::{SessionId, SessionStatus};
use crate::tea::{Mode, Notification};
use crate::workflow::{WorkflowId, WorkflowPhase, WorkflowStatus};
use chrono::{DateTime, Utc};
use std::sync::atomic::{AtomicU64, Ordering};

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
}
