//! Actor system for background tasks.
//!
//! Each actor is an independent tokio task that communicates with the main
//! application via message passing. Actors handle:
//! - Terminal pane preview capture (PreviewActor)
//! - Prompt detection (PromptDetectorActor)
//!
//! NOTE: Keyboard input is handled synchronously in the logic thread,
//! not via an actor, for minimum latency.

pub mod preview;
pub mod prompt;

use tokio_util::sync::CancellationToken;

pub use preview::PreviewActor;
pub use prompt::PromptDetectorActor;

/// Handle to a running actor, used for graceful shutdown.
pub struct ActorHandle {
    cancel: CancellationToken,
}

impl ActorHandle {
    /// Create a new actor handle with a cancellation token.
    pub fn new(cancel: CancellationToken) -> Self {
        Self { cancel }
    }

    /// Signal the actor to shut down gracefully.
    pub fn shutdown(&self) {
        self.cancel.cancel();
    }

    /// Check if shutdown has been requested.
    pub fn is_cancelled(&self) -> bool {
        self.cancel.is_cancelled()
    }
}

/// Session information shared with background actors.
#[derive(Debug, Clone)]
pub struct SessionInfo {
    pub id: crate::session::SessionId,
    pub tmux_name: String,
    pub repo_path: Option<std::path::PathBuf>,
    pub worktree_path: Option<std::path::PathBuf>,
    pub prompt_pattern: Option<String>,
}
