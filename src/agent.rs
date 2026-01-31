use crate::config::Config;
use crate::workflow::TaskId;
use serde::{Deserialize, Serialize};
use std::time::Instant;
use uuid::Uuid;

/// Unique identifier for an agent instance.
///
/// Uses UUID v4 for generation and provides a short form display
/// for human-readable output.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct AgentId(pub Uuid);

impl AgentId {
    /// Create a new unique agent identifier.
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    /// Return first 8 characters of the UUID for display.
    pub fn short(&self) -> String {
        self.0.to_string()[..8].to_string()
    }
}

impl Default for AgentId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for AgentId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::str::FromStr for AgentId {
    type Err = uuid::Error;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        Ok(Self(Uuid::parse_str(s)?))
    }
}

/// Status of an agent in its lifecycle.
///
/// Tracks the current state of an agent including what task it's working on,
/// if it's stuck, failed, or terminated.
#[derive(Debug, Clone)]
pub enum AgentStatus {
    /// Agent is idle and available for work
    Idle,
    /// Agent is running a task
    Running {
        /// The task the agent is executing
        task_id: TaskId,
    },
    /// Agent appears to be stuck
    Stuck {
        /// When the agent was detected as stuck
        #[allow(dead_code)]
        since: Instant,
        /// Reason for being considered stuck
        reason: String,
    },
    /// Agent failed with an error
    Failed {
        /// Error message describing the failure
        error: String,
    },
    /// Agent was terminated
    Terminated,
}

impl Default for AgentStatus {
    fn default() -> Self {
        Self::Idle
    }
}

impl std::fmt::Display for AgentStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AgentStatus::Idle => write!(f, "idle"),
            AgentStatus::Running { task_id } => write!(f, "running (task: {})", task_id.0),
            AgentStatus::Stuck { reason, .. } => write!(f, "stuck: {}", reason),
            AgentStatus::Failed { error } => write!(f, "failed: {}", error),
            AgentStatus::Terminated => write!(f, "terminated"),
        }
    }
}

// Note: AgentStatus cannot fully implement Serialize/Deserialize because
// Instant is not serializable. The Stuck variant's `since` field uses
// std::time::Instant which has no stable serialization format.
// For persistence, convert to a different representation or use Duration.

/// Agent configuration for spawning AI agents.
///
/// This struct holds the command configuration for an agent.
/// For runtime agent state tracking, see AgentStatus and AgentId.
pub struct Agent {
    base_command: Vec<String>,
}

impl Agent {
    pub fn from_config(config: &Config) -> Self {
        Self {
            base_command: config
                .effective_command()
                .split_whitespace()
                .map(String::from)
                .collect(),
        }
    }

    fn is_claude(&self) -> bool {
        self.base_command
            .first()
            .map(|s| s.contains("claude"))
            .unwrap_or(true)
    }

    pub fn name(&self) -> &'static str {
        if self.is_claude() {
            "Claude"
        } else {
            "Unknown"
        }
    }

    pub fn binary(&self) -> &str {
        self.base_command
            .first()
            .map(|s| s.as_str())
            .unwrap_or("claude")
    }

    pub fn command(&self, prompt: Option<&str>) -> Vec<String> {
        let mut cmd = self.base_command.clone();
        if let Some(p) = prompt {
            cmd.push(p.to_string());
        }
        cmd
    }

    pub fn is_available(&self) -> bool {
        which::which(self.binary()).is_ok()
    }

    pub fn prompt_pattern(&self) -> Option<&'static str> {
        if self.is_claude() {
            Some("Do you want")
        } else {
            None
        }
    }
}

impl Default for Agent {
    fn default() -> Self {
        Self::from_config(&Config::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_agent() {
        let agent = Agent::default();
        assert_eq!(agent.name(), "Claude");
        assert_eq!(agent.binary(), "claude");
        assert_eq!(agent.command(None), vec!["claude"]);
        assert_eq!(agent.command(Some("test")), vec!["claude", "test"]);
        assert_eq!(agent.prompt_pattern(), Some("Do you want"));
    }

    #[test]
    fn test_custom_command() {
        let config = Config {
            command: Some("claude --dangerously-skip-permissions".to_string()),
            ..Default::default()
        };
        let agent = Agent::from_config(&config);
        assert_eq!(agent.name(), "Claude");
        assert_eq!(
            agent.command(Some("fix bug")),
            vec!["claude", "--dangerously-skip-permissions", "fix bug"]
        );
    }

    #[test]
    fn test_path_to_claude() {
        let config = Config {
            command: Some("/usr/bin/claude --flag".to_string()),
            ..Default::default()
        };
        let agent = Agent::from_config(&config);
        assert_eq!(agent.name(), "Claude");
    }

    #[test]
    fn test_unknown_agent() {
        let config = Config {
            command: Some("aider --model gpt-4".to_string()),
            ..Default::default()
        };
        let agent = Agent::from_config(&config);
        assert_eq!(agent.name(), "Unknown");
        assert_eq!(agent.prompt_pattern(), None);
    }

    // AgentId tests

    #[test]
    fn test_agent_id_new() {
        let id1 = AgentId::new();
        let id2 = AgentId::new();
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_agent_id_default() {
        let id = AgentId::default();
        assert!(!id.0.is_nil());
    }

    #[test]
    fn test_agent_id_short() {
        let id = AgentId::new();
        let short = id.short();
        assert_eq!(short.len(), 8);
    }

    #[test]
    fn test_agent_id_display() {
        let id = AgentId::new();
        let display = format!("{}", id);
        assert_eq!(display, id.0.to_string());
    }

    #[test]
    fn test_agent_id_from_str() {
        let id = AgentId::new();
        let s = id.to_string();
        let parsed: AgentId = s.parse().unwrap();
        assert_eq!(id, parsed);
    }

    #[test]
    fn test_agent_id_from_str_invalid() {
        let result: std::result::Result<AgentId, _> = "invalid".parse();
        assert!(result.is_err());
    }

    #[test]
    fn test_agent_id_serialization() {
        let id = AgentId::new();
        let json = serde_json::to_string(&id).unwrap();
        let parsed: AgentId = serde_json::from_str(&json).unwrap();
        assert_eq!(id, parsed);
    }

    #[test]
    fn test_agent_id_equality() {
        let uuid = Uuid::new_v4();
        let id1 = AgentId(uuid);
        let id2 = AgentId(uuid);
        assert_eq!(id1, id2);
    }

    #[test]
    fn test_agent_id_hash() {
        use std::collections::HashSet;

        let uuid = Uuid::new_v4();
        let id1 = AgentId(uuid);
        let id2 = AgentId(uuid);

        let mut set = HashSet::new();
        set.insert(id1);
        assert!(set.contains(&id2));
    }

    // AgentStatus tests

    #[test]
    fn test_agent_status_idle_default() {
        let status = AgentStatus::default();
        assert!(matches!(status, AgentStatus::Idle));
    }

    #[test]
    fn test_agent_status_display_idle() {
        let status = AgentStatus::Idle;
        assert_eq!(format!("{}", status), "idle");
    }

    #[test]
    fn test_agent_status_display_running() {
        let task_id = TaskId::new();
        let status = AgentStatus::Running { task_id };
        let display = format!("{}", status);
        assert!(display.starts_with("running (task: "));
    }

    #[test]
    fn test_agent_status_display_stuck() {
        let status = AgentStatus::Stuck {
            since: Instant::now(),
            reason: "no output for 5 minutes".to_string(),
        };
        assert_eq!(format!("{}", status), "stuck: no output for 5 minutes");
    }

    #[test]
    fn test_agent_status_display_failed() {
        let status = AgentStatus::Failed {
            error: "process exited with code 1".to_string(),
        };
        assert_eq!(format!("{}", status), "failed: process exited with code 1");
    }

    #[test]
    fn test_agent_status_display_terminated() {
        let status = AgentStatus::Terminated;
        assert_eq!(format!("{}", status), "terminated");
    }

    #[test]
    fn test_agent_status_running_with_task() {
        let task_id = TaskId::new();
        let status = AgentStatus::Running { task_id };
        if let AgentStatus::Running { task_id: id } = status {
            assert!(!id.0.is_nil());
        } else {
            panic!("Expected Running variant");
        }
    }

    #[test]
    fn test_agent_status_stuck_fields() {
        let now = Instant::now();
        let reason = "no progress detected".to_string();
        let status = AgentStatus::Stuck {
            since: now,
            reason: reason.clone(),
        };
        if let AgentStatus::Stuck { since, reason: r } = status {
            // since should be close to now (within a few ms)
            assert!(since.elapsed().as_millis() < 100);
            assert_eq!(r, reason);
        } else {
            panic!("Expected Stuck variant");
        }
    }

    #[test]
    fn test_agent_status_failed_error() {
        let error = "connection timeout".to_string();
        let status = AgentStatus::Failed {
            error: error.clone(),
        };
        if let AgentStatus::Failed { error: e } = status {
            assert_eq!(e, error);
        } else {
            panic!("Expected Failed variant");
        }
    }

    #[test]
    fn test_agent_status_clone() {
        let status = AgentStatus::Failed {
            error: "test error".to_string(),
        };
        let cloned = status.clone();
        assert!(matches!(cloned, AgentStatus::Failed { error } if error == "test error"));
    }

    #[test]
    fn test_agent_status_debug() {
        let status = AgentStatus::Idle;
        let debug = format!("{:?}", status);
        assert!(debug.contains("Idle"));
    }
}
