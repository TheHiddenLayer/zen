# Actor System

> Background tasks that never block the UI.

**Related docs:**
- [Game Loop Architecture](./game-loop-architecture.md) - Threading model
- [TEA Pattern](./tea-pattern.md) - Message handling
- [Async Patterns](./async-patterns.md) - Tokio integration

---

## Overview

Actors are independent background tasks that:

1. Run on fixed intervals (polling)
2. Communicate via message passing
3. Never block the logic thread
4. Can be gracefully cancelled

```
┌─────────────────────────────────────────────────────────────────┐
│                        Logic Thread                              │
│                                                                  │
│   ┌──────────────────────────────────────────────────────────┐  │
│   │                  Message Channel (mpsc)                   │  │
│   └──────────────────────────────────────────────────────────┘  │
│          ▲               ▲               ▲                       │
│          │               │               │                       │
│   ┌──────┴─────┐  ┌──────┴─────┐  ┌──────┴─────┐                │
│   │  Preview   │  │    Diff    │  │   Prompt   │                │
│   │   Actor    │  │   Actor    │  │  Detector  │                │
│   │  (250ms)   │  │  (1000ms)  │  │  (500ms)   │                │
│   └────────────┘  └────────────┘  └────────────┘                │
│          │               │               │                       │
│          ▼               ▼               ▼                       │
│   ┌────────────────────────────────────────────────────────────┐│
│   │              SessionInfo (Arc<RwLock<Vec>>)                ││
│   │  - Session IDs, tmux names, paths, prompt patterns         ││
│   └────────────────────────────────────────────────────────────┘│
└─────────────────────────────────────────────────────────────────┘
```

---

## Actor Infrastructure

### ActorHandle

Every actor returns a handle for lifecycle management:

```rust
// zen/src/actors/mod.rs

use tokio_util::sync::CancellationToken;

pub struct ActorHandle {
    cancel: CancellationToken,
}

impl ActorHandle {
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
```

### SessionInfo

Shared state that actors read to know which sessions exist:

```rust
// zen/src/actors/mod.rs

#[derive(Debug, Clone)]
pub struct SessionInfo {
    pub id: SessionId,
    pub tmux_name: String,
    pub repo_path: Option<PathBuf>,
    pub worktree_path: Option<PathBuf>,
    pub prompt_pattern: Option<String>,
}
```

**Why Arc<RwLock<Vec>>?**

- `Arc` - Shared ownership across actors
- `RwLock` - Multiple readers, single writer
- `Vec` - Actors iterate all sessions

The logic thread updates this when sessions change:

```rust
// In execute_command()
Command::UpdateSessionInfo => {
    let info = build_session_info(&model.sessions, model.repo_path.as_deref(), &*model.agent);
    *session_info.write().await = info;
}
```

---

## PreviewActor

Captures tmux pane content for live terminal preview.

### Purpose

- Show real-time agent output in the viewport
- Update frequently for "live" feel
- Preserve ANSI escape codes for colors

### Implementation

```rust
// zen/src/actors/preview.rs

const PREVIEW_INTERVAL: Duration = Duration::from_millis(250);
const TMUX_TIMEOUT: Duration = Duration::from_millis(100);

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

    pub fn spawn(self) -> ActorHandle {
        let cancel = CancellationToken::new();
        let cancel_clone = cancel.clone();

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(self.interval);

            loop {
                tokio::select! {
                    _ = cancel_clone.cancelled() => break,
                    _ = interval.tick() => {
                        if self.msg_tx.is_closed() {
                            break;
                        }

                        let infos = self.session_info.read().await.clone();
                        if infos.is_empty() {
                            continue;
                        }

                        // Capture all panes in parallel
                        let mut handles = Vec::with_capacity(infos.len());
                        for info in &infos {
                            let id = info.id;
                            let tmux_name = info.tmux_name.clone();
                            handles.push(tokio::spawn(async move {
                                let result = capture_pane(&tmux_name).await;
                                (id, result)
                            }));
                        }

                        // Send results as messages
                        for handle in handles {
                            if let Ok((id, Ok(content))) = handle.await {
                                let _ = self.msg_tx.send(Message::PreviewUpdated(id, content));
                            }
                        }
                    }
                }
            }
        });

        ActorHandle::new(cancel)
    }
}
```

### Key Design Decisions

| Decision | Reason |
|----------|--------|
| 250ms interval | Balance responsiveness vs CPU usage |
| Parallel capture | Don't serialize across sessions |
| 100ms timeout | Prevent slow tmux from blocking |
| Silent failures | Missing session = no message |

---

## DiffActor

Computes git diff statistics between repo and worktrees.

### Purpose

- Show +/- line counts in HUD
- Track code changes per session
- Enable diff preview mode

### Implementation

```rust
// zen/src/actors/diff.rs

const DIFF_INTERVAL: Duration = Duration::from_millis(1000);
const GIT_TIMEOUT: Duration = Duration::from_millis(500);

pub struct DiffActor {
    msg_tx: mpsc::UnboundedSender<Message>,
    session_info: Arc<RwLock<Vec<SessionInfo>>>,
    interval: Duration,
}

impl DiffActor {
    pub fn spawn(self) -> ActorHandle {
        let cancel = CancellationToken::new();
        let cancel_clone = cancel.clone();

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(self.interval);

            loop {
                tokio::select! {
                    _ = cancel_clone.cancelled() => break,
                    _ = interval.tick() => {
                        let infos = self.session_info.read().await.clone();

                        // Filter to sessions with valid paths
                        let diff_sessions: Vec<_> = infos.iter()
                            .filter_map(|s| {
                                Some((s.id, s.repo_path.clone()?, s.worktree_path.clone()?))
                            })
                            .collect();

                        if diff_sessions.is_empty() {
                            continue;
                        }

                        // Compute diffs in parallel
                        let mut handles = Vec::with_capacity(diff_sessions.len());
                        for (id, repo_path, worktree_path) in diff_sessions {
                            handles.push(tokio::spawn(async move {
                                let result = diff_stats(&repo_path, &worktree_path).await;
                                (id, result)
                            }));
                        }

                        // Send results
                        for handle in handles {
                            if let Ok((id, Ok(stats))) = handle.await {
                                let _ = self.msg_tx.send(Message::DiffUpdated(id, stats));
                            }
                        }
                    }
                }
            }
        });

        ActorHandle::new(cancel)
    }
}

async fn diff_stats(repo_path: &Path, worktree_path: &Path) -> Result<DiffStats> {
    let repo_path = repo_path.to_path_buf();
    let worktree_path = worktree_path.to_path_buf();

    blocking_with_timeout(GIT_TIMEOUT, move || {
        let git_ops = GitOps::new(&repo_path)?;
        git_ops.diff_stats(&worktree_path)
    }).await
}
```

### Key Design Decisions

| Decision | Reason |
|----------|--------|
| 1000ms interval | Git operations are slower |
| 500ms timeout | Prevent git hang from blocking |
| Filter by path | Skip paused sessions |
| spawn_blocking | Git is synchronous |

---

## PromptDetectorActor

Detects when the agent is waiting for user input.

### Purpose

- Notify user when agent needs attention
- Detect "stuck" states
- Track user attachment status

### Implementation

```rust
// zen/src/actors/prompt.rs

const PROMPT_INTERVAL: Duration = Duration::from_millis(500);
const TMUX_TIMEOUT: Duration = Duration::from_millis(100);

pub struct PromptDetectorActor {
    msg_tx: mpsc::UnboundedSender<Message>,
    session_info: Arc<RwLock<Vec<SessionInfo>>>,
    interval: Duration,
}

impl PromptDetectorActor {
    pub fn spawn(self) -> ActorHandle {
        let cancel = CancellationToken::new();
        let cancel_clone = cancel.clone();

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(self.interval);

            loop {
                tokio::select! {
                    _ = cancel_clone.cancelled() => break,
                    _ = interval.tick() => {
                        let infos = self.session_info.read().await.clone();
                        if infos.is_empty() {
                            continue;
                        }

                        // Check all sessions in parallel
                        let mut handles = Vec::with_capacity(infos.len());
                        for info in &infos {
                            let id = info.id;
                            let tmux_name = info.tmux_name.clone();
                            let prompt_pattern = info.prompt_pattern.clone();

                            handles.push(tokio::spawn(async move {
                                let has_prompt = detect_prompt(&tmux_name, prompt_pattern.as_deref()).await;
                                let user_attached = session_attached(&tmux_name).await;
                                (id, has_prompt, user_attached)
                            }));
                        }

                        // Send results
                        for handle in handles {
                            if let Ok((id, has_prompt, user_attached)) = handle.await {
                                let _ = self.msg_tx.send(Message::PromptDetected(id, has_prompt, user_attached));
                            }
                        }
                    }
                }
            }
        });

        ActorHandle::new(cancel)
    }
}

async fn detect_prompt(tmux_name: &str, prompt_pattern: Option<&str>) -> bool {
    let Some(pattern) = prompt_pattern else {
        return false;
    };

    match capture_pane_plain(tmux_name).await {
        Ok(content) => content.contains(pattern),
        Err(_) => false,
    }
}

async fn session_attached(session_name: &str) -> bool {
    blocking_with_timeout(TMUX_TIMEOUT, move || {
        Tmux::session_attached(session_name)
    })
    .await
    .ok()
    .map(|s| s.trim() == "1")
    .unwrap_or(false)
}
```

### Prompt Patterns

Each agent defines its prompt pattern:

```rust
// In Agent trait
fn prompt_pattern(&self) -> Option<&str> {
    Some("Do you want to proceed?") // Example
}
```

This allows detection of agent-specific prompts.

---

## Actor Lifecycle

### Spawning

Actors are spawned when the logic thread starts:

```rust
// zen/src/app.rs

fn spawn_actors(
    msg_tx: mpsc::UnboundedSender<Message>,
    session_info: Arc<RwLock<Vec<SessionInfo>>>,
) -> Vec<ActorHandle> {
    vec![
        PreviewActor::new(msg_tx.clone(), session_info.clone()).spawn(),
        DiffActor::new(msg_tx.clone(), session_info.clone()).spawn(),
        PromptDetectorActor::new(msg_tx.clone(), session_info.clone()).spawn(),
    ]
}
```

### Shutdown

Actors are cancelled on application exit:

```rust
fn shutdown_actors(actors: &[ActorHandle]) {
    for actor in actors {
        actor.shutdown();
    }
}
```

Each actor checks for cancellation:

```rust
tokio::select! {
    _ = cancel_clone.cancelled() => break,  // Exit loop
    _ = interval.tick() => { /* normal work */ }
}
```

### Channel Closed Detection

Actors also exit if the message channel closes:

```rust
if self.msg_tx.is_closed() {
    break;
}
```

This handles the case where the logic thread exits without explicit cancellation.

---

## Blocking Operations

Both tmux capture and git operations are synchronous. We wrap them:

```rust
// zen/src/util.rs

pub async fn blocking_with_timeout<T, F>(timeout: Duration, f: F) -> Result<T>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T> + Send + 'static,
{
    let result = tokio::time::timeout(
        timeout,
        tokio::task::spawn_blocking(f)
    ).await;

    match result {
        Ok(Ok(inner)) => inner,
        Ok(Err(e)) => Err(Error::TaskPanic(e.to_string())),
        Err(_) => Err(Error::Timeout),
    }
}
```

**Why this pattern?**

| Concern | Solution |
|---------|----------|
| Blocking code | `spawn_blocking` moves to thread pool |
| Slow operations | `timeout` prevents hangs |
| Error handling | Convert panics to Results |

---

## Message Flow

Complete flow from actor to UI:

```
┌─────────────┐
│   Actor     │
│  interval   │
│   tick      │
└──────┬──────┘
       │
       ▼
┌─────────────┐
│  Parallel   │
│  tasks      │
│ (per session)│
└──────┬──────┘
       │
       ▼
┌─────────────┐
│  Message::  │
│  XxxUpdated │
└──────┬──────┘
       │
       ▼
┌─────────────┐
│  msg_rx     │
│ (try_recv)  │
└──────┬──────┘
       │
       ▼
┌─────────────┐
│  update()   │
│  TEA        │
└──────┬──────┘
       │
       ▼ (if selected session)
┌─────────────┐
│  dirty=true │
│  snapshot() │
└──────┬──────┘
       │
       ▼
┌─────────────┐
│  Render     │
│  Thread     │
└─────────────┘
```

---

## Bounded Message Drain

The logic thread limits how many actor messages it processes per tick:

```rust
const MAX_BG_MESSAGES_PER_TICK: usize = 50;

// In logic loop
let mut bg_count = 0;
while let Ok(msg) = msg_rx.try_recv() {
    let cmds = update(&mut model, msg);
    execute_commands(cmds).await;

    bg_count += 1;
    if bg_count >= MAX_BG_MESSAGES_PER_TICK {
        break; // Don't starve keyboard input
    }
}
```

**Why bound the drain?**

- Actors might flood messages (e.g., all sessions update at once)
- Keyboard input must remain responsive
- 50 messages is enough to avoid starvation

---

## Actor Comparison

| Actor | Interval | Operation | Timeout | Output |
|-------|----------|-----------|---------|--------|
| Preview | 250ms | tmux capture | 100ms | ANSI content |
| Diff | 1000ms | git diff | 500ms | DiffStats |
| Prompt | 500ms | tmux capture + check | 100ms | bool, bool |

---

## Adding a New Actor

To add a new background task:

1. **Define the actor struct:**

```rust
pub struct MyActor {
    msg_tx: mpsc::UnboundedSender<Message>,
    session_info: Arc<RwLock<Vec<SessionInfo>>>,
}
```

2. **Implement spawn:**

```rust
impl MyActor {
    pub fn spawn(self) -> ActorHandle {
        let cancel = CancellationToken::new();
        let cancel_clone = cancel.clone();

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_millis(X));

            loop {
                tokio::select! {
                    _ = cancel_clone.cancelled() => break,
                    _ = interval.tick() => {
                        // Do work, send messages
                    }
                }
            }
        });

        ActorHandle::new(cancel)
    }
}
```

3. **Add message type:**

```rust
pub enum Message {
    // ...
    MyActorUpdate(SessionId, Data),
}
```

4. **Handle in update:**

```rust
Message::MyActorUpdate(id, data) => {
    model.my_cache.insert(id, data);
    if is_selected(id) {
        model.dirty = true;
    }
}
```

5. **Spawn in app.rs:**

```rust
fn spawn_actors(...) -> Vec<ActorHandle> {
    vec![
        PreviewActor::new(...).spawn(),
        DiffActor::new(...).spawn(),
        PromptDetectorActor::new(...).spawn(),
        MyActor::new(...).spawn(),  // Add here
    ]
}
```

---

## Summary

The actor system provides:

1. **Parallel background work** - Multiple tasks run independently
2. **Non-blocking design** - Timeouts prevent hangs
3. **Message-based communication** - Clean integration with TEA
4. **Graceful shutdown** - Cancellation tokens for clean exit
5. **Bounded processing** - Keyboard input never starves

Actors are the "workers" that keep the UI updated without blocking user interaction.
