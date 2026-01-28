use crate::session::{SessionId, SessionStatus};
use crate::tea::{Mode, Notification};
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
}
