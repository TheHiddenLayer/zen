//! Workflow state management with phase transition validation.
//!
//! This module provides `WorkflowState` which wraps a `Workflow` and enforces
//! valid phase transitions according to the Skills-driven workflow ordering.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};

use super::{Workflow, WorkflowPhase};

/// A record of a phase transition with timestamp.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhaseHistoryEntry {
    /// The phase that was entered.
    pub phase: WorkflowPhase,
    /// When this phase was entered.
    pub entered_at: DateTime<Utc>,
}

/// Manages workflow state and enforces valid phase transitions.
///
/// The `WorkflowState` ensures that workflows progress through phases
/// in the correct order according to the Skills-driven workflow:
///
/// Planning -> TaskGeneration -> Implementation -> Merging -> Documentation -> Complete
///
/// Note: Documentation is optional, so Merging can transition directly to Complete.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowState {
    /// The underlying workflow being managed.
    workflow: Workflow,
    /// History of all phases visited, in order.
    phase_history: Vec<PhaseHistoryEntry>,
}

impl WorkflowState {
    /// Create a new WorkflowState from an existing Workflow.
    ///
    /// The workflow's current phase is recorded as the first history entry.
    pub fn new(workflow: Workflow) -> Self {
        let initial_phase = workflow.phase;
        Self {
            workflow,
            phase_history: vec![PhaseHistoryEntry {
                phase: initial_phase,
                entered_at: Utc::now(),
            }],
        }
    }

    /// Check if a transition to the target phase is valid from the current phase.
    ///
    /// Valid transitions follow the strict ordering:
    /// - Planning -> TaskGeneration
    /// - TaskGeneration -> Implementation
    /// - Implementation -> Merging
    /// - Merging -> Documentation OR Complete
    /// - Documentation -> Complete
    pub fn can_transition(&self, target: WorkflowPhase) -> bool {
        let current = self.workflow.phase;
        matches!(
            (current, target),
            (WorkflowPhase::Planning, WorkflowPhase::TaskGeneration)
                | (WorkflowPhase::TaskGeneration, WorkflowPhase::Implementation)
                | (WorkflowPhase::Implementation, WorkflowPhase::Merging)
                | (WorkflowPhase::Merging, WorkflowPhase::Documentation)
                | (WorkflowPhase::Merging, WorkflowPhase::Complete)
                | (WorkflowPhase::Documentation, WorkflowPhase::Complete)
        )
    }

    /// Attempt to transition the workflow to a new phase.
    ///
    /// Returns an error if the transition is not valid according to
    /// the workflow phase ordering rules.
    pub fn transition(&mut self, target: WorkflowPhase) -> Result<()> {
        if !self.can_transition(target) {
            return Err(Error::InvalidPhaseTransition {
                from: self.workflow.phase.to_string(),
                to: target.to_string(),
            });
        }

        self.workflow.phase = target;
        self.phase_history.push(PhaseHistoryEntry {
            phase: target,
            entered_at: Utc::now(),
        });

        Ok(())
    }

    /// Get the current phase of the workflow.
    pub fn current_phase(&self) -> WorkflowPhase {
        self.workflow.phase
    }

    /// Get the history of all phases visited, in order.
    pub fn phase_history(&self) -> &[PhaseHistoryEntry] {
        &self.phase_history
    }

    /// Get a reference to the underlying workflow.
    pub fn workflow(&self) -> &Workflow {
        &self.workflow
    }

    /// Get a mutable reference to the underlying workflow.
    pub fn workflow_mut(&mut self) -> &mut Workflow {
        &mut self.workflow
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workflow::WorkflowConfig;

    fn create_test_workflow() -> Workflow {
        Workflow::new("test workflow prompt", WorkflowConfig::default())
    }

    fn create_test_workflow_at_phase(phase: WorkflowPhase) -> Workflow {
        let mut workflow = create_test_workflow();
        workflow.phase = phase;
        workflow
    }

    // WorkflowState construction tests

    #[test]
    fn test_workflow_state_new() {
        let workflow = create_test_workflow();
        let state = WorkflowState::new(workflow);

        assert_eq!(state.current_phase(), WorkflowPhase::Planning);
        assert_eq!(state.phase_history().len(), 1);
        assert_eq!(state.phase_history()[0].phase, WorkflowPhase::Planning);
    }

    #[test]
    fn test_workflow_state_preserves_workflow_data() {
        let workflow = create_test_workflow();
        let original_id = workflow.id;
        let original_prompt = workflow.prompt.clone();

        let state = WorkflowState::new(workflow);

        assert_eq!(state.workflow().id, original_id);
        assert_eq!(state.workflow().prompt, original_prompt);
    }

    // Valid forward transitions tests

    #[test]
    fn test_transition_planning_to_task_generation() {
        let workflow = create_test_workflow_at_phase(WorkflowPhase::Planning);
        let mut state = WorkflowState::new(workflow);

        let result = state.transition(WorkflowPhase::TaskGeneration);

        assert!(result.is_ok());
        assert_eq!(state.current_phase(), WorkflowPhase::TaskGeneration);
    }

    #[test]
    fn test_transition_task_generation_to_implementation() {
        let workflow = create_test_workflow_at_phase(WorkflowPhase::TaskGeneration);
        let mut state = WorkflowState::new(workflow);

        let result = state.transition(WorkflowPhase::Implementation);

        assert!(result.is_ok());
        assert_eq!(state.current_phase(), WorkflowPhase::Implementation);
    }

    #[test]
    fn test_transition_implementation_to_merging() {
        let workflow = create_test_workflow_at_phase(WorkflowPhase::Implementation);
        let mut state = WorkflowState::new(workflow);

        let result = state.transition(WorkflowPhase::Merging);

        assert!(result.is_ok());
        assert_eq!(state.current_phase(), WorkflowPhase::Merging);
    }

    #[test]
    fn test_transition_merging_to_documentation() {
        let workflow = create_test_workflow_at_phase(WorkflowPhase::Merging);
        let mut state = WorkflowState::new(workflow);

        let result = state.transition(WorkflowPhase::Documentation);

        assert!(result.is_ok());
        assert_eq!(state.current_phase(), WorkflowPhase::Documentation);
    }

    #[test]
    fn test_transition_documentation_to_complete() {
        let workflow = create_test_workflow_at_phase(WorkflowPhase::Documentation);
        let mut state = WorkflowState::new(workflow);

        let result = state.transition(WorkflowPhase::Complete);

        assert!(result.is_ok());
        assert_eq!(state.current_phase(), WorkflowPhase::Complete);
    }

    // Optional documentation phase test

    #[test]
    fn test_transition_merging_to_complete_skipping_documentation() {
        let workflow = create_test_workflow_at_phase(WorkflowPhase::Merging);
        let mut state = WorkflowState::new(workflow);

        let result = state.transition(WorkflowPhase::Complete);

        assert!(result.is_ok());
        assert_eq!(state.current_phase(), WorkflowPhase::Complete);
    }

    // Invalid skip transitions tests

    #[test]
    fn test_invalid_transition_planning_to_implementation() {
        let workflow = create_test_workflow_at_phase(WorkflowPhase::Planning);
        let mut state = WorkflowState::new(workflow);

        let result = state.transition(WorkflowPhase::Implementation);

        assert!(result.is_err());
        assert_eq!(state.current_phase(), WorkflowPhase::Planning);
    }

    #[test]
    fn test_invalid_transition_planning_to_merging() {
        let workflow = create_test_workflow_at_phase(WorkflowPhase::Planning);
        let mut state = WorkflowState::new(workflow);

        let result = state.transition(WorkflowPhase::Merging);

        assert!(result.is_err());
        assert_eq!(state.current_phase(), WorkflowPhase::Planning);
    }

    #[test]
    fn test_invalid_transition_planning_to_documentation() {
        let workflow = create_test_workflow_at_phase(WorkflowPhase::Planning);
        let mut state = WorkflowState::new(workflow);

        let result = state.transition(WorkflowPhase::Documentation);

        assert!(result.is_err());
        assert_eq!(state.current_phase(), WorkflowPhase::Planning);
    }

    #[test]
    fn test_invalid_transition_planning_to_complete() {
        let workflow = create_test_workflow_at_phase(WorkflowPhase::Planning);
        let mut state = WorkflowState::new(workflow);

        let result = state.transition(WorkflowPhase::Complete);

        assert!(result.is_err());
        assert_eq!(state.current_phase(), WorkflowPhase::Planning);
    }

    #[test]
    fn test_invalid_transition_task_generation_to_merging() {
        let workflow = create_test_workflow_at_phase(WorkflowPhase::TaskGeneration);
        let mut state = WorkflowState::new(workflow);

        let result = state.transition(WorkflowPhase::Merging);

        assert!(result.is_err());
        assert_eq!(state.current_phase(), WorkflowPhase::TaskGeneration);
    }

    #[test]
    fn test_invalid_transition_task_generation_to_documentation() {
        let workflow = create_test_workflow_at_phase(WorkflowPhase::TaskGeneration);
        let mut state = WorkflowState::new(workflow);

        let result = state.transition(WorkflowPhase::Documentation);

        assert!(result.is_err());
        assert_eq!(state.current_phase(), WorkflowPhase::TaskGeneration);
    }

    #[test]
    fn test_invalid_transition_task_generation_to_complete() {
        let workflow = create_test_workflow_at_phase(WorkflowPhase::TaskGeneration);
        let mut state = WorkflowState::new(workflow);

        let result = state.transition(WorkflowPhase::Complete);

        assert!(result.is_err());
        assert_eq!(state.current_phase(), WorkflowPhase::TaskGeneration);
    }

    #[test]
    fn test_invalid_transition_implementation_to_documentation() {
        let workflow = create_test_workflow_at_phase(WorkflowPhase::Implementation);
        let mut state = WorkflowState::new(workflow);

        let result = state.transition(WorkflowPhase::Documentation);

        assert!(result.is_err());
        assert_eq!(state.current_phase(), WorkflowPhase::Implementation);
    }

    #[test]
    fn test_invalid_transition_implementation_to_complete() {
        let workflow = create_test_workflow_at_phase(WorkflowPhase::Implementation);
        let mut state = WorkflowState::new(workflow);

        let result = state.transition(WorkflowPhase::Complete);

        assert!(result.is_err());
        assert_eq!(state.current_phase(), WorkflowPhase::Implementation);
    }

    // Backward transitions tests (all invalid)

    #[test]
    fn test_invalid_backward_transition_task_generation_to_planning() {
        let workflow = create_test_workflow_at_phase(WorkflowPhase::TaskGeneration);
        let mut state = WorkflowState::new(workflow);

        let result = state.transition(WorkflowPhase::Planning);

        assert!(result.is_err());
        assert_eq!(state.current_phase(), WorkflowPhase::TaskGeneration);
    }

    #[test]
    fn test_invalid_backward_transition_implementation_to_task_generation() {
        let workflow = create_test_workflow_at_phase(WorkflowPhase::Implementation);
        let mut state = WorkflowState::new(workflow);

        let result = state.transition(WorkflowPhase::TaskGeneration);

        assert!(result.is_err());
        assert_eq!(state.current_phase(), WorkflowPhase::Implementation);
    }

    #[test]
    fn test_invalid_backward_transition_merging_to_implementation() {
        let workflow = create_test_workflow_at_phase(WorkflowPhase::Merging);
        let mut state = WorkflowState::new(workflow);

        let result = state.transition(WorkflowPhase::Implementation);

        assert!(result.is_err());
        assert_eq!(state.current_phase(), WorkflowPhase::Merging);
    }

    #[test]
    fn test_invalid_transition_from_complete() {
        let workflow = create_test_workflow_at_phase(WorkflowPhase::Complete);
        let mut state = WorkflowState::new(workflow);

        // Try all possible transitions from Complete - all should fail
        assert!(state.transition(WorkflowPhase::Planning).is_err());
        assert!(state.transition(WorkflowPhase::TaskGeneration).is_err());
        assert!(state.transition(WorkflowPhase::Implementation).is_err());
        assert!(state.transition(WorkflowPhase::Merging).is_err());
        assert!(state.transition(WorkflowPhase::Documentation).is_err());
        assert_eq!(state.current_phase(), WorkflowPhase::Complete);
    }

    // Same phase transition tests

    #[test]
    fn test_invalid_same_phase_transition() {
        let workflow = create_test_workflow_at_phase(WorkflowPhase::Planning);
        let mut state = WorkflowState::new(workflow);

        let result = state.transition(WorkflowPhase::Planning);

        assert!(result.is_err());
        assert_eq!(state.current_phase(), WorkflowPhase::Planning);
    }

    // Phase history tracking tests

    #[test]
    fn test_phase_history_tracks_all_transitions() {
        let workflow = create_test_workflow_at_phase(WorkflowPhase::Planning);
        let mut state = WorkflowState::new(workflow);

        state.transition(WorkflowPhase::TaskGeneration).unwrap();
        state.transition(WorkflowPhase::Implementation).unwrap();
        state.transition(WorkflowPhase::Merging).unwrap();

        let history = state.phase_history();
        assert_eq!(history.len(), 4);
        assert_eq!(history[0].phase, WorkflowPhase::Planning);
        assert_eq!(history[1].phase, WorkflowPhase::TaskGeneration);
        assert_eq!(history[2].phase, WorkflowPhase::Implementation);
        assert_eq!(history[3].phase, WorkflowPhase::Merging);
    }

    #[test]
    fn test_phase_history_preserves_order() {
        let workflow = create_test_workflow_at_phase(WorkflowPhase::Planning);
        let mut state = WorkflowState::new(workflow);

        state.transition(WorkflowPhase::TaskGeneration).unwrap();
        state.transition(WorkflowPhase::Implementation).unwrap();

        let history = state.phase_history();
        // Verify timestamps are in order
        for i in 1..history.len() {
            assert!(history[i].entered_at >= history[i - 1].entered_at);
        }
    }

    #[test]
    fn test_phase_history_not_modified_on_failed_transition() {
        let workflow = create_test_workflow_at_phase(WorkflowPhase::Planning);
        let mut state = WorkflowState::new(workflow);

        let initial_history_len = state.phase_history().len();

        // Try invalid transition
        let _ = state.transition(WorkflowPhase::Merging);

        assert_eq!(state.phase_history().len(), initial_history_len);
    }

    // can_transition helper tests

    #[test]
    fn test_can_transition_valid_transitions() {
        let workflow = create_test_workflow_at_phase(WorkflowPhase::Planning);
        let state = WorkflowState::new(workflow);
        assert!(state.can_transition(WorkflowPhase::TaskGeneration));

        let workflow = create_test_workflow_at_phase(WorkflowPhase::TaskGeneration);
        let state = WorkflowState::new(workflow);
        assert!(state.can_transition(WorkflowPhase::Implementation));

        let workflow = create_test_workflow_at_phase(WorkflowPhase::Implementation);
        let state = WorkflowState::new(workflow);
        assert!(state.can_transition(WorkflowPhase::Merging));

        let workflow = create_test_workflow_at_phase(WorkflowPhase::Merging);
        let state = WorkflowState::new(workflow);
        assert!(state.can_transition(WorkflowPhase::Documentation));
        assert!(state.can_transition(WorkflowPhase::Complete));

        let workflow = create_test_workflow_at_phase(WorkflowPhase::Documentation);
        let state = WorkflowState::new(workflow);
        assert!(state.can_transition(WorkflowPhase::Complete));
    }

    #[test]
    fn test_can_transition_invalid_transitions() {
        let workflow = create_test_workflow_at_phase(WorkflowPhase::Planning);
        let state = WorkflowState::new(workflow);

        assert!(!state.can_transition(WorkflowPhase::Planning));
        assert!(!state.can_transition(WorkflowPhase::Implementation));
        assert!(!state.can_transition(WorkflowPhase::Merging));
        assert!(!state.can_transition(WorkflowPhase::Documentation));
        assert!(!state.can_transition(WorkflowPhase::Complete));
    }

    // Error message tests

    #[test]
    fn test_error_message_contains_phase_info() {
        let workflow = create_test_workflow_at_phase(WorkflowPhase::Planning);
        let mut state = WorkflowState::new(workflow);

        let result = state.transition(WorkflowPhase::Merging);
        let err = result.unwrap_err();
        let msg = format!("{}", err);

        assert!(msg.contains("planning"));
        assert!(msg.contains("merging"));
    }

    // Full workflow traversal test

    #[test]
    fn test_full_workflow_traversal_with_documentation() {
        let workflow = create_test_workflow_at_phase(WorkflowPhase::Planning);
        let mut state = WorkflowState::new(workflow);

        state.transition(WorkflowPhase::TaskGeneration).unwrap();
        state.transition(WorkflowPhase::Implementation).unwrap();
        state.transition(WorkflowPhase::Merging).unwrap();
        state.transition(WorkflowPhase::Documentation).unwrap();
        state.transition(WorkflowPhase::Complete).unwrap();

        assert_eq!(state.current_phase(), WorkflowPhase::Complete);
        assert_eq!(state.phase_history().len(), 6);
    }

    #[test]
    fn test_full_workflow_traversal_without_documentation() {
        let workflow = create_test_workflow_at_phase(WorkflowPhase::Planning);
        let mut state = WorkflowState::new(workflow);

        state.transition(WorkflowPhase::TaskGeneration).unwrap();
        state.transition(WorkflowPhase::Implementation).unwrap();
        state.transition(WorkflowPhase::Merging).unwrap();
        state.transition(WorkflowPhase::Complete).unwrap();

        assert_eq!(state.current_phase(), WorkflowPhase::Complete);
        assert_eq!(state.phase_history().len(), 5);
    }

    // Serialization tests

    #[test]
    fn test_workflow_state_serialization() {
        let workflow = create_test_workflow_at_phase(WorkflowPhase::Planning);
        let mut state = WorkflowState::new(workflow);
        state.transition(WorkflowPhase::TaskGeneration).unwrap();

        let json = serde_json::to_string(&state).unwrap();
        let parsed: WorkflowState = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.current_phase(), WorkflowPhase::TaskGeneration);
        assert_eq!(parsed.phase_history().len(), 2);
    }

    #[test]
    fn test_phase_history_entry_serialization() {
        let entry = PhaseHistoryEntry {
            phase: WorkflowPhase::Implementation,
            entered_at: Utc::now(),
        };

        let json = serde_json::to_string(&entry).unwrap();
        let parsed: PhaseHistoryEntry = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.phase, WorkflowPhase::Implementation);
    }

    // workflow_mut test

    #[test]
    fn test_workflow_mut_allows_modification() {
        let workflow = create_test_workflow();
        let mut state = WorkflowState::new(workflow);

        state.workflow_mut().name = String::from("modified-name");

        assert_eq!(state.workflow().name, "modified-name");
    }
}
