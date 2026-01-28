//! Messages for the TEA (The Elm Architecture) pattern.
//!
//! Messages are inputs to the update function - they come from external sources
//! like keyboard events, background actors, or command completion callbacks.

use crossterm::event::KeyEvent;

use crate::session::{Session, SessionId};

/// Input messages to the update function.
#[derive(Debug)]
pub enum Message {
    // Keyboard/terminal events
    Key(KeyEvent),
    Resize(u16, u16),

    // From background actors
    /// Preview content updated with pane activity timestamp
    PreviewUpdated(SessionId, String, u64),
    PromptDetected(SessionId, bool, bool),
    /// Auto-approve prompt for a session (trust mode)
    AutoApprovePrompt(SessionId),

    // Command completion callbacks
    SessionCreated(Session),
    SessionCreateFailed(String, String),
    SessionDeleted(SessionId),
    SessionLocked(SessionId),
    SessionLockFailed(SessionId, String),
    SessionUnlocked(SessionId),
    SessionUnlockFailed(SessionId, String),

    // State persistence
    StateSaved,
    StateSaveFailed(String),
}
