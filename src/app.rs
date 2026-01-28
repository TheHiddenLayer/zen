use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use crossbeam_channel::Sender;
use crossterm::event::{self, Event, KeyCode};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use tokio::runtime::Runtime;
use tokio::sync::{mpsc, RwLock};

use crate::actors::{ActorHandle, PreviewActor, PromptDetectorActor, SessionInfo};
use crate::agent::Agent;
use crate::config::Config;
use crate::git::GitOps;
use crate::render::RenderState;
use crate::session::{Session, State};
use crate::tea::{update, Command, Message, Model};
use crate::tmux::Tmux;
use crate::{zlog_debug, zlog_error, zlog_warn, Result};

const MAX_BG_MESSAGES: usize = 50;

pub struct LogicThread;

impl LogicThread {
    pub fn run(
        config: Config,
        state_tx: Sender<RenderState>,
        shutdown: Arc<AtomicBool>,
        render_paused: Arc<AtomicBool>,
        render_acked: Arc<AtomicBool>,
    ) -> Result<()> {
        Runtime::new()?.block_on(Self::run_async(
            config,
            state_tx,
            shutdown,
            render_paused,
            render_acked,
        ))
    }

    async fn run_async(
        config: Config,
        state_tx: Sender<RenderState>,
        shutdown: Arc<AtomicBool>,
        render_paused: Arc<AtomicBool>,
        render_acked: Arc<AtomicBool>,
    ) -> Result<()> {
        zlog_debug!(
            "LogicThread::run_async trust={} command={}",
            config.trust,
            config.effective_command()
        );
        let agent = Arc::new(Agent::from_config(&config));
        let trust_enabled = config.trust;
        let mut model = Model::load(config, agent.clone()).await?;
        zlog_debug!("Model loaded: {} sessions", model.sessions.len());

        let session_info = Arc::new(RwLock::new(build_session_info(
            &model.sessions,
            model.repo_path.as_deref(),
            &agent,
        )));
        let (msg_tx, mut msg_rx) = mpsc::unbounded_channel::<Message>();
        let actors = spawn_actors(msg_tx.clone(), session_info.clone(), trust_enabled);

        send_state(&state_tx, &model);
        let mut esc_filter = EscapeSequenceFilter::new();

        loop {
            if shutdown.load(Ordering::Relaxed) {
                break;
            }

            // Keyboard input (priority)
            while event::poll(Duration::ZERO)? {
                if let Event::Key(key) = event::read()? {
                    if let KeyCode::Char(c) = key.code {
                        if esc_filter.filter(c) {
                            continue;
                        }
                    }

                    for cmd in update(&mut model, Message::Key(key)) {
                        if execute_command(
                            &mut model,
                            cmd,
                            &msg_tx,
                            &session_info,
                            &render_paused,
                            &render_acked,
                        )
                        .await
                        {
                            shutdown.store(true, Ordering::Relaxed);
                            shutdown_actors(&actors);
                            save_state_sync(&model);
                            return Ok(());
                        }
                    }

                    if model.dirty {
                        send_state(&state_tx, &model);
                        model.dirty = false;
                    }
                }
            }

            // Background messages (bounded)
            for _ in 0..MAX_BG_MESSAGES {
                let Ok(msg) = msg_rx.try_recv() else { break };
                for cmd in update(&mut model, msg) {
                    if execute_command(
                        &mut model,
                        cmd,
                        &msg_tx,
                        &session_info,
                        &render_paused,
                        &render_acked,
                    )
                    .await
                    {
                        shutdown.store(true, Ordering::Relaxed);
                        shutdown_actors(&actors);
                        save_state_sync(&model);
                        return Ok(());
                    }
                }
            }

            if model.dirty {
                send_state(&state_tx, &model);
                model.dirty = false;
            }

            tokio::time::sleep(Duration::from_micros(500)).await;
        }

        shutdown_actors(&actors);
        save_state_sync(&model);
        Ok(())
    }
}

async fn execute_command(
    model: &mut Model,
    cmd: Command,
    msg_tx: &mpsc::UnboundedSender<Message>,
    session_info: &Arc<RwLock<Vec<SessionInfo>>>,
    render_paused: &Arc<AtomicBool>,
    render_acked: &Arc<AtomicBool>,
) -> bool {
    match cmd {
        Command::AttachTmux { tmux_name } => {
            zlog_debug!("Command::AttachTmux tmux_name={}", tmux_name);

            if Tmux::inside_tmux() {
                let _ = Tmux::attach(&tmux_name);
            } else {
                render_paused.store(true, Ordering::Release);
                while !render_acked.load(Ordering::Acquire) {
                    std::hint::spin_loop();
                }

                let _ = disable_raw_mode();
                let _ = execute!(std::io::stdout(), LeaveAlternateScreen);
                let _ = Tmux::attach(&tmux_name);
                let _ = enable_raw_mode();
                let _ = execute!(std::io::stdout(), EnterAlternateScreen);

                render_paused.store(false, Ordering::Release);
            }

            // When attach returns, user has already detached from tmux (via ctrl+q or closing popup)
            model.dirty = true;
        }

        Command::CreateSession { name, prompt } => {
            zlog_debug!(
                "Command::CreateSession name={} prompt={:?}",
                name,
                prompt
                    .as_ref()
                    .map(|s| s.chars().take(30).collect::<String>())
            );
            let Some(repo_path) = model.repo_path.clone() else {
                zlog_warn!("CreateSession failed: no git repository detected");
                let _ = msg_tx.send(Message::SessionCreateFailed(
                    name,
                    "No git repository detected".to_string(),
                ));
                return false;
            };

            let agent = model.agent.clone();
            let tx = msg_tx.clone();

            tokio::spawn(async move {
                match Session::create(&name, &repo_path, agent.as_ref(), prompt.as_deref()).await {
                    Ok(session) => {
                        zlog_debug!("Session create success: {}", name);
                        let _ = tx.send(Message::SessionCreated(session));
                    }
                    Err(e) => {
                        zlog_error!("Session create failed: {} - {}", name, e);
                        let _ = tx.send(Message::SessionCreateFailed(name, e.to_string()));
                    }
                }
            });
        }

        Command::DeleteSession { id } => {
            zlog_debug!("Command::DeleteSession id={}", id.short());
            let repo_path = model.repo_path.clone();
            let tx = msg_tx.clone();

            if let Some(pos) = model.sessions.iter().position(|s| s.id == id) {
                let session = model.sessions.remove(pos);
                let session_name = session.name.clone();

                // Adjust selection if needed
                if model.selected >= model.sessions.len() && model.selected > 0 {
                    model.selected -= 1;
                }

                tokio::spawn(async move {
                    if let Some(repo) = repo_path {
                        if let Err(e) = session.delete(&repo).await {
                            zlog_error!("Error during session deletion '{}': {}", session_name, e);
                        }
                    } else {
                        // No repo path - still try to clean up tmux session
                        let tmux_name = session.tmux_name();
                        let _ = crate::tmux::Tmux::kill_session(&tmux_name);
                        zlog_warn!("No repo path, only killed tmux session '{}'", tmux_name);
                    }
                    let _ = tx.send(Message::SessionDeleted(id));
                });
            }
        }

        Command::LockSession { id } => {
            zlog_debug!("Command::LockSession id={}", id.short());
            let tx = msg_tx.clone();

            if let Some(session) = model.sessions.iter_mut().find(|s| s.id == id) {
                if let Some(repo) = &model.repo_path {
                    if let Ok(git) = GitOps::new(repo) {
                        let result = session.lock(&git).await;
                        match result {
                            Ok(()) => {
                                zlog_debug!("Session lock success: id={}", id.short());
                                let _ = tx.send(Message::SessionLocked(id));
                            }
                            Err(e) => {
                                zlog_warn!("Session lock failed: id={} - {}", id.short(), e);
                                let _ = tx.send(Message::SessionLockFailed(id, e.to_string()));
                            }
                        }
                    }
                }
            }
        }

        Command::UnlockSession { id } => {
            zlog_debug!("Command::UnlockSession id={}", id.short());
            let repo_path = model.repo_path.clone();
            let agent = model.agent.clone();
            let tx = msg_tx.clone();

            if let Some(session) = model.sessions.iter_mut().find(|s| s.id == id) {
                if let Some(repo) = &repo_path {
                    if let Ok(git) = GitOps::new(repo) {
                        let result = session.unlock(&git, agent.as_ref()).await;
                        match result {
                            Ok(()) => {
                                zlog_debug!("Session unlock success: id={}", id.short());
                                let _ = tx.send(Message::SessionUnlocked(id));
                            }
                            Err(e) => {
                                let err_str = e.to_string();
                                zlog_warn!(
                                    "Session unlock failed: id={} - {}",
                                    id.short(),
                                    err_str
                                );
                                let _ = tx.send(Message::SessionUnlockFailed(id, err_str));
                            }
                        }
                    }
                }
            }
        }

        Command::SaveState => {
            zlog_debug!("Command::SaveState sessions={}", model.sessions.len());
            let state = State {
                version: 1,
                sessions: model.sessions.clone(),
            };
            let tx = msg_tx.clone();
            tokio::spawn(async move {
                match state.save().await {
                    Ok(()) => {
                        let _ = tx.send(Message::StateSaved);
                    }
                    Err(e) => {
                        zlog_error!("State save failed: {}", e);
                        let _ = tx.send(Message::StateSaveFailed(e.to_string()));
                    }
                }
            });
        }

        Command::UpdateSessionInfo => {
            zlog_debug!("Command::UpdateSessionInfo");
            let info =
                build_session_info(&model.sessions, model.repo_path.as_deref(), &model.agent);
            *session_info.write().await = info;
        }

        Command::SendEnterToSession { tmux_name } => {
            zlog_debug!("Command::SendEnterToSession tmux_name={}", tmux_name);
            // Trust mode: send Enter to approve a prompt
            // Re-check attachment state to prevent race condition
            let name = tmux_name.clone();
            tokio::spawn(async move {
                // Fast re-check: if user attached between detection and now, abort
                let is_attached = crate::util::blocking({
                    let n = name.clone();
                    move || Tmux::session_attached(&n)
                })
                .await
                .ok()
                .map(|s| s.trim() == "1")
                .unwrap_or(false);

                if is_attached {
                    zlog_debug!("Trust: skipped auto-approve for '{}' (user attached)", name);
                    return;
                }

                if let Err(e) =
                    crate::util::blocking(move || Tmux::send_keys_enter(&name, "")).await
                {
                    zlog_warn!("Trust: failed to send Enter to '{}': {}", tmux_name, e);
                } else {
                    zlog_debug!("Trust: auto-approved prompt for session");
                }
            });
        }

        Command::Quit => {
            zlog_debug!("Command::Quit");
            return true;
        }
    }

    false
}

fn send_state(state_tx: &Sender<RenderState>, model: &Model) {
    let _ = state_tx.try_send(model.snapshot());
}

fn save_state_sync(model: &Model) {
    let _ = State {
        version: 1,
        sessions: model.sessions.clone(),
    }
    .save_sync();
}

fn spawn_actors(
    msg_tx: mpsc::UnboundedSender<Message>,
    session_info: Arc<RwLock<Vec<SessionInfo>>>,
    trust_enabled: bool,
) -> Vec<ActorHandle> {
    zlog_debug!("Spawning actors: trust_enabled={}", trust_enabled);
    vec![
        PreviewActor::new(msg_tx.clone(), session_info.clone()).spawn(),
        PromptDetectorActor::new(msg_tx.clone(), session_info.clone(), trust_enabled).spawn(),
    ]
}

fn shutdown_actors(actors: &[ActorHandle]) {
    zlog_debug!("Shutting down {} actors", actors.len());
    for actor in actors {
        actor.shutdown();
    }
}

fn build_session_info(
    sessions: &[Session],
    repo_path: Option<&std::path::Path>,
    agent: &Agent,
) -> Vec<SessionInfo> {
    let prompt_pattern = agent.prompt_pattern();
    sessions
        .iter()
        .map(|s| SessionInfo {
            id: s.id,
            tmux_name: s.tmux_name(),
            repo_path: repo_path.map(|p| p.to_path_buf()),
            worktree_path: s.worktree_path.clone(),
            prompt_pattern: prompt_pattern.map(|p| p.to_string()),
        })
        .collect()
}

struct EscapeSequenceFilter {
    len: u8,
    active: bool,
}

impl EscapeSequenceFilter {
    fn new() -> Self {
        Self {
            len: 0,
            active: false,
        }
    }

    fn filter(&mut self, c: char) -> bool {
        if c == '\x1b' || c == '[' || c == 'O' {
            self.active = true;
            self.len = 1;
            return true;
        }
        if self.active {
            self.len += 1;
            if c.is_ascii_alphabetic() || c == '~' || self.len > 10 {
                self.active = false;
            }
            return true;
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

    #[test]
    fn test_escape_filter() {
        let mut filter = EscapeSequenceFilter::new();
        assert!(!filter.filter('a'));
        assert!(!filter.filter('b'));
    }

    #[test]
    fn test_escape_filter_sequence() {
        let mut filter = EscapeSequenceFilter::new();
        // Test escape sequence filtering
        assert!(filter.filter('\x1b')); // ESC
        assert!(filter.filter('[')); // CSI
        assert!(filter.filter('A')); // End of sequence
                                     // Next character should not be filtered
        assert!(!filter.filter('x'));
    }

    /// Test that the state channel (bounded(1) with try_send) never blocks.
    /// This is CRITICAL for the decoupled game loop architecture.
    #[test]
    fn test_state_channel_never_blocks() {
        let (tx, _rx) = crossbeam_channel::bounded::<RenderState>(1);

        // Fill the channel
        let state1 = RenderState::default();
        let _ = tx.try_send(state1);

        // Measure time to send when channel is full (should NOT block)
        let start = Instant::now();
        let state2 = RenderState::default();
        let result = tx.try_send(state2);
        let elapsed = start.elapsed();

        // Should complete in under 1ms (typically microseconds)
        assert!(
            elapsed.as_millis() < 1,
            "try_send blocked for {:?} - this breaks the decoupled architecture!",
            elapsed
        );

        // Result should be Err(Full), confirming old state was NOT dropped
        // (We're using try_send which doesn't drop - that's intentional)
        assert!(result.is_err());
    }

    /// Test the "latest-wins" pattern: when sender is faster than receiver,
    /// old states are dropped and only the latest is received.
    #[test]
    fn test_latest_wins_pattern() {
        let (tx, rx) = crossbeam_channel::bounded::<RenderState>(1);

        // Send multiple states rapidly
        for i in 0..5 {
            let mut state = RenderState::default();
            state.selected = i;
            // Drain and send to simulate latest-wins
            let _ = rx.try_recv();
            let _ = tx.try_send(state);
        }

        // Receiver should get the latest state
        let received = rx.try_recv().unwrap();
        assert_eq!(received.selected, 4, "Should receive the latest state");
    }

    /// Test that state snapshots have increasing version numbers.
    #[test]
    fn test_snapshot_versions_increase() {
        use crate::render::next_version;

        let v1 = next_version();
        let v2 = next_version();
        let v3 = next_version();

        assert!(v2 > v1, "Version should increase: {} > {}", v2, v1);
        assert!(v3 > v2, "Version should increase: {} > {}", v3, v2);
    }

    /// Test that the bounded channel capacity is exactly 1.
    /// This is important for the latest-wins semantics.
    #[test]
    fn test_channel_capacity_is_one() {
        let (tx, rx) = crossbeam_channel::bounded::<RenderState>(1);

        // First send should succeed
        assert!(tx.try_send(RenderState::default()).is_ok());

        // Second send should fail (channel full)
        assert!(tx.try_send(RenderState::default()).is_err());

        // After receiving, we can send again
        let _ = rx.try_recv();
        assert!(tx.try_send(RenderState::default()).is_ok());
    }
}
