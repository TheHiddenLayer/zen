# Async Patterns: Tokio + Ratatui Integration

> How to build a responsive TUI with non-blocking operations.

**Related docs:**
- [Game Loop Architecture](./game-loop-architecture.md) - Two-thread design
- [TEA Pattern](./tea-pattern.md) - State management
- [Actor System](./actors.md) - Background tasks

---

## The Challenge

TUI applications must:
1. **Respond instantly to input** (< 16ms for 60fps feel)
2. **Render smoothly** without blocking
3. **Perform I/O operations** (git, tmux, file system) without freezing

Traditional async approaches create latency. Zen's solution: **separate threads for logic and rendering**.

---

## The Decoupled Architecture

```
┌──────────────────────────────────────────────────────────────────────┐
│                    Two-Thread Architecture                            │
│                                                                       │
│  ┌─────────────────────────┐        ┌──────────────────────────────┐ │
│  │      Main Thread        │        │       Logic Thread           │ │
│  │    (Render Loop)        │        │     (Tokio Runtime)          │ │
│  │                         │        │                              │ │
│  │  ┌─────────────────┐    │        │  ┌────────────────────────┐  │ │
│  │  │    Terminal     │    │        │  │   Synchronous Input    │  │ │
│  │  │   (Ratatui)     │    │        │  │   event::poll(ZERO)    │  │ │
│  │  └─────────────────┘    │        │  └────────────────────────┘  │ │
│  │          ▲              │        │            │                 │ │
│  │          │ draw()       │        │            ▼                 │ │
│  │  ┌─────────────────┐    │        │  ┌────────────────────────┐  │ │
│  │  │  RenderState    │◄───┼────────┼──│   TEA Update           │  │ │
│  │  │  (snapshot)     │    │        │  │   Model + Msg = Cmds   │  │ │
│  │  └─────────────────┘    │        │  └────────────────────────┘  │ │
│  │                         │        │            │                 │ │
│  │  60 FPS cap             │        │            ▼                 │ │
│  │  Version-tracked        │        │  ┌────────────────────────┐  │ │
│  │                         │        │  │   Background Actors    │  │ │
│  └─────────────────────────┘        │  │   (Tokio tasks)        │  │ │
│                                     │  └────────────────────────┘  │ │
│                                     └──────────────────────────────┘ │
└──────────────────────────────────────────────────────────────────────┘
```

**Key insight:** Keyboard input is polled synchronously in the logic thread, not via async streams.

---

## Synchronous Input Polling

The logic thread polls keyboard with zero timeout:

```rust
// zen/src/app.rs

// Poll with Duration::ZERO = non-blocking check
while event::poll(Duration::ZERO)? {
    if let Event::Key(key) = event::read()? {
        // Process immediately via TEA update
        let cmds = update(&mut model, Message::Key(key));

        for cmd in cmds {
            execute_command(&mut model, cmd, &msg_tx).await;
        }

        // Send state snapshot after keyboard input
        if model.dirty {
            send_state(&state_tx, &model);
            model.dirty = false;
        }
    }
}
```

### Why Synchronous?

| Approach | Latency | Problem |
|----------|---------|---------|
| Async EventStream | ~1-5ms | Event buffering, queue delays |
| **Sync poll(ZERO)** | **~0.1ms** | **None - instant response** |

The `poll(Duration::ZERO)` call returns immediately if no input is available.

---

## The Logic Thread Loop

Three-phase loop prioritizing input:

```rust
impl LogicThread {
    async fn run_async(
        config: Config,
        state_tx: Sender<RenderState>,
        shutdown: Arc<AtomicBool>,
    ) -> Result<()> {
        let mut model = Model::load(config, agent).await?;
        let (msg_tx, mut msg_rx) = mpsc::unbounded_channel::<Message>();
        let actors = spawn_actors(msg_tx.clone(), session_info.clone());

        loop {
            // Check shutdown
            if shutdown.load(Ordering::SeqCst) {
                break;
            }

            // ═══════════════════════════════════════════════════════
            // PHASE 1: KEYBOARD INPUT (Highest priority, synchronous)
            // ═══════════════════════════════════════════════════════
            while event::poll(Duration::ZERO)? {
                if let Event::Key(key) = event::read()? {
                    let cmds = update(&mut model, Message::Key(key));
                    for cmd in cmds {
                        if execute_command(&mut model, cmd, &msg_tx).await {
                            return Ok(()); // Quit
                        }
                    }

                    if model.dirty {
                        send_state(&state_tx, &model);
                        model.dirty = false;
                    }
                }
            }

            // ═══════════════════════════════════════════════════════
            // PHASE 2: BACKGROUND MESSAGES (Bounded drain)
            // ═══════════════════════════════════════════════════════
            let mut bg_count = 0;
            while let Ok(msg) = msg_rx.try_recv() {
                let cmds = update(&mut model, msg);
                for cmd in cmds {
                    if execute_command(&mut model, cmd, &msg_tx).await {
                        return Ok(());
                    }
                }

                bg_count += 1;
                if bg_count >= MAX_BG_MESSAGES_PER_TICK {
                    break; // Don't starve keyboard
                }
            }

            if model.dirty {
                send_state(&state_tx, &model);
                model.dirty = false;
            }

            // ═══════════════════════════════════════════════════════
            // PHASE 3: YIELD (Let async tasks make progress)
            // ═══════════════════════════════════════════════════════
            tokio::time::sleep(Duration::from_micros(500)).await;
        }

        shutdown_actors(&actors);
        save_state_sync(&model);
        Ok(())
    }
}
```

### Bounded Message Drain

Background actors might flood the message channel. We bound the drain:

```rust
const MAX_BG_MESSAGES_PER_TICK: usize = 50;

let mut bg_count = 0;
while let Ok(msg) = msg_rx.try_recv() {
    // Process message...

    bg_count += 1;
    if bg_count >= MAX_BG_MESSAGES_PER_TICK {
        break; // Return to keyboard polling
    }
}
```

This ensures input never waits for background work to complete.

---

## State Channel: Bounded(1) Latest-Wins

Communication between logic and render uses a special channel pattern:

```rust
// In main()
let (state_tx, state_rx) = crossbeam_channel::bounded::<RenderState>(1);
```

### The Pattern

```rust
// Sending (logic thread) - NEVER BLOCKS
fn send_state(state_tx: &Sender<RenderState>, model: &Model) {
    let snapshot = model.snapshot();
    // try_send: if channel full, this fails silently
    // Old state is NOT overwritten - we just skip this send
    let _ = state_tx.try_send(snapshot);
}

// Receiving (render thread) - NON-BLOCKING
match state_rx.try_recv() {
    Ok(state) => current_state = state,
    Err(TryRecvError::Empty) => { /* keep using current */ }
    Err(TryRecvError::Disconnected) => break,
}
```

### Why This Works

| Scenario | Behavior |
|----------|----------|
| Render faster than logic | Render waits, uses current state |
| Logic faster than render | Old states dropped, only latest received |
| Burst of updates | Render sees only final state |

**Result:** Render thread always has recent state, logic thread never blocks.

---

## Command Execution: Async Tasks

Commands spawn Tokio tasks that report results via messages:

```rust
async fn execute_command(
    model: &mut Model,
    cmd: Command,
    msg_tx: &mpsc::UnboundedSender<Message>,
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

**Pattern:** Command execution spawns task -> task sends completion Message -> update() processes Message.

---

## Blocking Operations with Timeout

Git and tmux operations are synchronous. We wrap them safely:

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

### Usage in Actors

```rust
// In DiffActor
async fn diff_stats(repo_path: &Path, worktree_path: &Path) -> Result<DiffStats> {
    let repo_path = repo_path.to_path_buf();
    let worktree_path = worktree_path.to_path_buf();

    blocking_with_timeout(Duration::from_millis(500), move || {
        let git_ops = GitOps::new(&repo_path)?;
        git_ops.diff_stats(&worktree_path)
    }).await
}

// In PreviewActor
async fn capture_pane(session_name: &str) -> Result<String> {
    let name = session_name.to_string();
    blocking_with_timeout(Duration::from_millis(100), move || {
        Tmux::capture_pane(&name)
    }).await
}
```

### Timeout Values

| Operation | Timeout | Reason |
|-----------|---------|--------|
| Tmux capture | 100ms | Should be instant |
| Git diff | 500ms | Can be slow on large repos |
| Session create | None | User-initiated, show progress |

---

## Actor Communication

Actors use unbounded mpsc channels to send messages:

```rust
// Actor sends
let _ = self.msg_tx.send(Message::PreviewUpdated(id, content));

// Logic thread receives (non-blocking)
while let Ok(msg) = msg_rx.try_recv() {
    let cmds = update(&mut model, msg);
    // ...
}
```

### Why Unbounded for Actor->Logic?

- Actors are fixed (3 actors, known message rate)
- Bounded would require actors to block or drop messages
- Logic thread drains quickly (bounded drain)

---

## Graceful Shutdown

Shutdown uses multiple signals:

### 1. Atomic Flag (Thread-Safe)

```rust
// Shared between threads
let shutdown = Arc::new(AtomicBool::new(false));

// Logic thread checks
if shutdown.load(Ordering::SeqCst) {
    break;
}

// Main thread sets on cleanup
shutdown.store(true, Ordering::SeqCst);
```

### 2. Cancellation Tokens (Actors)

```rust
// Each actor has a token
let cancel = CancellationToken::new();

// Actor checks in select
tokio::select! {
    _ = cancel.cancelled() => break,
    _ = interval.tick() => { /* work */ }
}

// Shutdown signals all actors
fn shutdown_actors(actors: &[ActorHandle]) {
    for actor in actors {
        actor.shutdown(); // calls cancel.cancel()
    }
}
```

### 3. Channel Disconnect Detection

```rust
// Actor checks if channel closed
if self.msg_tx.is_closed() {
    break;
}

// Render thread checks
match state_rx.try_recv() {
    Err(TryRecvError::Disconnected) => break,
    // ...
}
```

---

## Render Thread: Pure Loop

The render thread is intentionally simple:

```rust
fn render_loop(
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    state_rx: Receiver<RenderState>,
    shutdown: &AtomicBool,
) -> Result<()> {
    let mut current_state = RenderState::default();
    let mut last_rendered_version: u64 = 0;
    let mut last_frame = Instant::now();
    let mut state_changed = true;

    loop {
        // Check shutdown
        if shutdown.load(Ordering::SeqCst) {
            break;
        }

        // Non-blocking receive
        match state_rx.try_recv() {
            Ok(state) => {
                if state.version != last_rendered_version {
                    state_changed = true;
                }
                current_state = state;
            }
            Err(TryRecvError::Empty) => {}
            Err(TryRecvError::Disconnected) => break,
        }

        // Frame timing
        let elapsed = last_frame.elapsed();
        if elapsed < FRAME_DURATION {
            thread::sleep(Duration::from_micros(500));
            continue;
        }
        last_frame = Instant::now();

        // On-change rendering
        if state_changed {
            terminal.draw(|frame| ui::draw(frame, &current_state))?;
            last_rendered_version = current_state.version;
            state_changed = false;
        }
    }

    Ok(())
}
```

**Properties:**
- Never mutates state
- Never blocks on channel
- Consistent 60 FPS timing
- Skips redundant renders

---

## Summary: The Async Model

| Component | Approach | Why |
|-----------|----------|-----|
| Keyboard Input | Synchronous poll | Lowest latency |
| State Transfer | Bounded(1) channel | Latest-wins, non-blocking |
| Background Work | Actor tasks + messages | Independent, cancellable |
| Blocking I/O | spawn_blocking + timeout | Thread pool, won't hang |
| Rendering | Main thread, 60 FPS | Owns terminal, consistent |

**The key insight:** Use async where it helps (I/O, background work), but keep input synchronous for instant response.

---

## Testing Async Code

### Channel Behavior

```rust
#[test]
fn test_state_channel_never_blocks() {
    let (tx, _rx) = crossbeam_channel::bounded::<RenderState>(1);

    // Fill channel
    let _ = tx.try_send(RenderState::default());

    // Second send should NOT block
    let start = Instant::now();
    let _ = tx.try_send(RenderState::default());
    let elapsed = start.elapsed();

    assert!(elapsed.as_millis() < 1);
}
```

### Timeout Behavior

```rust
#[tokio::test]
async fn test_blocking_timeout() {
    let result = blocking_with_timeout(Duration::from_millis(10), || {
        std::thread::sleep(Duration::from_secs(1));
        Ok(())
    }).await;

    assert!(matches!(result, Err(Error::Timeout)));
}
```

---

## Common Pitfalls

### 1. Don't Use Async EventStream for Input

```rust
// BAD: Adds latency
let mut reader = EventStream::new();
while let Some(event) = reader.next().await { ... }

// GOOD: Instant response
while event::poll(Duration::ZERO)? {
    let event = event::read()?;
    // ...
}
```

### 2. Don't Block in Update Function

```rust
// BAD: Blocks logic thread
fn update(model: &mut Model, msg: Message) -> Vec<Command> {
    if let Message::Key(_) = msg {
        let diff = git.diff_stats(); // BLOCKS!
    }
}

// GOOD: Return command, execute async
fn update(model: &mut Model, msg: Message) -> Vec<Command> {
    if let Message::Key(_) = msg {
        vec![Command::RefreshDiff { id: session.id }]
    }
}
```

### 3. Don't Forget Timeouts

```rust
// BAD: Can hang forever
tokio::task::spawn_blocking(|| git.diff_stats()).await

// GOOD: Timeout protected
blocking_with_timeout(Duration::from_millis(500), || git.diff_stats()).await
```
