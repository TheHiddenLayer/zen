//! Commands for the TEA (The Elm Architecture) pattern.
//!
//! Commands are outputs from the update function - they represent side effects
//! to be executed by the runtime.

use crate::session::SessionId;

/// Output commands from the update function.
/// These represent side effects that need to be executed.
#[derive(Debug)]
pub enum Command {
    // Terminal control
    AttachTmux {
        tmux_name: String,
    },

    // Session operations (spawn async tasks)
    CreateSession {
        name: String,
        prompt: Option<String>,
    },
    DeleteSession {
        id: SessionId,
    },
    LockSession {
        id: SessionId,
    },
    UnlockSession {
        id: SessionId,
    },

    // State persistence
    SaveState,

    // Update session info for background actors
    UpdateSessionInfo,

    // Trust mode: auto-approve prompts
    SendEnterToSession {
        tmux_name: String,
    },

    // App lifecycle
    Quit,
}
