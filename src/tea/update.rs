//! Pure update function for the TEA (The Elm Architecture) pattern.
//!
//! The update function takes a model and a message, mutates the model,
//! and returns a list of commands to execute.

use crossterm::event::{KeyCode, KeyEvent};

use crate::session::SessionStatus;
use crate::{zlog, zlog_debug, zlog_warn};

use super::command::Command;
use super::message::Message;
use super::model::{InputKind, Mode, Model, Notification, NotificationLevel, PromptState};

/// Helper to set an error notification and mark model as dirty.
fn set_error(model: &mut Model, message: String) {
    zlog_warn!("UI Error: {}", message);
    model.notification = Some(Notification {
        level: NotificationLevel::Error,
        message,
    });
    model.dirty = true;
}

/// Pure update function: Model + Message → Commands
///
/// This function:
/// 1. Takes the current model and an input message
/// 2. Mutates the model state (and sets dirty flag)
/// 3. Returns a list of commands (side effects) to execute
///
/// The function itself has no side effects - all I/O happens via returned Commands.
pub fn update(model: &mut Model, msg: Message) -> Vec<Command> {
    let mut cmds = Vec::new();

    match msg {
        Message::Key(key) => {
            model.notification = None; // Clear notification on any key press
            model.dirty = true; // Keyboard input always triggers render
            match model.mode {
                Mode::List => update_list_mode(model, key, &mut cmds),
                Mode::Input(kind) => update_input_mode(model, key, kind, &mut cmds),
            }
        }

        Message::Resize(_, _) => {
            model.dirty = true; // Resize triggers re-render
        }

        // Background actor updates
        Message::PreviewUpdated(id, content, activity_ts) => {
            use std::time::{Duration, Instant};

            model.preview_cache.insert(id, content);

            // Activity tracking: compare consecutive timestamps to detect output
            // - First observation: store timestamp only (is_active stays None = "Unknown")
            // - Second+ observation: if timestamp changed → active, else → idle
            if let Some((prev_ts, prev_last_change)) = model.activity_cache.get(&id).copied() {
                // We have a previous observation to compare against
                let last_change = if activity_ts != prev_ts {
                    // Timestamp changed - session is actively outputting
                    Instant::now()
                } else {
                    // No change - keep previous last_change time
                    prev_last_change
                };
                model.activity_cache.insert(id, (activity_ts, last_change));
            } else {
                // First observation - store timestamp but use old time so we start as Ready
                // (not Busy). Next poll will detect any changes.
                let initial_last_change = Instant::now() - Duration::from_secs(2);
                model
                    .activity_cache
                    .insert(id, (activity_ts, initial_last_change));
            }

            // Always dirty - activity state may have changed due to grace period expiring
            model.dirty = true;
        }

        Message::PromptDetected(id, has_prompt, user_attached) => {
            model.prompt_cache.insert(
                id,
                PromptState {
                    has_prompt,
                    user_attached,
                },
            );
            // Prompt state doesn't change visual display directly
        }

        Message::AutoApprovePrompt(id) => {
            // Trust mode: send Enter to the session to approve the prompt
            if let Some(session) = model.sessions.iter().find(|s| s.id == id) {
                cmds.push(Command::SendEnterToSession {
                    tmux_name: session.tmux_name(),
                });
            }
        }

        // Command completion callbacks
        Message::SessionCreated(session) => {
            zlog!(
                "Message::SessionCreated name={} id={}",
                session.name,
                session.id.short()
            );
            model.sessions.push(session);
            model.selected = model.sessions.len() - 1;
            model.dirty = true;
            cmds.push(Command::SaveState);
            cmds.push(Command::UpdateSessionInfo);
        }

        Message::SessionCreateFailed(name, err) => {
            zlog_warn!("Message::SessionCreateFailed name={} err={}", name, err);
            set_error(model, format!("Failed to create '{}': {}", name, err));
        }

        Message::SessionDeleted(id) => {
            zlog!("Message::SessionDeleted id={}", id.short());
            model.preview_cache.remove(&id);
            model.prompt_cache.remove(&id);
            model.dirty = true;
            cmds.push(Command::SaveState);
            cmds.push(Command::UpdateSessionInfo);
        }

        Message::SessionLocked(id) => {
            zlog_debug!("Message::SessionLocked id={}", id.short());
            if let Some(session) = model.sessions.iter_mut().find(|s| s.id == id) {
                session.status = SessionStatus::Locked;
            }
            model.dirty = true;
            cmds.push(Command::SaveState);
        }

        Message::SessionLockFailed(_id, err) => {
            zlog_warn!("Message::SessionLockFailed err={}", err);
            let message = if err.contains("does not exist") {
                "Cannot lock: worktree path does not exist.".to_string()
            } else if err.contains("dirty") || err.contains("uncommitted") {
                "Cannot lock: uncommitted changes.".to_string()
            } else {
                format!("Cannot lock: {}", err)
            };
            set_error(model, message);
        }

        Message::SessionUnlocked(id) => {
            zlog_debug!("Message::SessionUnlocked id={}", id.short());
            if let Some(session) = model.sessions.iter_mut().find(|s| s.id == id) {
                session.status = SessionStatus::Running;
            }
            model.dirty = true;
            cmds.push(Command::SaveState);
        }

        Message::SessionUnlockFailed(_id, err) => {
            zlog_warn!("Message::SessionUnlockFailed err={}", err);
            let message = if err.contains("already checked out") || err.contains("is already used")
            {
                "Cannot unlock: currently checked out.".to_string()
            } else if err.contains("no longer exists")
                || err.contains("not found")
                || err.contains("NotFound")
            {
                "Cannot unlock: branch no longer exists.".to_string()
            } else {
                format!("Cannot unlock: {}", err)
            };
            set_error(model, message);
        }

        Message::StateSaved => {
            zlog_debug!("Message::StateSaved");
        }

        Message::StateSaveFailed(err) => {
            zlog_warn!("Message::StateSaveFailed err={}", err);
            set_error(model, format!("Failed to save state: {}", err));
        }
    }

    cmds
}

fn update_list_mode(model: &mut Model, key: KeyEvent, cmds: &mut Vec<Command>) {
    match key.code {
        KeyCode::Char('j') | KeyCode::Down => {
            if !model.sessions.is_empty() {
                model.selected = (model.selected + 1) % model.sessions.len();
            }
        }

        KeyCode::Char('k') | KeyCode::Up => {
            if !model.sessions.is_empty() {
                model.selected = model
                    .selected
                    .checked_sub(1)
                    .unwrap_or(model.sessions.len() - 1);
            }
        }

        KeyCode::Char('n') => {
            model.mode = Mode::Input(InputKind::SessionName);
            model.input_buffer.clear();
        }

        KeyCode::Char('o') => {
            // Open/attach to session
            if let Some(session) = model.sessions.get(model.selected) {
                if session.status == SessionStatus::Running {
                    cmds.push(Command::AttachTmux {
                        tmux_name: session.tmux_name(),
                    });
                }
            }
        }

        KeyCode::Char('d') => {
            if let Some(session) = model.sessions.get(model.selected) {
                model.pending_delete = Some(session.id);
                model.mode = Mode::Input(InputKind::Confirm);
                model.input_buffer.clear();
            }
        }

        KeyCode::Char('l') => {
            if let Some(session) = model.sessions.get(model.selected) {
                let id = session.id;
                match session.status {
                    SessionStatus::Running => cmds.push(Command::LockSession { id }),
                    SessionStatus::Locked => cmds.push(Command::UnlockSession { id }),
                }
            }
        }

        KeyCode::Char('q') | KeyCode::Esc => {
            cmds.push(Command::Quit);
        }

        KeyCode::Char('?') => {
            model.show_keymap = !model.show_keymap;
        }

        _ => {}
    }
}

fn update_input_mode(model: &mut Model, key: KeyEvent, kind: InputKind, cmds: &mut Vec<Command>) {
    match key.code {
        KeyCode::Enter => {
            // Store current field value before submitting
            store_current_field(model, kind);
            model.input_buffer.clear();
            model.mode = Mode::List;

            match kind {
                InputKind::SessionName | InputKind::Prompt => {
                    // Submit session with whatever values we have
                    let name = model.pending_session_name.take().unwrap_or_default();
                    if !name.is_empty() {
                        let prompt = model.pending_prompt.take();
                        cmds.push(Command::CreateSession { name, prompt });
                    }
                }
                InputKind::Confirm => {
                    if let Some(id) = model.pending_delete.take() {
                        cmds.push(Command::DeleteSession { id });
                    }
                }
            }
        }

        KeyCode::Tab => {
            // Cycle to next input field (store current, load next)
            if let Some(next_kind) = kind.next() {
                store_current_field(model, kind);
                model.mode = Mode::Input(next_kind);
                load_field_buffer(model, next_kind);
            }
        }

        KeyCode::Esc => {
            model.input_buffer.clear();
            model.pending_delete = None;
            model.pending_session_name = None;
            model.pending_prompt = None;
            model.mode = Mode::List;
        }

        KeyCode::Backspace => {
            model.input_buffer.pop();
        }

        KeyCode::Char(c) => {
            model.input_buffer.push(c);
        }

        _ => {}
    }
}

/// Store current input buffer into the appropriate pending field.
fn store_current_field(model: &mut Model, kind: InputKind) {
    let value = std::mem::take(&mut model.input_buffer);
    let value_opt = if value.is_empty() { None } else { Some(value) };
    match kind {
        InputKind::SessionName => model.pending_session_name = value_opt,
        InputKind::Prompt => model.pending_prompt = value_opt,
        InputKind::Confirm => {}
    }
}

/// Load the appropriate pending field into input buffer.
fn load_field_buffer(model: &mut Model, kind: InputKind) {
    model.input_buffer = match kind {
        InputKind::SessionName => model.pending_session_name.clone().unwrap_or_default(),
        InputKind::Prompt => model.pending_prompt.clone().unwrap_or_default(),
        InputKind::Confirm => String::new(),
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::Agent;
    use crate::config::Config;
    use crate::session::{Session, SessionId};
    use crossterm::event::KeyModifiers;
    use std::sync::Arc;

    /// Create a test model.
    fn test_model() -> Model {
        Model::new(vec![], Config::default(), None, Arc::new(Agent::default()))
    }

    /// Create a test model with sessions.
    fn test_model_with_sessions(count: usize) -> Model {
        let now = chrono::Utc::now();
        let sessions: Vec<Session> = (0..count)
            .map(|i| Session {
                id: SessionId::new(),
                name: format!("session-{}", i),
                project: "test-project".to_string(),
                branch: format!("branch-{}", i),
                base_branch: "main".to_string(),
                base_commit: "abc123".to_string(),
                agent: "claude".to_string(),
                status: SessionStatus::Running,
                created_at: now,
                last_active: now,
                worktree_path: None,
            })
            .collect();
        Model::new(
            sessions,
            Config::default(),
            None,
            Arc::new(Agent::default()),
        )
    }

    /// Helper to create a key event.
    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::empty())
    }

    // ═══════════════════════════════════════════════════════════════════════
    // Navigation Tests - Verify list mode navigation
    // ═══════════════════════════════════════════════════════════════════════

    #[test]
    fn test_select_next_wraps() {
        let mut model = test_model_with_sessions(3);
        model.selected = 2; // Last item

        update(&mut model, Message::Key(key(KeyCode::Char('j'))));
        assert_eq!(model.selected, 0, "Selection should wrap to first item");
    }

    #[test]
    fn test_select_prev_wraps() {
        let mut model = test_model_with_sessions(3);
        model.selected = 0; // First item

        update(&mut model, Message::Key(key(KeyCode::Char('k'))));
        assert_eq!(model.selected, 2, "Selection should wrap to last item");
    }

    #[test]
    fn test_navigation_empty_list() {
        let mut model = test_model();

        // Should not panic with empty list
        update(&mut model, Message::Key(key(KeyCode::Char('j'))));
        assert_eq!(model.selected, 0);

        update(&mut model, Message::Key(key(KeyCode::Char('k'))));
        assert_eq!(model.selected, 0);
    }

    #[test]
    fn test_arrow_keys_navigation() {
        let mut model = test_model_with_sessions(3);

        update(&mut model, Message::Key(key(KeyCode::Down)));
        assert_eq!(model.selected, 1);

        update(&mut model, Message::Key(key(KeyCode::Up)));
        assert_eq!(model.selected, 0);
    }

    // ═══════════════════════════════════════════════════════════════════════
    // Mode Transition Tests - Verify mode changes
    // ═══════════════════════════════════════════════════════════════════════

    #[test]
    fn test_n_key_starts_session_name_input() {
        let mut model = test_model();

        update(&mut model, Message::Key(key(KeyCode::Char('n'))));
        assert_eq!(model.mode, Mode::Input(InputKind::SessionName));
        assert!(model.input_buffer.is_empty());
    }

    #[test]
    fn test_d_key_starts_confirm_mode() {
        let mut model = test_model_with_sessions(1);

        update(&mut model, Message::Key(key(KeyCode::Char('d'))));
        assert_eq!(model.mode, Mode::Input(InputKind::Confirm));
        assert!(model.pending_delete.is_some());
    }

    #[test]
    fn test_esc_cancels_input_mode() {
        let mut model = test_model();
        model.mode = Mode::Input(InputKind::SessionName);
        model.input_buffer = "test".to_string();

        update(&mut model, Message::Key(key(KeyCode::Esc)));
        assert_eq!(model.mode, Mode::List);
        assert!(model.input_buffer.is_empty());
    }

    // ═══════════════════════════════════════════════════════════════════════
    // Input Mode Tests - Verify text entry behavior
    // ═══════════════════════════════════════════════════════════════════════

    #[test]
    fn test_input_buffer_accepts_characters() {
        let mut model = test_model();
        model.mode = Mode::Input(InputKind::SessionName);

        update(&mut model, Message::Key(key(KeyCode::Char('t'))));
        update(&mut model, Message::Key(key(KeyCode::Char('e'))));
        update(&mut model, Message::Key(key(KeyCode::Char('s'))));
        update(&mut model, Message::Key(key(KeyCode::Char('t'))));

        assert_eq!(model.input_buffer, "test");
    }

    #[test]
    fn test_backspace_removes_characters() {
        let mut model = test_model();
        model.mode = Mode::Input(InputKind::SessionName);
        model.input_buffer = "test".to_string();

        update(&mut model, Message::Key(key(KeyCode::Backspace)));
        assert_eq!(model.input_buffer, "tes");

        update(&mut model, Message::Key(key(KeyCode::Backspace)));
        assert_eq!(model.input_buffer, "te");
    }

    #[test]
    fn test_tab_cycles_from_name_to_prompt() {
        let mut model = test_model();
        model.mode = Mode::Input(InputKind::SessionName);
        model.input_buffer = "my-session".to_string();

        update(&mut model, Message::Key(key(KeyCode::Tab)));
        assert_eq!(model.mode, Mode::Input(InputKind::Prompt));
        assert_eq!(model.pending_session_name, Some("my-session".to_string()));
        assert!(model.input_buffer.is_empty());
    }

    #[test]
    fn test_tab_cycles_from_prompt_to_name() {
        let mut model = test_model();
        model.mode = Mode::Input(InputKind::Prompt);
        model.pending_session_name = Some("my-session".to_string());
        model.input_buffer = "fix bugs".to_string();

        update(&mut model, Message::Key(key(KeyCode::Tab)));
        assert_eq!(model.mode, Mode::Input(InputKind::SessionName));
        assert_eq!(model.pending_prompt, Some("fix bugs".to_string()));
        assert_eq!(model.input_buffer, "my-session");
    }

    #[test]
    fn test_enter_in_name_creates_session() {
        let mut model = test_model();
        model.mode = Mode::Input(InputKind::SessionName);
        model.input_buffer = "my-session".to_string();

        let cmds = update(&mut model, Message::Key(key(KeyCode::Enter)));
        assert_eq!(model.mode, Mode::List);
        assert_eq!(cmds.len(), 1);
        match &cmds[0] {
            Command::CreateSession { name, prompt } => {
                assert_eq!(name, "my-session");
                assert_eq!(prompt, &None);
            }
            _ => panic!("Expected CreateSession command"),
        }
    }

    #[test]
    fn test_empty_session_name_no_create() {
        let mut model = test_model();
        model.mode = Mode::Input(InputKind::SessionName);

        let cmds = update(&mut model, Message::Key(key(KeyCode::Enter)));
        assert_eq!(model.mode, Mode::List);
        assert!(cmds.is_empty(), "Should not create session with empty name");
    }

    // ═══════════════════════════════════════════════════════════════════════
    // Command Generation Tests - Verify commands are created correctly
    // ═══════════════════════════════════════════════════════════════════════

    #[test]
    fn test_o_creates_attach_command() {
        let mut model = test_model_with_sessions(1);

        let cmds = update(&mut model, Message::Key(key(KeyCode::Char('o'))));
        assert_eq!(cmds.len(), 1);
        assert!(matches!(cmds[0], Command::AttachTmux { .. }));
    }

    #[test]
    fn test_o_on_locked_session_no_attach() {
        let mut model = test_model_with_sessions(1);
        model.sessions[0].status = SessionStatus::Locked;

        let cmds = update(&mut model, Message::Key(key(KeyCode::Char('o'))));
        assert!(cmds.is_empty(), "Should not attach to locked session");
    }

    #[test]
    fn test_q_creates_quit_command() {
        let mut model = test_model();

        let cmds = update(&mut model, Message::Key(key(KeyCode::Char('q'))));
        assert_eq!(cmds.len(), 1);
        assert!(matches!(cmds[0], Command::Quit));
    }

    #[test]
    fn test_esc_in_list_creates_quit() {
        let mut model = test_model();

        let cmds = update(&mut model, Message::Key(key(KeyCode::Esc)));
        assert!(matches!(cmds[0], Command::Quit));
    }

    #[test]
    fn test_l_toggles_lock_unlock() {
        let mut model = test_model_with_sessions(1);

        // Running -> Lock
        let cmds = update(&mut model, Message::Key(key(KeyCode::Char('l'))));
        assert!(matches!(cmds[0], Command::LockSession { .. }));

        // Simulate lock completion
        model.sessions[0].status = SessionStatus::Locked;

        // Locked -> Unlock
        let cmds = update(&mut model, Message::Key(key(KeyCode::Char('l'))));
        assert!(matches!(cmds[0], Command::UnlockSession { .. }));
    }

    #[test]
    fn test_prompt_enter_creates_session() {
        let mut model = test_model();
        model.mode = Mode::Input(InputKind::Prompt);
        model.pending_session_name = Some("test-session".to_string());
        model.input_buffer = "fix bugs".to_string();

        let cmds = update(&mut model, Message::Key(key(KeyCode::Enter)));
        assert_eq!(cmds.len(), 1);
        match &cmds[0] {
            Command::CreateSession { name, prompt } => {
                assert_eq!(name, "test-session");
                assert_eq!(prompt, &Some("fix bugs".to_string()));
            }
            _ => panic!("Expected CreateSession command"),
        }
    }

    // ═══════════════════════════════════════════════════════════════════════
    // Dirty Flag Tests - Verify render triggers are set correctly
    // ═══════════════════════════════════════════════════════════════════════

    #[test]
    fn test_keyboard_sets_dirty_flag() {
        let mut model = test_model();
        model.dirty = false;

        update(&mut model, Message::Key(key(KeyCode::Char('j'))));
        assert!(model.dirty, "Keyboard input should set dirty flag");
    }

    #[test]
    fn test_resize_sets_dirty_flag() {
        let mut model = test_model();
        model.dirty = false;

        update(&mut model, Message::Resize(80, 24));
        assert!(model.dirty, "Resize should set dirty flag");
    }

    #[test]
    fn test_keyboard_clears_notification() {
        let mut model = test_model();
        model.notification = Some(Notification {
            level: NotificationLevel::Error,
            message: "Previous error".to_string(),
        });

        update(&mut model, Message::Key(key(KeyCode::Char('j'))));
        assert!(
            model.notification.is_none(),
            "Keyboard should clear notification"
        );
    }

    // ═══════════════════════════════════════════════════════════════════════
    // Session Message Tests - Verify background message handling
    // ═══════════════════════════════════════════════════════════════════════

    #[test]
    fn test_session_created_adds_to_list() {
        let mut model = test_model();
        let now = chrono::Utc::now();
        let session = Session {
            id: SessionId::new(),
            name: "new-session".to_string(),
            project: "project".to_string(),
            branch: "feature".to_string(),
            base_branch: "main".to_string(),
            base_commit: "abc".to_string(),
            agent: "claude".to_string(),
            status: SessionStatus::Running,
            created_at: now,
            last_active: now,
            worktree_path: None,
        };

        let cmds = update(&mut model, Message::SessionCreated(session));

        assert_eq!(model.sessions.len(), 1);
        assert_eq!(model.selected, 0);
        assert!(model.dirty);
        assert!(cmds.iter().any(|c| matches!(c, Command::SaveState)));
    }

    #[test]
    fn test_session_create_failed_shows_error() {
        let mut model = test_model();

        update(
            &mut model,
            Message::SessionCreateFailed("test".to_string(), "Network error".to_string()),
        );

        assert!(model.notification.is_some());
        let notification = model.notification.as_ref().unwrap();
        assert_eq!(notification.level, NotificationLevel::Error);
        assert!(notification.message.contains("test"));
        assert!(model.dirty);
    }

    #[test]
    fn test_preview_update_always_dirtys() {
        let mut model = test_model_with_sessions(2);
        let selected_id = model.sessions[0].id;
        let other_id = model.sessions[1].id;

        // Update for selected session - should dirty
        model.dirty = false;
        update(
            &mut model,
            Message::PreviewUpdated(selected_id, "content".to_string(), 1000),
        );
        assert!(model.dirty, "Preview update should dirty");

        // Update for other session - also dirtys (activity state tracked)
        model.dirty = false;
        update(
            &mut model,
            Message::PreviewUpdated(other_id, "other content".to_string(), 1000),
        );
        assert!(model.dirty, "Preview update should dirty");
    }

    // ═══════════════════════════════════════════════════════════════════════
    // Keymap Toggle Tests - Verify '?' toggles keymap visibility
    // ═══════════════════════════════════════════════════════════════════════

    #[test]
    fn test_question_mark_toggles_keymap() {
        let mut model = test_model();
        assert!(!model.show_keymap, "Keymap should be hidden by default");

        // First press: show keymap
        update(&mut model, Message::Key(key(KeyCode::Char('?'))));
        assert!(
            model.show_keymap,
            "Keymap should be visible after first '?'"
        );

        // Second press: hide keymap
        update(&mut model, Message::Key(key(KeyCode::Char('?'))));
        assert!(
            !model.show_keymap,
            "Keymap should be hidden after second '?'"
        );
    }

    #[test]
    fn test_question_mark_only_works_in_list_mode() {
        let mut model = test_model();
        model.mode = Mode::Input(InputKind::SessionName);

        // '?' in input mode should be treated as text input, not toggle
        update(&mut model, Message::Key(key(KeyCode::Char('?'))));
        assert!(
            !model.show_keymap,
            "Keymap toggle should not work in input mode"
        );
        assert_eq!(
            model.input_buffer, "?",
            "'?' should be added to input buffer"
        );
    }

    // ═══════════════════════════════════════════════════════════════════════
    // Trust Mode Tests - Verify auto-approve message handling
    // ═══════════════════════════════════════════════════════════════════════

    #[test]
    fn test_auto_approve_prompt_emits_send_enter_command() {
        let mut model = test_model_with_sessions(1);
        let id = model.sessions[0].id;

        let cmds = update(&mut model, Message::AutoApprovePrompt(id));

        assert_eq!(cmds.len(), 1);
        match &cmds[0] {
            Command::SendEnterToSession { tmux_name } => {
                assert!(tmux_name.starts_with("zen_"));
            }
            _ => panic!("Expected SendEnterToSession command"),
        }
    }

    #[test]
    fn test_auto_approve_prompt_nonexistent_session_no_command() {
        let mut model = test_model_with_sessions(1);
        let fake_id = SessionId::new(); // Different from any session

        let cmds = update(&mut model, Message::AutoApprovePrompt(fake_id));

        assert!(
            cmds.is_empty(),
            "Should not emit command for nonexistent session"
        );
    }

    #[test]
    fn test_auto_approve_prompt_multiple_sessions_correct_target() {
        let mut model = test_model_with_sessions(3);
        let target_id = model.sessions[1].id;
        let expected_name = model.sessions[1].tmux_name();

        let cmds = update(&mut model, Message::AutoApprovePrompt(target_id));

        assert_eq!(cmds.len(), 1);
        match &cmds[0] {
            Command::SendEnterToSession { tmux_name } => {
                assert_eq!(tmux_name, &expected_name);
            }
            _ => panic!("Expected SendEnterToSession command"),
        }
    }

    #[test]
    fn test_prompt_detected_updates_cache() {
        let mut model = test_model_with_sessions(1);
        let id = model.sessions[0].id;

        update(&mut model, Message::PromptDetected(id, true, false));

        let state = model.prompt_cache.get(&id).unwrap();
        assert!(state.has_prompt);
        assert!(!state.user_attached);
    }

    #[test]
    fn test_prompt_detected_user_attached_updates_cache() {
        let mut model = test_model_with_sessions(1);
        let id = model.sessions[0].id;

        update(&mut model, Message::PromptDetected(id, true, true));

        let state = model.prompt_cache.get(&id).unwrap();
        assert!(state.has_prompt);
        assert!(state.user_attached);
    }

    // ═══════════════════════════════════════════════════════════════════════
    // Notification Routing Tests
    // ═══════════════════════════════════════════════════════════════════════

    #[test]
    fn test_session_lock_failed_creates_error_notification() {
        let mut model = test_model_with_sessions(1);
        let id = model.sessions[0].id;

        update(
            &mut model,
            Message::SessionLockFailed(id, "worktree does not exist".to_string()),
        );

        assert!(model.notification.is_some());
        let notification = model.notification.as_ref().unwrap();
        assert_eq!(notification.level, NotificationLevel::Error);
        assert!(notification.message.contains("does not exist"));
        assert!(model.dirty);
    }

    #[test]
    fn test_session_unlock_failed_creates_error_notification() {
        let mut model = test_model_with_sessions(1);
        let id = model.sessions[0].id;

        update(
            &mut model,
            Message::SessionUnlockFailed(id, "branch no longer exists".to_string()),
        );

        assert!(model.notification.is_some());
        let notification = model.notification.as_ref().unwrap();
        assert_eq!(notification.level, NotificationLevel::Error);
        assert!(notification.message.contains("no longer exists"));
        assert!(model.dirty);
    }

    #[test]
    fn test_state_save_failed_creates_error_notification() {
        let mut model = test_model();

        update(
            &mut model,
            Message::StateSaveFailed("disk full".to_string()),
        );

        assert!(model.notification.is_some());
        let notification = model.notification.as_ref().unwrap();
        assert_eq!(notification.level, NotificationLevel::Error);
        assert!(notification.message.contains("Failed to save state"));
        assert!(notification.message.contains("disk full"));
        assert!(model.dirty);
    }

    #[test]
    fn test_keypress_clears_notification_regardless_of_level() {
        let mut model = test_model();

        // Test with error notification
        model.notification = Some(Notification {
            level: NotificationLevel::Error,
            message: "Error message".to_string(),
        });
        update(&mut model, Message::Key(key(KeyCode::Char('j'))));
        assert!(
            model.notification.is_none(),
            "Keypress should clear error notification"
        );

        // Test with info notification
        model.notification = Some(Notification {
            level: NotificationLevel::Info,
            message: "Info message".to_string(),
        });
        update(&mut model, Message::Key(key(KeyCode::Char('k'))));
        assert!(
            model.notification.is_none(),
            "Keypress should clear info notification"
        );
    }

    #[test]
    fn test_snapshot_includes_notification() {
        let mut model = test_model();

        // Test with no notification
        let snapshot = model.snapshot();
        assert!(snapshot.notification.is_none());

        // Test with error notification
        model.notification = Some(Notification {
            level: NotificationLevel::Error,
            message: "Test error".to_string(),
        });
        let snapshot = model.snapshot();
        assert!(snapshot.notification.is_some());
        let notification = snapshot.notification.as_ref().unwrap();
        assert_eq!(notification.level, NotificationLevel::Error);
        assert_eq!(notification.message, "Test error");

        // Test with info notification
        model.notification = Some(Notification {
            level: NotificationLevel::Info,
            message: "Test info".to_string(),
        });
        let snapshot = model.snapshot();
        assert!(snapshot.notification.is_some());
        let notification = snapshot.notification.as_ref().unwrap();
        assert_eq!(notification.level, NotificationLevel::Info);
        assert_eq!(notification.message, "Test info");
    }
}
