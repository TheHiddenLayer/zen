use std::path::Path;
use std::process::Command;

use crate::{zlog_debug, zlog_trace, zlog_warn, Error, Result};

pub struct Tmux;

impl Tmux {
    pub fn create_session(name: &str, cwd: &Path, cmd: &[String]) -> Result<()> {
        if cmd.is_empty() {
            return Err(Error::Validation("Command cannot be empty".to_string()));
        }

        let cmd_str = cmd
            .iter()
            .map(|s| shell_escape(s))
            .collect::<Vec<_>>()
            .join(" ");
        zlog_debug!(
            "Tmux::create_session name={} cwd={} cmd={}",
            name,
            cwd.display(),
            cmd_str
        );
        let output = Command::new("tmux")
            .args([
                "new-session",
                "-d",
                "-s",
                name,
                "-c",
                &cwd.display().to_string(),
                &cmd_str,
            ])
            .output()?;

        if !output.status.success() {
            let err = format!(
                "Failed to create session '{}': {}",
                name,
                String::from_utf8_lossy(&output.stderr)
            );
            zlog_warn!("tmux create_session failed: {}", err);
            return Err(Error::Tmux(err));
        }

        // Keep session alive when command exits
        let _ = Command::new("tmux")
            .args(["set-option", "-t", name, "remain-on-exit", "on"])
            .output();

        zlog_debug!("Tmux session created: {}", name);
        Ok(())
    }

    pub fn kill_session(name: &str) -> Result<()> {
        zlog_debug!("Tmux::kill_session name={}", name);
        let output = Command::new("tmux")
            .args(["kill-session", "-t", name])
            .output()?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            if !stderr.contains("session not found") {
                zlog_warn!("Failed to kill tmux session '{}': {}", name, stderr);
                return Err(Error::Tmux(format!(
                    "Failed to kill session '{}': {}",
                    name, stderr
                )));
            }
            zlog_debug!("Tmux session '{}' not found (already dead?)", name);
        } else {
            zlog_debug!("Tmux session killed: {}", name);
        }
        Ok(())
    }

    pub fn capture_pane(name: &str) -> Result<String> {
        zlog_trace!("Tmux::capture_pane name={}", name);
        let output = Command::new("tmux")
            .args(["capture-pane", "-t", name, "-p", "-e"])
            .output()?;
        if !output.status.success() {
            return Err(Error::Tmux(format!(
                "Failed to capture pane '{}': {}",
                name,
                String::from_utf8_lossy(&output.stderr)
            )));
        }
        let content = String::from_utf8_lossy(&output.stdout).to_string();
        zlog_trace!("capture_pane: {} bytes", content.len());
        Ok(content)
    }

    pub fn capture_pane_plain(name: &str) -> Result<String> {
        let output = Command::new("tmux")
            .args(["capture-pane", "-t", name, "-p"])
            .output()?;
        if !output.status.success() {
            return Err(Error::Tmux(format!(
                "Failed to capture pane '{}': {}",
                name,
                String::from_utf8_lossy(&output.stderr)
            )));
        }
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    /// Capture only the last N lines of a tmux pane.
    /// This is more efficient than capturing the entire pane and helps avoid
    /// false positives from historical output.
    pub fn capture_pane_tail(name: &str, lines: u16) -> Result<String> {
        // Use -S (start line) with negative value to get last N lines
        // -S -N means "start N lines from the end"
        let start = format!("-{}", lines);
        let output = Command::new("tmux")
            .args(["capture-pane", "-t", name, "-p", "-S", &start])
            .output()?;
        if !output.status.success() {
            return Err(Error::Tmux(format!(
                "Failed to capture pane tail '{}': {}",
                name,
                String::from_utf8_lossy(&output.stderr)
            )));
        }
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    pub fn session_exists(name: &str) -> bool {
        Command::new("tmux")
            .args(["has-session", "-t", name])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    /// Check if we're running inside a tmux session
    pub fn inside_tmux() -> bool {
        std::env::var("TMUX").is_ok()
    }

    pub fn attach(name: &str) -> Result<()> {
        zlog_debug!("Tmux::attach name={}", name);
        // Configure session-local settings
        let _ = Command::new("tmux")
            .args(["set-option", "-t", name, "status", "off"])
            .output();
        let _ = Command::new("tmux")
            .args(["bind-key", "-n", "C-q", "detach-client"])
            .output();
        let _ = Command::new("tmux")
            .args([
                "set-hook",
                "-t",
                name,
                "client-attached",
                "display-message -d 5000 'Press ^q to return'",
            ])
            .output();

        if Self::inside_tmux() {
            zlog_debug!("Attaching via popup (inside tmux)");
            let status = Command::new("tmux")
                .args([
                    "display-popup",
                    "-E",
                    "-w",
                    "95%",
                    "-h",
                    "95%",
                    "tmux",
                    "attach-session",
                    "-t",
                    name,
                ])
                .status()?;
            if !status.success() {
                zlog_warn!("Failed to open popup for session '{}'", name);
                return Err(Error::Tmux(format!(
                    "Failed to open popup for session '{}'",
                    name
                )));
            }
        } else {
            zlog_debug!("Attaching directly (outside tmux)");
            let status = Command::new("tmux")
                .args(["attach-session", "-t", name])
                .status()?;
            if !status.success() {
                zlog_warn!("Failed to attach to session '{}'", name);
                return Err(Error::Tmux(format!(
                    "Failed to attach to session '{}'",
                    name
                )));
            }
        }
        zlog_debug!("Detached from session: {}", name);
        Ok(())
    }

    pub fn send_keys(name: &str, keys: &str) -> Result<()> {
        zlog_debug!("Tmux::send_keys name={} keys={}", name, keys);
        let output = Command::new("tmux")
            .args(["send-keys", "-t", name, keys])
            .output()?;
        if !output.status.success() {
            zlog_warn!(
                "Failed to send keys to '{}': {}",
                name,
                String::from_utf8_lossy(&output.stderr)
            );
            return Err(Error::Tmux(format!(
                "Failed to send keys to '{}': {}",
                name,
                String::from_utf8_lossy(&output.stderr)
            )));
        }
        Ok(())
    }

    pub fn send_keys_enter(name: &str, keys: &str) -> Result<()> {
        zlog_debug!("Tmux::send_keys_enter name={} keys={}", name, keys);
        let output = Command::new("tmux")
            .args(["send-keys", "-t", name, keys, "Enter"])
            .output()?;
        if !output.status.success() {
            zlog_warn!(
                "Failed to send keys to '{}': {}",
                name,
                String::from_utf8_lossy(&output.stderr)
            );
            return Err(Error::Tmux(format!(
                "Failed to send keys to '{}': {}",
                name,
                String::from_utf8_lossy(&output.stderr)
            )));
        }
        Ok(())
    }

    pub fn list_sessions() -> Result<Vec<String>> {
        zlog_trace!("Tmux::list_sessions");
        let output = Command::new("tmux")
            .args(["list-sessions", "-F", "#{session_name}"])
            .output()?;
        if !output.status.success() {
            zlog_debug!("No tmux sessions found");
            return Ok(Vec::new());
        }
        let sessions: Vec<String> = String::from_utf8_lossy(&output.stdout)
            .lines()
            .map(String::from)
            .collect();
        zlog_trace!("list_sessions: found {} sessions", sessions.len());
        Ok(sessions)
    }

    pub fn list_zen_sessions() -> Result<Vec<String>> {
        let sessions: Vec<String> = Self::list_sessions()?
            .into_iter()
            .filter(|s| s.starts_with("zen_"))
            .collect();
        zlog_debug!("list_zen_sessions: found {} zen sessions", sessions.len());
        Ok(sessions)
    }

    pub fn session_cwd(name: &str) -> Result<String> {
        let output = Command::new("tmux")
            .args(["display-message", "-t", name, "-p", "#{pane_current_path}"])
            .output()?;
        if !output.status.success() {
            return Err(Error::Tmux(format!(
                "Failed to get cwd for '{}': {}",
                name,
                String::from_utf8_lossy(&output.stderr)
            )));
        }
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }

    pub fn resize_pane(name: &str, width: u16, height: u16) -> Result<()> {
        let output = Command::new("tmux")
            .args([
                "resize-pane",
                "-t",
                name,
                "-x",
                &width.to_string(),
                "-y",
                &height.to_string(),
            ])
            .output()?;
        if !output.status.success() {
            return Err(Error::Tmux(format!(
                "Failed to resize pane '{}': {}",
                name,
                String::from_utf8_lossy(&output.stderr)
            )));
        }
        Ok(())
    }

    /// Check if a session has any clients attached.
    /// Returns "1" if attached, "0" otherwise.
    pub fn session_attached(name: &str) -> Result<String> {
        let output = Command::new("tmux")
            .args(["display-message", "-t", name, "-p", "#{session_attached}"])
            .output()?;
        if !output.status.success() {
            return Err(Error::Tmux(format!(
                "Failed to get session attached status for '{}': {}",
                name,
                String::from_utf8_lossy(&output.stderr)
            )));
        }
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }

    /// Get window activity timestamp (Unix timestamp of last activity).
    pub fn pane_activity(name: &str) -> Result<u64> {
        let output = Command::new("tmux")
            .args(["display-message", "-t", name, "-p", "#{window_activity}"])
            .output()?;
        if !output.status.success() {
            return Err(Error::Tmux(format!(
                "Failed to get window activity for '{}': {}",
                name,
                String::from_utf8_lossy(&output.stderr)
            )));
        }
        let timestamp_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
        timestamp_str.parse::<u64>().map_err(|_| {
            Error::Tmux(format!(
                "Invalid window activity timestamp: {}",
                timestamp_str
            ))
        })
    }

    pub fn is_available() -> bool {
        Command::new("tmux")
            .arg("-V")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    pub fn version() -> Result<String> {
        let output = Command::new("tmux").arg("-V").output()?;
        if !output.status.success() {
            return Err(Error::Tmux("Failed to get tmux version".to_string()));
        }
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }

    pub fn session_name(name: &str, short_id: &str) -> String {
        format!("zen_{}_{}", sanitize_session_name(name), short_id)
    }
}

fn shell_escape(s: &str) -> String {
    if s.chars()
        .all(|c| c.is_alphanumeric() || c == '-' || c == '_' || c == '.')
    {
        s.to_string()
    } else {
        format!("'{}'", s.replace('\'', "'\"'\"'"))
    }
}

fn sanitize_session_name(s: &str) -> String {
    s.chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shell_escape() {
        assert_eq!(shell_escape("hello"), "hello");
        assert_eq!(shell_escape("hello world"), "'hello world'");
    }

    #[test]
    fn test_sanitize_session_name() {
        assert_eq!(sanitize_session_name("hello world"), "hello_world");
    }

    #[test]
    fn test_session_name() {
        assert_eq!(
            Tmux::session_name("my-session", "abc123"),
            "zen_my-session_abc123"
        );
    }

    #[test]
    fn test_session_attached_format() {
        // Test that the function exists
        // Note: tmux display-message may succeed even for non-existent sessions
        // if tmux is running, so we just verify the function doesn't panic
        let _result = Tmux::session_attached("nonexistent_session_12345");
    }
}
