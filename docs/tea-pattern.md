# TEA Pattern: The Elm Architecture in Rust

> Pure state management with message-driven updates.

**Related docs:**
- [Game Loop Architecture](./game-loop-architecture.md) - Threading model
- [Actor System](./actors.md) - Background message sources
- [Architecture](./architecture.md) - Module overview

---

## What is TEA?

TEA (The Elm Architecture) is a pattern for managing application state:

```
                    ┌─────────────────┐
                    │     Message     │
                    │   (Input)       │
                    └────────┬────────┘
                             │
                             ▼
┌──────────────┐      ┌─────────────┐      ┌──────────────┐
│    Model     │─────►│   update()  │─────►│   Commands   │
│   (State)    │◄─────│             │      │   (Effects)  │
└──────────────┘      └─────────────┘      └──────────────┘
                             │
                             ▼
                    ┌─────────────────┐
                    │  RenderState    │
                    │  (View Data)    │
                    └─────────────────┘
```

**Core principle:** All state changes go through a single, pure `update` function.

---

## The Three Components

### 1. Model (State)

The Model is the single source of truth for application state:

```rust
// zen/src/tea/model.rs

pub struct Model {
    // Core state
    pub sessions: Vec<Session>,
    pub selected: usize,
    pub mode: Mode,
    pub preview_mode: PreviewMode,

    // Caches (updated by background actors)
    pub preview_cache: HashMap<SessionId, String>,
    pub diff_cache: HashMap<SessionId, (DiffStats, String)>,
    pub prompt_cache: HashMap<SessionId, PromptState>,

    // Input state
    pub input_buffer: String,
    pub error_message: Option<String>,
    pub pending_delete: Option<SessionId>,
    pub pending_session_name: Option<String>,

    // Dirty flag - triggers render when set
    pub dirty: bool,

    // Immutable config
    pub config: Config,
    pub repo_path: Option<PathBuf>,
    pub agent: Arc<dyn Agent>,
}
```

**Key design decisions:**

1. **Flat structure** - No deeply nested state
2. **Explicit caches** - Background data separated from core state
3. **Dirty flag** - Explicit render trigger
4. **No channels/handles** - Pure data, no runtime infrastructure

### 2. Message (Input)

Messages are inputs to the update function:

```rust
// zen/src/tea/message.rs

pub enum Message {
    // Keyboard/terminal events
    Key(KeyEvent),
    Resize(u16, u16),

    // From background actors
    PreviewUpdated(SessionId, String),
    DiffUpdated(SessionId, DiffStats),
    PromptDetected(SessionId, bool, bool),

    // Command completion callbacks
    SessionCreated(Session),
    SessionCreateFailed(String, String),
    SessionDeleted(SessionId),
    SessionPaused(SessionId),
    SessionResumed(SessionId),
    SessionPushed(SessionId, String),
    SessionPushFailed(SessionId, String),

    // State persistence
    StateSaved,
    StateSaveFailed(String),
}
```

**Message sources:**

| Source | Message Types |
|--------|---------------|
| Keyboard | `Key(KeyEvent)` |
| Terminal | `Resize(w, h)` |
| PreviewActor | `PreviewUpdated` |
| DiffActor | `DiffUpdated` |
| PromptDetectorActor | `PromptDetected` |
| Async task completion | `SessionCreated`, etc. |

### 3. Command (Output)

Commands are outputs from the update function - side effects to execute:

```rust
// zen/src/tea/command.rs

pub enum Command {
    // Terminal control
    AttachTmux { tmux_name: String },

    // Session operations (spawn async tasks)
    CreateSession { name: String, prompt: Option<String> },
    DeleteSession { id: SessionId },
    PauseSession { id: SessionId },
    ResumeSession { id: SessionId },
    PushSession { id: SessionId },

    // State persistence
    SaveState,

    // Actor coordination
    UpdateSessionInfo,

    // App lifecycle
    Quit,
}
```

**Commands vs Messages:**

| Aspect | Message | Command |
|--------|---------|---------|
| Direction | Input to update | Output from update |
| Source | External events | Update function |
| Purpose | Trigger state change | Request side effect |
| Execution | Immediate | Async/deferred |

---

## The Update Function

The heart of TEA - a pure function that processes messages:

```rust
// zen/src/tea/update.rs

/// Pure update function: Model + Message -> Commands
pub fn update(model: &mut Model, msg: Message) -> Vec<Command> {
    let mut cmds = Vec::new();

    match msg {
        Message::Key(key) => {
            model.error_message = None; // Clear error on any key
            model.dirty = true;         // Keyboard always triggers render

            match model.mode {
                Mode::List => update_list_mode(model, key, &mut cmds),
                Mode::Input(kind) => update_input_mode(model, key, kind, &mut cmds),
                Mode::Attached => update_attached_mode(model, key, &mut cmds),
            }
        }

        Message::PreviewUpdated(id, content) => {
            model.preview_cache.insert(id, content);
            // Only dirty if this affects visible state
            if model.selected_session().map(|s| s.id) == Some(id) {
                model.dirty = true;
            }
        }

        Message::SessionCreated(session) => {
            model.sessions.push(session);
            model.selected = model.sessions.len() - 1;
            model.dirty = true;
            cmds.push(Command::SaveState);
            cmds.push(Command::UpdateSessionInfo);
        }

        // ... other message handlers
    }

    cmds
}
```

**Properties of the update function:**

1. **Deterministic** - Same input always produces same output
2. **No I/O** - All side effects via returned Commands
3. **Explicit mutations** - Clear what state changes
4. **Testable** - Pure functions are easy to test

---

## Mode-Based State Machine

The application has three modes with distinct key bindings:

```
                    ┌─────────┐
                    │  List   │ <- Default state
                    └────┬────┘
                         │
         ┌───────────────┼───────────────┐
         │ 'n'           │ Enter         │ Esc/q
         ▼               ▼               ▼
    ┌─────────┐    ┌──────────┐    ┌──────────┐
    │  Input  │    │ Attached │    │   Quit   │
    │ (name)  │    │ (tmux)   │    │          │
    └────┬────┘    └──────────┘    └──────────┘
         │ Enter
         ▼
    ┌─────────┐
    │  Input  │
    │(prompt) │
    └─────────┘
```

### List Mode

Default mode - browsing sessions:

```rust
fn update_list_mode(model: &mut Model, key: KeyEvent, cmds: &mut Vec<Command>) {
    match key.code {
        // Navigation
        KeyCode::Char('j') | KeyCode::Down => {
            if !model.sessions.is_empty() {
                model.selected = (model.selected + 1) % model.sessions.len();
            }
        }
        KeyCode::Char('k') | KeyCode::Up => {
            if !model.sessions.is_empty() {
                model.selected = model.selected
                    .checked_sub(1)
                    .unwrap_or(model.sessions.len() - 1);
            }
        }

        // Session creation
        KeyCode::Char('n') => {
            model.mode = Mode::Input(InputKind::SessionName);
            model.input_buffer.clear();
        }

        // Attach to session
        KeyCode::Enter => {
            if let Some(session) = model.sessions.get(model.selected) {
                if session.status == SessionStatus::Running {
                    cmds.push(Command::AttachTmux {
                        tmux_name: session.tmux_name(),
                    });
                }
            }
        }

        // Toggle preview mode
        KeyCode::Tab => {
            model.preview_mode = match model.preview_mode {
                PreviewMode::Terminal => PreviewMode::Diff,
                PreviewMode::Diff => PreviewMode::Terminal,
            };
        }

        // Quit
        KeyCode::Char('q') | KeyCode::Esc => {
            cmds.push(Command::Quit);
        }

        _ => {}
    }
}
```

### Input Mode

Text entry for session creation:

```rust
fn update_input_mode(
    model: &mut Model,
    key: KeyEvent,
    kind: InputKind,
    cmds: &mut Vec<Command>
) {
    match key.code {
        KeyCode::Enter => {
            let input = std::mem::take(&mut model.input_buffer);
            model.mode = Mode::List;

            match kind {
                InputKind::SessionName => {
                    if !input.is_empty() {
                        model.mode = Mode::Input(InputKind::Prompt);
                        model.pending_session_name = Some(input);
                    }
                }
                InputKind::Prompt => {
                    let name = model.pending_session_name.take().unwrap_or_default();
                    let prompt = if input.is_empty() { None } else { Some(input) };
                    cmds.push(Command::CreateSession { name, prompt });
                }
                InputKind::Confirm => {
                    if input.to_lowercase() == "y" {
                        if let Some(id) = model.pending_delete.take() {
                            cmds.push(Command::DeleteSession { id });
                        }
                    }
                }
            }
        }

        KeyCode::Esc => {
            model.input_buffer.clear();
            model.pending_delete = None;
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
```

---

## Command Execution

Commands are executed outside the update function:

```rust
// zen/src/app.rs

async fn execute_command(
    model: &mut Model,
    cmd: Command,
    msg_tx: &mpsc::UnboundedSender<Message>,
    session_info: &Arc<RwLock<Vec<SessionInfo>>>,
) -> bool {
    match cmd {
        Command::CreateSession { name, prompt } => {
            let agent = model.agent.clone();
            let repo_path = model.repo_path.clone();
            let tx = msg_tx.clone();

            // Spawn async task - result comes back as Message
            tokio::spawn(async move {
                match Session::create(&name, &repo_path, agent.as_ref(), prompt.as_deref()).await {
                    Ok(session) => {
                        let _ = tx.send(Message::SessionCreated(session));
                    }
                    Err(e) => {
                        let _ = tx.send(Message::SessionCreateFailed(name, e.to_string()));
                    }
                }
            });
        }

        Command::SaveState => {
            let state = State::from_model(model);
            let tx = msg_tx.clone();
            tokio::spawn(async move {
                match state.save().await {
                    Ok(()) => { let _ = tx.send(Message::StateSaved); }
                    Err(e) => { let _ = tx.send(Message::StateSaveFailed(e.to_string())); }
                }
            });
        }

        Command::Quit => {
            return true; // Signal shutdown
        }

        // ... other commands
    }

    false
}
```

**Pattern:** Commands spawn async tasks that send Messages when complete.

---

## State Snapshots for Rendering

The Model creates immutable snapshots for the render thread:

```rust
// zen/src/tea/model.rs

impl Model {
    pub fn snapshot(&self) -> RenderState {
        let sessions: Vec<SessionView> = self.sessions
            .iter()
            .map(|s| SessionView {
                id: s.id,
                name: s.name.clone(),
                project: s.project.clone(),
                branch: s.branch.clone(),
                status: s.status,
                last_active: s.last_active,
                // ...
            })
            .collect();

        let preview = self.selected_session()
            .and_then(|s| self.preview_cache.get(&s.id))
            .cloned();

        RenderState {
            version: next_version(), // Monotonically increasing
            sessions,
            selected: self.selected,
            mode: self.mode,
            preview_mode: self.preview_mode,
            preview,
            diff: self.get_selected_diff(),
            input_buffer: self.input_buffer.clone(),
            error_message: self.error_message.clone(),
        }
    }
}
```

**Why snapshots?**

1. **Thread safety** - Immutable data can cross thread boundaries
2. **Decoupling** - Render doesn't need Model internals
3. **Versioning** - Change detection for render optimization

---

## Dirty Flag Pattern

The `dirty` flag controls when snapshots are sent:

```rust
// In logic thread loop
let cmds = update(&mut model, msg);

for cmd in cmds {
    execute_command(&mut model, cmd, &msg_tx).await;
}

// Only send state if something changed
if model.dirty {
    send_state(&state_tx, &model);
    model.dirty = false;
}
```

**Rules for setting dirty:**

| Event | Sets Dirty? | Reason |
|-------|-------------|--------|
| Keyboard input | Always | User expects immediate feedback |
| Resize | Always | Layout must update |
| PreviewUpdated (selected) | Yes | Visible content changed |
| PreviewUpdated (other) | No | Not currently visible |
| DiffUpdated (in diff mode) | Yes | Visible content changed |
| DiffUpdated (in terminal mode) | No | Not currently visible |
| SessionCreated | Always | List changed |
| Error | Always | User needs to see error |

---

## Testing the Update Function

Pure functions are trivial to test:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn test_model() -> Model {
        Model::new(vec![], Config::default(), None, Arc::new(MockAgent))
    }

    #[test]
    fn test_select_next_wraps() {
        let mut model = test_model_with_sessions(3);
        model.selected = 2; // Last item

        update(&mut model, Message::Key(key(KeyCode::Char('j'))));

        assert_eq!(model.selected, 0, "Should wrap to first");
    }

    #[test]
    fn test_n_key_starts_input() {
        let mut model = test_model();

        update(&mut model, Message::Key(key(KeyCode::Char('n'))));

        assert_eq!(model.mode, Mode::Input(InputKind::SessionName));
    }

    #[test]
    fn test_enter_creates_attach_command() {
        let mut model = test_model_with_sessions(1);

        let cmds = update(&mut model, Message::Key(key(KeyCode::Enter)));

        assert!(matches!(cmds[0], Command::AttachTmux { .. }));
    }

    #[test]
    fn test_keyboard_sets_dirty() {
        let mut model = test_model();
        model.dirty = false;

        update(&mut model, Message::Key(key(KeyCode::Char('j'))));

        assert!(model.dirty);
    }
}
```

---

## Message Flow Diagram

Complete flow from input to render:

```
  Keyboard Event
       │
       ▼
  ┌─────────────┐
  │   poll()    │  Synchronous, zero timeout
  └──────┬──────┘
         │
         ▼
  ┌─────────────┐
  │  Message::  │
  │  Key(event) │
  └──────┬──────┘
         │
         ▼
  ┌─────────────┐      ┌──────────────┐
  │  update()   │─────►│  Commands    │
  │             │      │  (Vec)       │
  └──────┬──────┘      └──────┬───────┘
         │                    │
         │ model.dirty = true │
         │                    ▼
         │             ┌──────────────┐
         │             │  execute()   │
         │             │  (async)     │
         │             └──────┬───────┘
         │                    │
         ▼                    │ spawns task
  ┌─────────────┐             │
  │  snapshot() │             ▼
  └──────┬──────┘      ┌──────────────┐
         │             │   Task       │
         ▼             │  completes   │
  ┌─────────────┐      └──────┬───────┘
  │ RenderState │             │
  │ (channel)   │             ▼
  └──────┬──────┘      ┌──────────────┐
         │             │  Message::   │
         ▼             │  SessionXxx  │
  ┌─────────────┐      └──────┬───────┘
  │ Render      │             │
  │ Thread      │             │
  └─────────────┘             │
                              ▼
                       (next loop iteration)
```

---

## Summary

TEA in Zen provides:

1. **Model** - Single source of truth, pure data
2. **Message** - All inputs as explicit events
3. **Command** - All side effects as explicit requests
4. **update()** - Pure function, deterministic, testable
5. **Snapshots** - Immutable data for rendering

Benefits:
- Predictable state transitions
- Easy debugging (log messages)
- Comprehensive testing
- Clean separation of concerns
