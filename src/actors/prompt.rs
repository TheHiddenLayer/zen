//! Prompt detector actor for detecting when agent is waiting for input.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Duration;

use futures::future::join_all;
use tokio::sync::{mpsc, RwLock};
use tokio_util::sync::CancellationToken;

use crate::session::SessionId;
use crate::tea::Message;
use crate::tmux::Tmux;
use crate::util::blocking_with_timeout;
use crate::{zlog_debug, zlog_trace, zlog_warn};

use super::{ActorHandle, SessionInfo};

/// Result of prompt detection for a session.
#[derive(Debug, Clone)]
struct PromptCheckResult {
    session_id: SessionId,
    has_prompt: bool,
    user_attached: bool,
    content_hash: u64,
}

const PROMPT_INTERVAL: Duration = Duration::from_millis(500);
const TMUX_TIMEOUT: Duration = Duration::from_millis(100);
/// Number of lines to capture from the end of the tmux pane.
/// Searching only recent output is more efficient and avoids false positives.
const TAIL_LINES: u16 = 30;

/// Actor that detects when the agent is waiting for user input.
/// When trust mode is enabled, automatically approves prompts for unattached sessions.
pub struct PromptDetectorActor {
    msg_tx: mpsc::UnboundedSender<Message>,
    session_info: Arc<RwLock<Vec<SessionInfo>>>,
    interval: Duration,
    trust_enabled: bool,
}

impl PromptDetectorActor {
    pub fn new(
        msg_tx: mpsc::UnboundedSender<Message>,
        session_info: Arc<RwLock<Vec<SessionInfo>>>,
        trust_enabled: bool,
    ) -> Self {
        Self {
            msg_tx,
            session_info,
            interval: PROMPT_INTERVAL,
            trust_enabled,
        }
    }

    pub fn with_interval(mut self, interval: Duration) -> Self {
        self.interval = interval;
        self
    }

    pub fn spawn(self) -> ActorHandle {
        let cancel = CancellationToken::new();
        let cancel_clone = cancel.clone();
        let trust_enabled = self.trust_enabled;

        zlog_debug!("PromptDetectorActor::spawn trust_enabled={}", trust_enabled);

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(self.interval);
            // Track content hash per session to prevent duplicate auto-approvals
            let mut last_content_hash: HashMap<SessionId, u64> = HashMap::new();

            loop {
                tokio::select! {
                    _ = cancel_clone.cancelled() => {
                        zlog_debug!("PromptDetectorActor cancelled");
                        break;
                    }
                    _ = interval.tick() => {
                        if self.msg_tx.is_closed() {
                            zlog_debug!("PromptDetectorActor: message channel closed");
                            break;
                        }

                        let infos = self.session_info.read().await;
                        if infos.is_empty() {
                            continue;
                        }

                        // Build current session ID set for cleanup
                        let current_session_ids: HashSet<SessionId> =
                            infos.iter().map(|info| info.id).collect();

                        zlog_trace!(
                            "PromptDetector: checking {} sessions",
                            infos.len()
                        );

                        // Check all sessions in parallel
                        let tasks: Vec<_> = infos.iter().map(|info| {
                            let id = info.id;
                            let tmux_name = info.tmux_name.clone();
                            let prompt_pattern = info.prompt_pattern.clone();
                            let needs_hash = trust_enabled;

                            tokio::spawn(async move {
                                let (has_prompt, content_hash) = detect_prompt(
                                    &tmux_name,
                                    prompt_pattern.as_deref(),
                                    needs_hash
                                ).await;
                                let user_attached = session_attached(&tmux_name).await;

                                PromptCheckResult {
                                    session_id: id,
                                    has_prompt,
                                    user_attached,
                                    content_hash,
                                }
                            })
                        }).collect();

                        // TRUE PARALLELISM: await all tasks concurrently
                        let results = join_all(tasks).await;

                        // Process results - all I/O already completed
                        for result in results {
                            match result {
                                Ok(check) => {
                                    if self.msg_tx.send(Message::PromptDetected(
                                        check.session_id,
                                        check.has_prompt,
                                        check.user_attached
                                    )).is_err() {
                                        zlog_warn!("Failed to send PromptDetected message - channel closed");
                                        break;
                                    }

                                    // Trust mode: auto-approve if prompt detected and user not attached
                                    if trust_enabled && check.has_prompt && !check.user_attached {
                                        // Only auto-approve if content changed (new prompt)
                                        let content_changed = last_content_hash
                                            .get(&check.session_id)
                                            .map(|&last_hash| last_hash != check.content_hash)
                                            .unwrap_or(true); // First time seeing this session

                                        if content_changed {
                                            zlog_debug!(
                                                "Auto-approving prompt for session {}",
                                                check.session_id.short()
                                            );
                                            if self.msg_tx.send(Message::AutoApprovePrompt(check.session_id)).is_err() {
                                                zlog_warn!("Failed to send AutoApprovePrompt message - channel closed");
                                                break;
                                            }
                                            last_content_hash.insert(check.session_id, check.content_hash);
                                        }
                                    }

                                    // Clean up old entries when prompt disappears
                                    if !check.has_prompt {
                                        last_content_hash.remove(&check.session_id);
                                    }
                                }
                                Err(join_err) => {
                                    zlog_warn!("Prompt detection task panicked: {}", join_err);
                                }
                            }
                        }

                        // Clean up hash entries for sessions that no longer exist
                        last_content_hash.retain(|id, _| current_session_ids.contains(id));
                    }
                }
            }
        });

        ActorHandle::new(cancel)
    }
}

/// Detect if a prompt is present in the tmux pane.
/// Returns (has_prompt, content_hash) to enable deduplication.
/// Only computes hash when needs_hash=true to avoid wasted cycles.
async fn detect_prompt(
    tmux_name: &str,
    prompt_pattern: Option<&str>,
    needs_hash: bool,
) -> (bool, u64) {
    let Some(pattern) = prompt_pattern else {
        return (false, 0);
    };

    match capture_pane_tail(tmux_name, TAIL_LINES).await {
        Ok(content) => {
            let has_prompt = content.contains(pattern);
            // OPTIMIZATION: Only hash if we need it (trust mode enabled)
            let hash = if needs_hash && has_prompt {
                hash_string(&content)
            } else {
                0
            };
            (has_prompt, hash)
        }
        Err(_) => (false, 0),
    }
}

/// Capture only the last N lines of a tmux pane.
/// This is more efficient than capturing the entire pane and avoids false positives
/// from historical output.
async fn capture_pane_tail(session_name: &str, lines: u16) -> crate::Result<String> {
    // OPTIMIZATION: Pass &str directly, only clone inside blocking task
    let name = session_name.to_string();
    blocking_with_timeout(TMUX_TIMEOUT, move || Tmux::capture_pane_tail(&name, lines)).await
}

async fn session_attached(session_name: &str) -> bool {
    // OPTIMIZATION: Pass &str directly, only clone inside blocking task
    let name = session_name.to_string();
    blocking_with_timeout(TMUX_TIMEOUT, move || Tmux::session_attached(&name))
        .await
        .ok()
        .map(|s| s.trim() == "1")
        .unwrap_or(false)
}

/// Simple hash function for content deduplication.
/// Uses FNV-1a hash algorithm for speed.
fn hash_string(s: &str) -> u64 {
    const FNV_OFFSET: u64 = 14695981039346656037;
    const FNV_PRIME: u64 = 1099511628211;

    let mut hash = FNV_OFFSET;
    for byte in s.as_bytes() {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_prompt_interval() {
        assert_eq!(PROMPT_INTERVAL, Duration::from_millis(500));
    }

    #[test]
    fn test_hash_string_consistency() {
        let s1 = "Do you want to continue?";
        let s2 = "Do you want to continue?";
        let s3 = "Do you want to stop?";

        assert_eq!(
            hash_string(s1),
            hash_string(s2),
            "Same strings should have same hash"
        );
        assert_ne!(
            hash_string(s1),
            hash_string(s3),
            "Different strings should have different hashes"
        );
    }

    #[test]
    fn test_hash_string_empty() {
        let empty = "";
        let hash = hash_string(empty);
        assert_eq!(
            hash, 14695981039346656037,
            "Empty string should return FNV offset"
        );
    }

    #[test]
    fn test_hash_string_deterministic() {
        // Hash should be deterministic across multiple invocations
        let text = "Some test content";
        let hash1 = hash_string(text);
        let hash2 = hash_string(text);
        let hash3 = hash_string(text);

        assert_eq!(hash1, hash2);
        assert_eq!(hash2, hash3);
    }

    #[test]
    fn test_hash_string_different_content() {
        // Even small changes should produce different hashes
        let h1 = hash_string("test");
        let h2 = hash_string("Test");
        let h3 = hash_string("test ");
        let h4 = hash_string(" test");

        assert_ne!(h1, h2, "Case change should change hash");
        assert_ne!(h1, h3, "Trailing space should change hash");
        assert_ne!(h1, h4, "Leading space should change hash");
    }

    #[test]
    fn test_tail_lines_config() {
        assert_eq!(
            TAIL_LINES, 30,
            "Should only search last 30 lines for efficiency"
        );
    }

    #[test]
    fn test_prompt_check_result_creation() {
        let id = SessionId::new();
        let result = PromptCheckResult {
            session_id: id,
            has_prompt: true,
            user_attached: false,
            content_hash: 12345,
        };

        assert_eq!(result.session_id, id);
        assert!(result.has_prompt);
        assert!(!result.user_attached);
        assert_eq!(result.content_hash, 12345);
    }

    #[test]
    fn test_prompt_detector_actor_creation() {
        let (tx, _rx) = mpsc::unbounded_channel();
        let session_info = Arc::new(RwLock::new(Vec::new()));

        let actor = PromptDetectorActor::new(tx, session_info, false);

        assert_eq!(actor.interval, PROMPT_INTERVAL);
        assert!(!actor.trust_enabled);
    }

    #[test]
    fn test_prompt_detector_actor_with_custom_interval() {
        let (tx, _rx) = mpsc::unbounded_channel();
        let session_info = Arc::new(RwLock::new(Vec::new()));

        let custom_interval = Duration::from_millis(1000);
        let actor = PromptDetectorActor::new(tx, session_info, true).with_interval(custom_interval);

        assert_eq!(actor.interval, custom_interval);
        assert!(actor.trust_enabled);
    }

    #[test]
    fn test_fnv_hash_collision_resistance() {
        // Test that FNV hash has good distribution for similar strings
        let inputs = vec![
            "session_1",
            "session_2",
            "session_3",
            "session1",
            "session2",
            "session3",
        ];

        let mut hashes = std::collections::HashSet::new();
        for input in inputs {
            let hash = hash_string(input);
            assert!(
                hashes.insert(hash),
                "Hash collision detected for input: {}",
                input
            );
        }
    }
}
