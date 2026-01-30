use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Git error: {0}")]
    Git(#[from] git2::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("TOML parse error: {0}")]
    TomlParse(#[from] toml::de::Error),

    #[error("TOML serialize error: {0}")]
    TomlSerialize(#[from] toml::ser::Error),

    #[error("Tmux error: {0}")]
    Tmux(String),

    #[error("Session not found: {0}")]
    SessionNotFound(String),

    #[error("Session already exists: {0}")]
    SessionExists(String),

    #[error("No home directory")]
    NoHomeDir,

    #[error("Validation error: {0}")]
    Validation(String),

    #[error("Agent not available: {0}")]
    AgentNotAvailable(String),

    #[error("Operation timed out after {0:?}")]
    Timeout(std::time::Duration),

    #[error("Task join error: {0}")]
    TaskJoin(String),

    #[error("Ref already exists: {0}")]
    RefExists(String),

    #[error("Ref not found: {0}")]
    RefNotFound(String),

    #[error("Invalid phase transition from {from} to {to}")]
    InvalidPhaseTransition { from: String, to: String },

    #[error("Agent pool is full (max: {max})")]
    AgentPoolFull { max: usize },

    #[error("Agent not found: {id}")]
    AgentNotFound { id: crate::agent::AgentId },
}

pub type Result<T> = std::result::Result<T, Error>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        assert_eq!(format!("{}", Error::NoHomeDir), "No home directory");
        assert_eq!(
            format!("{}", Error::Tmux("failed".to_string())),
            "Tmux error: failed"
        );
    }
}
