//! Preview actor for capturing tmux pane content.

use std::sync::Arc;
use std::time::Duration;

use tokio::sync::{mpsc, RwLock};
use tokio_util::sync::CancellationToken;

use crate::tea::Message;
use crate::tmux::Tmux;
use crate::util::blocking_with_timeout;
use crate::{zlog_debug, zlog_trace};

use super::{ActorHandle, SessionInfo};

const PREVIEW_INTERVAL: Duration = Duration::from_millis(250);
const TMUX_TIMEOUT: Duration = Duration::from_millis(100);

/// Actor that periodically captures tmux pane content for live preview.
pub struct PreviewActor {
    msg_tx: mpsc::UnboundedSender<Message>,
    session_info: Arc<RwLock<Vec<SessionInfo>>>,
    interval: Duration,
}

impl PreviewActor {
    pub fn new(
        msg_tx: mpsc::UnboundedSender<Message>,
        session_info: Arc<RwLock<Vec<SessionInfo>>>,
    ) -> Self {
        Self {
            msg_tx,
            session_info,
            interval: PREVIEW_INTERVAL,
        }
    }

    pub fn with_interval(mut self, interval: Duration) -> Self {
        self.interval = interval;
        self
    }

    pub fn spawn(self) -> ActorHandle {
        let cancel = CancellationToken::new();
        let cancel_clone = cancel.clone();

        zlog_debug!("PreviewActor::spawn");

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(self.interval);

            loop {
                tokio::select! {
                    _ = cancel_clone.cancelled() => {
                        zlog_debug!("PreviewActor cancelled");
                        break;
                    }
                    _ = interval.tick() => {
                        if self.msg_tx.is_closed() {
                            zlog_debug!("PreviewActor: message channel closed");
                            break;
                        }

                        let infos = self.session_info.read().await.clone();
                        if infos.is_empty() {
                            continue;
                        }

                        zlog_trace!("PreviewActor: capturing {} panes", infos.len());

                        // Capture all panes in parallel (content + activity timestamp)
                        let mut handles = Vec::with_capacity(infos.len());
                        for info in &infos {
                            let id = info.id;
                            let tmux_name = info.tmux_name.clone();
                            handles.push(tokio::spawn(async move {
                                let content = capture_pane(&tmux_name).await;
                                let activity = pane_activity(&tmux_name).await;
                                (id, content, activity)
                            }));
                        }

                        // Collect results and send messages
                        for handle in handles {
                            if let Ok((id, Ok(content), activity)) = handle.await {
                                let activity_ts = activity.unwrap_or(0);
                                let _ = self.msg_tx.send(Message::PreviewUpdated(id, content, activity_ts));
                            }
                        }
                    }
                }
            }
        });

        ActorHandle::new(cancel)
    }
}

async fn capture_pane(session_name: &str) -> crate::Result<String> {
    let name = session_name.to_string();
    blocking_with_timeout(TMUX_TIMEOUT, move || Tmux::capture_pane(&name)).await
}

async fn pane_activity(session_name: &str) -> crate::Result<u64> {
    let name = session_name.to_string();
    blocking_with_timeout(TMUX_TIMEOUT, move || Tmux::pane_activity(&name)).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_preview_interval() {
        assert_eq!(PREVIEW_INTERVAL, Duration::from_millis(250));
    }
}
