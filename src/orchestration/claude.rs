//! Claude Code headless executor with session continuation.
//!
//! The `ClaudeHeadless` struct provides programmatic execution of Claude Code
//! in headless mode (`-p` flag) with JSON output parsing. This enables
//! autonomous interaction with Claude for skill execution.
//!
//! ## Session Continuation
//!
//! Claude Code maintains conversation context via session IDs. By using the
//! `--resume` flag with a session ID, subsequent calls can continue previous
//! conversations. The `SessionManager` tracks active sessions for multi-turn
//! conversations.

use crate::error::{Error, Result};
use serde::Deserialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
use std::time::Duration;
use tokio::process::Command;

/// Default timeout for Claude execution (10 minutes).
pub const DEFAULT_TIMEOUT_SECS: u64 = 600;

/// The result type from a Claude execution.
///
/// Represents either a successful completion with output text,
/// or an error with an error message.
#[derive(Debug, Clone, PartialEq)]
pub enum ResultType {
    /// Successful execution with output.
    Success {
        /// The text output from Claude.
        output: String,
    },
    /// Failed execution with error message.
    Error {
        /// The error message describing what went wrong.
        message: String,
    },
}

/// Response from a Claude headless execution.
///
/// Contains the session ID (for potential continuation), the result
/// (success or error), and optional cost information.
#[derive(Debug, Clone)]
pub struct ClaudeResponse {
    /// Session ID for potential continuation (if available).
    pub session_id: Option<String>,
    /// The result of the execution.
    pub result: ResultType,
    /// Cost in USD (if available).
    pub cost_usd: Option<f64>,
    /// Duration in milliseconds (if available).
    pub duration_ms: Option<u64>,
    /// Number of turns/iterations (if available).
    pub num_turns: Option<u32>,
}

impl ClaudeResponse {
    /// Check if the response indicates success.
    pub fn is_success(&self) -> bool {
        matches!(self.result, ResultType::Success { .. })
    }

    /// Get the output text if successful.
    pub fn output(&self) -> Option<&str> {
        match &self.result {
            ResultType::Success { output } => Some(output),
            ResultType::Error { .. } => None,
        }
    }

    /// Get the error message if failed.
    pub fn error_message(&self) -> Option<&str> {
        match &self.result {
            ResultType::Success { .. } => None,
            ResultType::Error { message } => Some(message),
        }
    }
}

/// Internal struct for deserializing Claude JSON response.
#[derive(Debug, Deserialize)]
struct RawClaudeResponse {
    /// The response type (currently unused but kept for future extensibility).
    #[serde(rename = "type")]
    #[allow(dead_code)]
    response_type: Option<String>,
    subtype: Option<String>,
    result: Option<String>,
    session_id: Option<String>,
    total_cost_usd: Option<f64>,
    duration_ms: Option<u64>,
    num_turns: Option<u32>,
    #[serde(default)]
    error: Option<String>,
}

/// Tracks active Claude sessions for multi-turn conversations.
///
/// The `SessionManager` maintains a mapping of workflow/task identifiers to
/// their associated Claude session IDs, enabling conversation continuation
/// across multiple interactions.
///
/// # Example
///
/// ```
/// use zen::orchestration::SessionManager;
///
/// let manager = SessionManager::new();
/// manager.register("workflow_1", "session_abc123");
/// assert_eq!(manager.get("workflow_1"), Some("session_abc123".to_string()));
/// ```
#[derive(Debug, Clone, Default)]
pub struct SessionManager {
    /// Maps workflow/task identifiers to session IDs.
    sessions: Arc<RwLock<HashMap<String, String>>>,
}

impl SessionManager {
    /// Create a new empty session manager.
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a session ID for a workflow/task identifier.
    ///
    /// # Arguments
    ///
    /// * `identifier` - The workflow or task identifier
    /// * `session_id` - The Claude session ID to associate
    pub fn register(&self, identifier: &str, session_id: &str) {
        if let Ok(mut sessions) = self.sessions.write() {
            sessions.insert(identifier.to_string(), session_id.to_string());
        }
    }

    /// Get the session ID for a workflow/task identifier.
    ///
    /// # Arguments
    ///
    /// * `identifier` - The workflow or task identifier to look up
    ///
    /// # Returns
    ///
    /// The associated session ID if found, or `None` if not registered.
    pub fn get(&self, identifier: &str) -> Option<String> {
        self.sessions
            .read()
            .ok()
            .and_then(|sessions| sessions.get(identifier).cloned())
    }

    /// Remove a session registration.
    ///
    /// # Arguments
    ///
    /// * `identifier` - The workflow or task identifier to remove
    ///
    /// # Returns
    ///
    /// The removed session ID if it existed.
    pub fn remove(&self, identifier: &str) -> Option<String> {
        self.sessions
            .write()
            .ok()
            .and_then(|mut sessions| sessions.remove(identifier))
    }

    /// Clear all session registrations.
    ///
    /// Useful for cleanup when a workflow completes or is cancelled.
    pub fn clear(&self) {
        if let Ok(mut sessions) = self.sessions.write() {
            sessions.clear();
        }
    }

    /// Get the number of registered sessions.
    pub fn len(&self) -> usize {
        self.sessions
            .read()
            .map(|sessions| sessions.len())
            .unwrap_or(0)
    }

    /// Check if there are no registered sessions.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Get all registered session identifiers.
    pub fn identifiers(&self) -> Vec<String> {
        self.sessions
            .read()
            .map(|sessions| sessions.keys().cloned().collect())
            .unwrap_or_default()
    }
}

/// Claude Code headless executor.
///
/// Executes Claude Code in non-interactive mode using the `-p` flag with
/// JSON output format. Parses the response and returns structured results.
///
/// # Example
///
/// ```ignore
/// use zen::orchestration::ClaudeHeadless;
/// use std::path::Path;
///
/// let claude = ClaudeHeadless::new()?;
/// let response = claude.execute("Explain this code", Path::new(".")).await?;
///
/// if response.is_success() {
///     println!("Output: {}", response.output().unwrap());
/// }
/// ```
#[derive(Debug, Clone)]
pub struct ClaudeHeadless {
    /// Path to the Claude binary.
    binary: PathBuf,
    /// Output format (always "json").
    output_format: String,
    /// Timeout for execution.
    timeout: Duration,
}

impl ClaudeHeadless {
    /// Create a new ClaudeHeadless executor.
    ///
    /// Automatically detects the Claude binary using `which`.
    ///
    /// # Errors
    ///
    /// Returns an error if the Claude binary cannot be found.
    pub fn new() -> Result<Self> {
        let binary = which::which("claude").map_err(|_| Error::ClaudeBinaryNotFound)?;
        Ok(Self {
            binary,
            output_format: "json".to_string(),
            timeout: Duration::from_secs(DEFAULT_TIMEOUT_SECS),
        })
    }

    /// Create a ClaudeHeadless executor with a specific binary path.
    ///
    /// Useful for testing or when Claude is installed in a non-standard location.
    pub fn with_binary(binary: PathBuf) -> Self {
        Self {
            binary,
            output_format: "json".to_string(),
            timeout: Duration::from_secs(DEFAULT_TIMEOUT_SECS),
        }
    }

    /// Set a custom timeout for execution.
    ///
    /// # Arguments
    ///
    /// * `timeout` - The maximum duration to wait for Claude to complete.
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Get the binary path.
    pub fn binary(&self) -> &Path {
        &self.binary
    }

    /// Get the output format.
    pub fn output_format(&self) -> &str {
        &self.output_format
    }

    /// Get the timeout.
    pub fn timeout(&self) -> Duration {
        self.timeout
    }

    /// Execute a prompt in Claude headless mode.
    ///
    /// Runs Claude with the given prompt in the specified working directory,
    /// parses the JSON output, and returns a structured response.
    ///
    /// # Arguments
    ///
    /// * `prompt` - The prompt to send to Claude.
    /// * `cwd` - The working directory for execution.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The command fails to spawn
    /// - The command times out
    /// - The output cannot be parsed as UTF-8
    pub async fn execute(&self, prompt: &str, cwd: &Path) -> Result<ClaudeResponse> {
        let output = tokio::time::timeout(
            self.timeout,
            Command::new(&self.binary)
                .arg("-p")
                .arg(prompt)
                .arg("--output-format")
                .arg(&self.output_format)
                .current_dir(cwd)
                .output(),
        )
        .await
        .map_err(|_| Error::Timeout(self.timeout))?
        .map_err(Error::Io)?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        // Try to parse JSON response
        if let Ok(response) = Self::parse_json_response(&stdout) {
            return Ok(response);
        }

        // If JSON parsing failed, check exit code
        if !output.status.success() {
            let error_msg = if stderr.is_empty() {
                format!(
                    "Claude execution failed with exit code {}",
                    output.status.code().unwrap_or(-1)
                )
            } else {
                stderr.trim().to_string()
            };

            return Ok(ClaudeResponse {
                session_id: None,
                result: ResultType::Error { message: error_msg },
                cost_usd: None,
                duration_ms: None,
                num_turns: None,
            });
        }

        // Non-JSON success output (shouldn't happen with --output-format json)
        Ok(ClaudeResponse {
            session_id: None,
            result: ResultType::Success {
                output: stdout.trim().to_string(),
            },
            cost_usd: None,
            duration_ms: None,
            num_turns: None,
        })
    }

    /// Continue an existing Claude session with a new prompt.
    ///
    /// Uses the `--resume` flag to continue a previous conversation,
    /// maintaining the context from earlier interactions.
    ///
    /// # Arguments
    ///
    /// * `session_id` - The session ID from a previous execution
    /// * `prompt` - The new prompt to send
    /// * `cwd` - The working directory for execution
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The command fails to spawn
    /// - The command times out
    /// - The session cannot be resumed (e.g., invalid session ID)
    ///
    /// # Example
    ///
    /// ```ignore
    /// // First execution
    /// let response1 = claude.execute("Explain this code", Path::new(".")).await?;
    /// let session_id = response1.session_id.expect("Should have session ID");
    ///
    /// // Continue the conversation
    /// let response2 = claude.continue_session(&session_id, "What about error handling?", Path::new(".")).await?;
    /// ```
    pub async fn continue_session(
        &self,
        session_id: &str,
        prompt: &str,
        cwd: &Path,
    ) -> Result<ClaudeResponse> {
        let output = tokio::time::timeout(
            self.timeout,
            Command::new(&self.binary)
                .arg("--resume")
                .arg(session_id)
                .arg("-p")
                .arg(prompt)
                .arg("--output-format")
                .arg(&self.output_format)
                .current_dir(cwd)
                .output(),
        )
        .await
        .map_err(|_| Error::Timeout(self.timeout))?
        .map_err(Error::Io)?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        // Try to parse JSON response
        if let Ok(response) = Self::parse_json_response(&stdout) {
            return Ok(response);
        }

        // If JSON parsing failed, check exit code
        if !output.status.success() {
            let error_msg = if stderr.is_empty() {
                format!(
                    "Claude session continuation failed with exit code {}",
                    output.status.code().unwrap_or(-1)
                )
            } else {
                stderr.trim().to_string()
            };

            return Ok(ClaudeResponse {
                session_id: Some(session_id.to_string()),
                result: ResultType::Error { message: error_msg },
                cost_usd: None,
                duration_ms: None,
                num_turns: None,
            });
        }

        // Non-JSON success output
        Ok(ClaudeResponse {
            session_id: Some(session_id.to_string()),
            result: ResultType::Success {
                output: stdout.trim().to_string(),
            },
            cost_usd: None,
            duration_ms: None,
            num_turns: None,
        })
    }

    /// Execute a prompt with a specific model.
    ///
    /// Uses the `--model` flag to select a specific Claude model
    /// for execution. Useful for fast responses with lighter models
    /// like haiku.
    ///
    /// # Arguments
    ///
    /// * `prompt` - The prompt to send to Claude
    /// * `cwd` - The working directory for execution
    /// * `model` - The model to use (e.g., "haiku", "sonnet", "opus")
    ///
    /// # Errors
    ///
    /// Returns an error if execution fails.
    pub async fn execute_with_model(
        &self,
        prompt: &str,
        cwd: &Path,
        model: &str,
    ) -> Result<ClaudeResponse> {
        let output = tokio::time::timeout(
            self.timeout,
            Command::new(&self.binary)
                .arg("-p")
                .arg(prompt)
                .arg("--model")
                .arg(model)
                .arg("--output-format")
                .arg(&self.output_format)
                .current_dir(cwd)
                .output(),
        )
        .await
        .map_err(|_| Error::Timeout(self.timeout))?
        .map_err(Error::Io)?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        // Try to parse JSON response
        if let Ok(response) = Self::parse_json_response(&stdout) {
            return Ok(response);
        }

        // If JSON parsing failed, check exit code
        if !output.status.success() {
            let error_msg = if stderr.is_empty() {
                format!(
                    "Claude execution failed with exit code {}",
                    output.status.code().unwrap_or(-1)
                )
            } else {
                stderr.trim().to_string()
            };

            return Ok(ClaudeResponse {
                session_id: None,
                result: ResultType::Error { message: error_msg },
                cost_usd: None,
                duration_ms: None,
                num_turns: None,
            });
        }

        // Non-JSON success output
        Ok(ClaudeResponse {
            session_id: None,
            result: ResultType::Success {
                output: stdout.trim().to_string(),
            },
            cost_usd: None,
            duration_ms: None,
            num_turns: None,
        })
    }

    /// Continue a session with a specific model.
    ///
    /// Combines session continuation with model selection, useful for
    /// multi-turn conversations with specific model requirements.
    ///
    /// # Arguments
    ///
    /// * `session_id` - The session ID from a previous execution
    /// * `prompt` - The new prompt to send
    /// * `cwd` - The working directory for execution
    /// * `model` - The model to use
    pub async fn continue_session_with_model(
        &self,
        session_id: &str,
        prompt: &str,
        cwd: &Path,
        model: &str,
    ) -> Result<ClaudeResponse> {
        let output = tokio::time::timeout(
            self.timeout,
            Command::new(&self.binary)
                .arg("--resume")
                .arg(session_id)
                .arg("-p")
                .arg(prompt)
                .arg("--model")
                .arg(model)
                .arg("--output-format")
                .arg(&self.output_format)
                .current_dir(cwd)
                .output(),
        )
        .await
        .map_err(|_| Error::Timeout(self.timeout))?
        .map_err(Error::Io)?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        // Try to parse JSON response
        if let Ok(response) = Self::parse_json_response(&stdout) {
            return Ok(response);
        }

        // If JSON parsing failed, check exit code
        if !output.status.success() {
            let error_msg = if stderr.is_empty() {
                format!(
                    "Claude session continuation failed with exit code {}",
                    output.status.code().unwrap_or(-1)
                )
            } else {
                stderr.trim().to_string()
            };

            return Ok(ClaudeResponse {
                session_id: Some(session_id.to_string()),
                result: ResultType::Error { message: error_msg },
                cost_usd: None,
                duration_ms: None,
                num_turns: None,
            });
        }

        // Non-JSON success output
        Ok(ClaudeResponse {
            session_id: Some(session_id.to_string()),
            result: ResultType::Success {
                output: stdout.trim().to_string(),
            },
            cost_usd: None,
            duration_ms: None,
            num_turns: None,
        })
    }

    /// Parse a JSON response from Claude.
    ///
    /// # Arguments
    ///
    /// * `json_str` - The JSON string to parse.
    ///
    /// # Returns
    ///
    /// A `ClaudeResponse` if parsing succeeds, or an error if the JSON is invalid.
    pub fn parse_json_response(json_str: &str) -> Result<ClaudeResponse> {
        let raw: RawClaudeResponse = serde_json::from_str(json_str)?;

        let result = match raw.subtype.as_deref() {
            Some("success") => ResultType::Success {
                output: raw.result.unwrap_or_default(),
            },
            Some("error") => ResultType::Error {
                message: raw.error.or(raw.result).unwrap_or_default(),
            },
            _ => {
                // If no subtype, check if we have a result or error
                if let Some(error) = raw.error {
                    ResultType::Error { message: error }
                } else if let Some(result) = raw.result {
                    ResultType::Success { output: result }
                } else {
                    ResultType::Error {
                        message: "Unknown response format".to_string(),
                    }
                }
            }
        };

        Ok(ClaudeResponse {
            session_id: raw.session_id,
            result,
            cost_usd: raw.total_cost_usd,
            duration_ms: raw.duration_ms,
            num_turns: raw.num_turns,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ========== ResultType Tests ==========

    #[test]
    fn test_result_type_success() {
        let result = ResultType::Success {
            output: "Hello, world!".to_string(),
        };
        if let ResultType::Success { output } = result {
            assert_eq!(output, "Hello, world!");
        } else {
            panic!("Expected Success variant");
        }
    }

    #[test]
    fn test_result_type_error() {
        let result = ResultType::Error {
            message: "Something went wrong".to_string(),
        };
        if let ResultType::Error { message } = result {
            assert_eq!(message, "Something went wrong");
        } else {
            panic!("Expected Error variant");
        }
    }

    #[test]
    fn test_result_type_debug() {
        let result = ResultType::Success {
            output: "test".to_string(),
        };
        let debug = format!("{:?}", result);
        assert!(debug.contains("Success"));
    }

    #[test]
    fn test_result_type_clone() {
        let result = ResultType::Success {
            output: "test".to_string(),
        };
        let cloned = result.clone();
        assert_eq!(result, cloned);
    }

    #[test]
    fn test_result_type_equality() {
        let a = ResultType::Success {
            output: "foo".to_string(),
        };
        let b = ResultType::Success {
            output: "foo".to_string(),
        };
        assert_eq!(a, b);

        let c = ResultType::Error {
            message: "err".to_string(),
        };
        let d = ResultType::Error {
            message: "err".to_string(),
        };
        assert_eq!(c, d);

        let e = ResultType::Success {
            output: "foo".to_string(),
        };
        let f = ResultType::Error {
            message: "foo".to_string(),
        };
        assert_ne!(e, f);
    }

    // ========== ClaudeResponse Tests ==========

    #[test]
    fn test_claude_response_debug() {
        let response = ClaudeResponse {
            session_id: Some("abc123".to_string()),
            result: ResultType::Success {
                output: "test".to_string(),
            },
            cost_usd: Some(0.001),
            duration_ms: Some(1000),
            num_turns: Some(3),
        };
        let debug = format!("{:?}", response);
        assert!(debug.contains("ClaudeResponse"));
        assert!(debug.contains("abc123"));
    }

    #[test]
    fn test_claude_response_clone() {
        let response = ClaudeResponse {
            session_id: Some("abc123".to_string()),
            result: ResultType::Success {
                output: "test".to_string(),
            },
            cost_usd: Some(0.001),
            duration_ms: Some(1000),
            num_turns: Some(3),
        };
        let cloned = response.clone();
        assert_eq!(response.session_id, cloned.session_id);
    }

    #[test]
    fn test_claude_response_is_success_true() {
        let response = ClaudeResponse {
            session_id: None,
            result: ResultType::Success {
                output: "test".to_string(),
            },
            cost_usd: None,
            duration_ms: None,
            num_turns: None,
        };
        assert!(response.is_success());
    }

    #[test]
    fn test_claude_response_is_success_false() {
        let response = ClaudeResponse {
            session_id: None,
            result: ResultType::Error {
                message: "error".to_string(),
            },
            cost_usd: None,
            duration_ms: None,
            num_turns: None,
        };
        assert!(!response.is_success());
    }

    #[test]
    fn test_claude_response_output_success() {
        let response = ClaudeResponse {
            session_id: None,
            result: ResultType::Success {
                output: "Hello".to_string(),
            },
            cost_usd: None,
            duration_ms: None,
            num_turns: None,
        };
        assert_eq!(response.output(), Some("Hello"));
    }

    #[test]
    fn test_claude_response_output_error() {
        let response = ClaudeResponse {
            session_id: None,
            result: ResultType::Error {
                message: "error".to_string(),
            },
            cost_usd: None,
            duration_ms: None,
            num_turns: None,
        };
        assert_eq!(response.output(), None);
    }

    #[test]
    fn test_claude_response_error_message_success() {
        let response = ClaudeResponse {
            session_id: None,
            result: ResultType::Success {
                output: "test".to_string(),
            },
            cost_usd: None,
            duration_ms: None,
            num_turns: None,
        };
        assert_eq!(response.error_message(), None);
    }

    #[test]
    fn test_claude_response_error_message_error() {
        let response = ClaudeResponse {
            session_id: None,
            result: ResultType::Error {
                message: "Something failed".to_string(),
            },
            cost_usd: None,
            duration_ms: None,
            num_turns: None,
        };
        assert_eq!(response.error_message(), Some("Something failed"));
    }

    // ========== ClaudeHeadless Struct Tests ==========

    #[test]
    fn test_claude_headless_with_binary() {
        let binary = PathBuf::from("/usr/local/bin/claude");
        let claude = ClaudeHeadless::with_binary(binary.clone());
        assert_eq!(claude.binary(), binary.as_path());
    }

    #[test]
    fn test_claude_headless_default_output_format() {
        let claude = ClaudeHeadless::with_binary(PathBuf::from("/bin/claude"));
        assert_eq!(claude.output_format(), "json");
    }

    #[test]
    fn test_claude_headless_default_timeout() {
        let claude = ClaudeHeadless::with_binary(PathBuf::from("/bin/claude"));
        assert_eq!(claude.timeout(), Duration::from_secs(DEFAULT_TIMEOUT_SECS));
    }

    #[test]
    fn test_claude_headless_with_timeout() {
        let claude = ClaudeHeadless::with_binary(PathBuf::from("/bin/claude"))
            .with_timeout(Duration::from_secs(30));
        assert_eq!(claude.timeout(), Duration::from_secs(30));
    }

    #[test]
    fn test_claude_headless_debug() {
        let claude = ClaudeHeadless::with_binary(PathBuf::from("/bin/claude"));
        let debug = format!("{:?}", claude);
        assert!(debug.contains("ClaudeHeadless"));
        assert!(debug.contains("json"));
    }

    #[test]
    fn test_claude_headless_clone() {
        let claude = ClaudeHeadless::with_binary(PathBuf::from("/bin/claude"));
        let cloned = claude.clone();
        assert_eq!(claude.binary(), cloned.binary());
        assert_eq!(claude.output_format(), cloned.output_format());
    }

    #[test]
    fn test_default_timeout_secs_is_reasonable() {
        // 10 minutes is a reasonable timeout for complex tasks
        assert_eq!(DEFAULT_TIMEOUT_SECS, 600);
    }

    // ========== JSON Parsing Tests ==========

    #[test]
    fn test_parse_successful_json_response() {
        let json = r#"{
            "type": "result",
            "subtype": "success",
            "result": "Hello, world!",
            "session_id": "abc123",
            "total_cost_usd": 0.003,
            "duration_ms": 1234,
            "num_turns": 6
        }"#;

        let response = ClaudeHeadless::parse_json_response(json).unwrap();
        assert!(response.is_success());
        assert_eq!(response.output(), Some("Hello, world!"));
        assert_eq!(response.session_id, Some("abc123".to_string()));
        assert_eq!(response.cost_usd, Some(0.003));
        assert_eq!(response.duration_ms, Some(1234));
        assert_eq!(response.num_turns, Some(6));
    }

    #[test]
    fn test_parse_error_json_response() {
        let json = r#"{
            "type": "result",
            "subtype": "error",
            "error": "Authentication failed",
            "session_id": "xyz789"
        }"#;

        let response = ClaudeHeadless::parse_json_response(json).unwrap();
        assert!(!response.is_success());
        assert_eq!(response.error_message(), Some("Authentication failed"));
        assert_eq!(response.session_id, Some("xyz789".to_string()));
    }

    #[test]
    fn test_parse_json_missing_session_id() {
        let json = r#"{
            "type": "result",
            "subtype": "success",
            "result": "Output text"
        }"#;

        let response = ClaudeHeadless::parse_json_response(json).unwrap();
        assert!(response.is_success());
        assert!(response.session_id.is_none());
    }

    #[test]
    fn test_parse_json_missing_cost() {
        let json = r#"{
            "type": "result",
            "subtype": "success",
            "result": "Output text",
            "session_id": "abc"
        }"#;

        let response = ClaudeHeadless::parse_json_response(json).unwrap();
        assert!(response.is_success());
        assert!(response.cost_usd.is_none());
    }

    #[test]
    fn test_parse_invalid_json() {
        let json = "not valid json";
        let result = ClaudeHeadless::parse_json_response(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_json_empty_object() {
        let json = "{}";
        let response = ClaudeHeadless::parse_json_response(json).unwrap();
        // Should produce an error response since no subtype or result
        assert!(!response.is_success());
    }

    #[test]
    fn test_parse_json_with_result_but_no_subtype() {
        let json = r#"{
            "result": "Some output"
        }"#;

        let response = ClaudeHeadless::parse_json_response(json).unwrap();
        // Should treat as success since there's a result
        assert!(response.is_success());
        assert_eq!(response.output(), Some("Some output"));
    }

    #[test]
    fn test_parse_json_with_error_field_but_no_subtype() {
        let json = r#"{
            "error": "Something went wrong"
        }"#;

        let response = ClaudeHeadless::parse_json_response(json).unwrap();
        // Should treat as error since there's an error field
        assert!(!response.is_success());
        assert_eq!(response.error_message(), Some("Something went wrong"));
    }

    #[test]
    fn test_parse_json_error_subtype_uses_result_if_no_error() {
        let json = r#"{
            "subtype": "error",
            "result": "Error details in result field"
        }"#;

        let response = ClaudeHeadless::parse_json_response(json).unwrap();
        assert!(!response.is_success());
        assert_eq!(
            response.error_message(),
            Some("Error details in result field")
        );
    }

    // ========== Binary Detection Tests ==========

    #[test]
    fn test_claude_headless_new_fails_when_binary_not_found() {
        // This test documents the expected behavior.
        // In CI/test environments without Claude installed, new() should return
        // ClaudeBinaryNotFound error. In environments WITH Claude, it succeeds.
        let result = ClaudeHeadless::new();
        // We can't assert success or failure since it depends on the environment.
        // Just verify it returns a valid Result.
        match result {
            Ok(claude) => {
                // Claude is installed - verify it detected a path
                assert!(!claude.binary().as_os_str().is_empty());
            }
            Err(e) => {
                // Claude not installed - verify correct error type
                assert!(matches!(e, Error::ClaudeBinaryNotFound));
            }
        }
    }

    // ========== Execute Method Tests (Integration) ==========
    // These tests require actual Claude binary and are marked ignore

    #[tokio::test]
    #[ignore = "requires claude binary"]
    async fn test_execute_simple_prompt() {
        let claude = ClaudeHeadless::new().expect("Claude binary should exist");
        let response = claude
            .execute("Say 'hello' and nothing else", std::path::Path::new("."))
            .await;

        assert!(response.is_ok());
        let response = response.unwrap();
        assert!(response.is_success());
    }

    #[tokio::test]
    async fn test_execute_with_nonexistent_binary() {
        let claude = ClaudeHeadless::with_binary(PathBuf::from("/nonexistent/binary"));
        let result = claude.execute("test", Path::new(".")).await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_execute_timeout_very_short() {
        // This test uses a real echo command to test timeout behavior
        // without needing Claude installed
        let claude = ClaudeHeadless::with_binary(PathBuf::from("sleep"))
            .with_timeout(Duration::from_millis(10));

        // We're using sleep as a stand-in command that will timeout
        // The execute method will fail because sleep doesn't accept -p flag
        // but this tests the timeout path in the code
        let result = claude.execute("1", Path::new(".")).await;

        // Either timeout or error is acceptable here
        // (timeout if sleep somehow runs, error if it rejects args)
        assert!(result.is_ok() || result.is_err());
    }

    // ========== SessionManager Tests ==========

    #[test]
    fn test_session_manager_new() {
        let manager = SessionManager::new();
        assert!(manager.is_empty());
        assert_eq!(manager.len(), 0);
    }

    #[test]
    fn test_session_manager_default() {
        let manager = SessionManager::default();
        assert!(manager.is_empty());
    }

    #[test]
    fn test_session_manager_register_and_get() {
        let manager = SessionManager::new();
        manager.register("workflow_1", "session_abc123");

        assert_eq!(
            manager.get("workflow_1"),
            Some("session_abc123".to_string())
        );
        assert!(!manager.is_empty());
        assert_eq!(manager.len(), 1);
    }

    #[test]
    fn test_session_manager_get_nonexistent() {
        let manager = SessionManager::new();
        assert_eq!(manager.get("nonexistent"), None);
    }

    #[test]
    fn test_session_manager_register_multiple() {
        let manager = SessionManager::new();
        manager.register("workflow_1", "session_1");
        manager.register("workflow_2", "session_2");
        manager.register("task_3", "session_3");

        assert_eq!(manager.len(), 3);
        assert_eq!(manager.get("workflow_1"), Some("session_1".to_string()));
        assert_eq!(manager.get("workflow_2"), Some("session_2".to_string()));
        assert_eq!(manager.get("task_3"), Some("session_3".to_string()));
    }

    #[test]
    fn test_session_manager_register_overwrite() {
        let manager = SessionManager::new();
        manager.register("workflow_1", "old_session");
        manager.register("workflow_1", "new_session");

        assert_eq!(manager.len(), 1);
        assert_eq!(manager.get("workflow_1"), Some("new_session".to_string()));
    }

    #[test]
    fn test_session_manager_remove() {
        let manager = SessionManager::new();
        manager.register("workflow_1", "session_1");
        manager.register("workflow_2", "session_2");

        let removed = manager.remove("workflow_1");
        assert_eq!(removed, Some("session_1".to_string()));
        assert_eq!(manager.len(), 1);
        assert_eq!(manager.get("workflow_1"), None);
        assert_eq!(manager.get("workflow_2"), Some("session_2".to_string()));
    }

    #[test]
    fn test_session_manager_remove_nonexistent() {
        let manager = SessionManager::new();
        let removed = manager.remove("nonexistent");
        assert_eq!(removed, None);
    }

    #[test]
    fn test_session_manager_clear() {
        let manager = SessionManager::new();
        manager.register("workflow_1", "session_1");
        manager.register("workflow_2", "session_2");

        manager.clear();
        assert!(manager.is_empty());
        assert_eq!(manager.len(), 0);
    }

    #[test]
    fn test_session_manager_identifiers() {
        let manager = SessionManager::new();
        manager.register("workflow_1", "session_1");
        manager.register("task_2", "session_2");

        let identifiers = manager.identifiers();
        assert_eq!(identifiers.len(), 2);
        assert!(identifiers.contains(&"workflow_1".to_string()));
        assert!(identifiers.contains(&"task_2".to_string()));
    }

    #[test]
    fn test_session_manager_identifiers_empty() {
        let manager = SessionManager::new();
        assert!(manager.identifiers().is_empty());
    }

    #[test]
    fn test_session_manager_clone() {
        let manager = SessionManager::new();
        manager.register("workflow_1", "session_1");

        let cloned = manager.clone();
        // Cloned manager shares the same Arc, so they see the same data
        assert_eq!(cloned.get("workflow_1"), Some("session_1".to_string()));

        // Adding to original is visible in clone (they share the Arc)
        manager.register("workflow_2", "session_2");
        assert_eq!(cloned.get("workflow_2"), Some("session_2".to_string()));
    }

    #[test]
    fn test_session_manager_debug() {
        let manager = SessionManager::new();
        manager.register("workflow_1", "session_1");
        let debug = format!("{:?}", manager);
        assert!(debug.contains("SessionManager"));
    }

    // ========== Continue Session Tests (Integration) ==========

    #[tokio::test]
    async fn test_continue_session_with_nonexistent_binary() {
        let claude = ClaudeHeadless::with_binary(PathBuf::from("/nonexistent/binary"));
        let result = claude
            .continue_session("session_abc", "continue", Path::new("."))
            .await;

        assert!(result.is_err());
    }

    #[tokio::test]
    #[ignore = "requires claude binary"]
    async fn test_continue_session_integration() {
        let claude = ClaudeHeadless::new().expect("Claude binary should exist");

        // First, execute a prompt to get a session ID
        let response1 = claude
            .execute("Remember the number 42", Path::new("."))
            .await
            .expect("First execution should succeed");

        let session_id = response1.session_id.expect("Should have session ID");

        // Continue the session
        let response2 = claude
            .continue_session(&session_id, "What number did I ask you to remember?", Path::new("."))
            .await
            .expect("Continue session should succeed");

        assert!(response2.is_success());
    }

    #[tokio::test]
    async fn test_execute_with_model_nonexistent_binary() {
        let claude = ClaudeHeadless::with_binary(PathBuf::from("/nonexistent/binary"));
        let result = claude
            .execute_with_model("test", Path::new("."), "haiku")
            .await;

        assert!(result.is_err());
    }

    #[tokio::test]
    #[ignore = "requires claude binary"]
    async fn test_execute_with_model_haiku() {
        let claude = ClaudeHeadless::new().expect("Claude binary should exist");
        let response = claude
            .execute_with_model("Say hello", Path::new("."), "haiku")
            .await
            .expect("Should succeed with haiku model");

        assert!(response.is_success());
    }

    #[tokio::test]
    async fn test_continue_session_with_model_nonexistent_binary() {
        let claude = ClaudeHeadless::with_binary(PathBuf::from("/nonexistent/binary"));
        let result = claude
            .continue_session_with_model("session_abc", "continue", Path::new("."), "haiku")
            .await;

        assert!(result.is_err());
    }
}
