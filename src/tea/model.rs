//! Model for the TEA (The Elm Architecture) pattern.
//!
//! The Model is pure application state - no channels, no handles, no runtime infrastructure.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use crate::agent::Agent;
use crate::config::Config;
use crate::render::{next_version, RenderState, SessionView};
use crate::session::{Session, SessionId, State};

/// Level of a notification message.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotificationLevel {
    /// Error notification - displayed in red with "Error:" prefix
    Error,
    /// Informational notification - displayed in green
    Info,
}

/// A notification message to display to the user.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Notification {
    /// The severity level of the notification
    pub level: NotificationLevel,
    /// The notification message text
    pub message: String,
}

/// Application UI mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Mode {
    #[default]
    List,
    Input(InputKind),
}

/// Types of input prompts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputKind {
    SessionName,
    Prompt,
    Confirm,
}

impl InputKind {
    pub fn label(&self) -> &'static str {
        match self {
            InputKind::SessionName => "Name",
            InputKind::Prompt => "Prompt",
            InputKind::Confirm => "Delete?",
        }
    }

    /// Cycle to next input field (Tab behavior).
    /// Returns None for Confirm since it doesn't cycle.
    pub fn next(&self) -> Option<InputKind> {
        match self {
            InputKind::SessionName => Some(InputKind::Prompt),
            InputKind::Prompt => Some(InputKind::SessionName),
            InputKind::Confirm => None,
        }
    }
}

/// Prompt detection state for a session.
#[derive(Debug, Clone, Copy, Default)]
pub struct PromptState {
    pub has_prompt: bool,
    pub user_attached: bool,
}

/// Pure application state - the single source of truth.
pub struct Model {
    // Core state
    pub sessions: Vec<Session>,
    pub selected: usize,
    pub mode: Mode,

    // Caches (updated by background actors)
    pub preview_cache: HashMap<SessionId, String>,
    pub prompt_cache: HashMap<SessionId, PromptState>,
    /// Activity state: (last_activity_timestamp, last_change_time)
    /// last_change_time is when we last saw the timestamp change (for grace period)
    pub activity_cache: HashMap<SessionId, (u64, std::time::Instant)>,

    // Input state
    pub input_buffer: String,
    pub notification: Option<Notification>,
    pub pending_delete: Option<SessionId>,
    pub pending_session_name: Option<String>,
    pub pending_prompt: Option<String>,

    // UI toggle state
    /// Whether the keymap legend is expanded (toggled by '?')
    pub show_keymap: bool,

    // Dirty flag - set when state changes and render is needed
    pub dirty: bool,

    // Config (immutable after init)
    pub config: Config,
    pub repo_path: Option<PathBuf>,
    pub agent: Arc<Agent>,
}

impl Model {
    /// Create a new Model from loaded state.
    pub fn new(
        sessions: Vec<Session>,
        config: Config,
        repo_path: Option<PathBuf>,
        agent: Arc<Agent>,
    ) -> Self {
        Self {
            sessions,
            selected: 0,
            mode: Mode::default(),
            preview_cache: HashMap::new(),
            prompt_cache: HashMap::new(),
            activity_cache: HashMap::new(),
            input_buffer: String::new(),
            notification: None,
            pending_delete: None,
            pending_session_name: None,
            pending_prompt: None,
            show_keymap: false,
            dirty: true,
            config,
            repo_path,
            agent,
        }
    }

    /// Load model from persisted state.
    pub async fn load(config: Config, agent: Arc<Agent>) -> crate::Result<Self> {
        let repo_path = detect_repo_path();
        let mut state = State::load().await?;
        state.reconcile().await;
        state.save().await?;

        Ok(Self::new(state.sessions, config, repo_path, agent))
    }

    // Accessor methods for UI

    pub fn selected_session(&self) -> Option<&Session> {
        self.sessions.get(self.selected)
    }

    /// Create an immutable snapshot for the render thread.
    ///
    /// This is called after state updates to send the current view
    /// to the render thread via a lock-free channel.
    ///
    /// Each snapshot gets a monotonically increasing version number,
    /// enabling the render thread to detect state changes and skip
    /// redundant renders. This is a zero-cost abstraction when no
    /// render occurs.
    pub fn snapshot(&self) -> RenderState {
        use std::time::Duration;
        const ACTIVITY_GRACE_PERIOD: Duration = Duration::from_millis(1500);

        let sessions: Vec<SessionView> = self
            .sessions
            .iter()
            .map(|s| {
                // Active if we saw activity change within the grace period
                // None if we haven't received any activity data yet (loading state)
                let is_active = self
                    .activity_cache
                    .get(&s.id)
                    .map(|(_, last_change)| last_change.elapsed() < ACTIVITY_GRACE_PERIOD);
                SessionView {
                    id: s.id,
                    name: s.name.clone(),
                    project: s.project.clone(),
                    branch: s.branch.clone(),
                    base_branch: s.base_branch.clone(),
                    base_commit: s.base_commit.clone(),
                    agent: s.agent.clone(),
                    status: s.status,
                    last_active: s.last_active,
                    is_active,
                }
            })
            .collect();

        let preview = self
            .selected_session()
            .and_then(|s| self.preview_cache.get(&s.id))
            .cloned();

        RenderState {
            version: next_version(),
            sessions,
            selected: self.selected,
            mode: self.mode,
            preview,
            input_buffer: self.input_buffer.clone(),
            notification: self.notification.clone(),
            show_keymap: self.show_keymap,
            trust_enabled: self.config.trust,
            workflow: None, // TODO: populate from workflow state when available
        }
    }
}

fn detect_repo_path() -> Option<PathBuf> {
    let cwd = std::env::current_dir().ok()?;
    crate::git::GitOps::new(&cwd)
        .ok()
        .map(|g| g.repo_path().to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;

    // ═══════════════════════════════════════════════════════════════════════
    // Notification Tests
    // ═══════════════════════════════════════════════════════════════════════

    #[test]
    fn test_notification_error_level() {
        let notification = Notification {
            level: NotificationLevel::Error,
            message: "Test error".to_string(),
        };
        assert_eq!(notification.level, NotificationLevel::Error);
        assert_eq!(notification.message, "Test error");
    }

    #[test]
    fn test_notification_info_level() {
        let notification = Notification {
            level: NotificationLevel::Info,
            message: "Test info".to_string(),
        };
        assert_eq!(notification.level, NotificationLevel::Info);
        assert_eq!(notification.message, "Test info");
    }

    #[test]
    fn test_notification_clone() {
        let notification = Notification {
            level: NotificationLevel::Error,
            message: "Original message".to_string(),
        };
        let cloned = notification.clone();
        assert_eq!(cloned.level, notification.level);
        assert_eq!(cloned.message, notification.message);
    }

    #[test]
    fn test_notification_level_equality() {
        assert_eq!(NotificationLevel::Error, NotificationLevel::Error);
        assert_eq!(NotificationLevel::Info, NotificationLevel::Info);
        assert_ne!(NotificationLevel::Error, NotificationLevel::Info);
    }

    #[test]
    fn test_notification_equality() {
        let notif1 = Notification {
            level: NotificationLevel::Error,
            message: "Same message".to_string(),
        };
        let notif2 = Notification {
            level: NotificationLevel::Error,
            message: "Same message".to_string(),
        };
        let notif3 = Notification {
            level: NotificationLevel::Info,
            message: "Same message".to_string(),
        };
        let notif4 = Notification {
            level: NotificationLevel::Error,
            message: "Different message".to_string(),
        };

        assert_eq!(notif1, notif2);
        assert_ne!(notif1, notif3); // Different level
        assert_ne!(notif1, notif4); // Different message
    }

    // ═══════════════════════════════════════════════════════════════════════
    // Mode and InputKind Tests
    // ═══════════════════════════════════════════════════════════════════════

    #[test]
    fn test_mode_default() {
        assert_eq!(Mode::default(), Mode::List);
    }

    #[test]
    fn test_input_kind_label() {
        assert_eq!(InputKind::SessionName.label(), "Name");
        assert_eq!(InputKind::Prompt.label(), "Prompt");
        assert_eq!(InputKind::Confirm.label(), "Delete?");
    }

    #[test]
    fn test_input_kind_next() {
        assert_eq!(InputKind::SessionName.next(), Some(InputKind::Prompt));
        assert_eq!(InputKind::Prompt.next(), Some(InputKind::SessionName));
        assert_eq!(InputKind::Confirm.next(), None);
    }
}
