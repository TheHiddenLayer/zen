# Decoupled Game Loop Architecture

> The secret sauce: Two threads, independent clocks, zero blocking.

**Related docs:**
- [Architecture](./architecture.md) - Module overview
- [TEA Pattern](./tea-pattern.md) - State management
- [Actor System](./actors.md) - Background tasks
- [Async Patterns](./async-patterns.md) - Tokio integration

---

## The Core Insight

Traditional TUI applications use a single event loop: read input, process, render, repeat. This creates a fundamental tension: rendering must wait for input processing, and input processing must wait for I/O operations.

**Zen solves this with a game engine architecture:**

```
                    ┌─────────────────────────────────────────┐
                    │            Decoupled Game Loop           │
                    │                                          │
   ┌────────────────┤  Logic Thread        Render Thread       │
   │   Input        │  (Tokio Runtime)     (Main Thread)       │
   │   Keyboard     │                                          │
   │   Events       │  ┌──────────┐       ┌──────────────┐    │
   │                │  │  Model   │       │   Terminal   │    │
   └───────────────►│  │  State   │──────►│   Drawing    │    │
                    │  └──────────┘       └──────────────┘    │
                    │       │                    ▲             │
                    │       │   RenderState      │             │
                    │       └────────────────────┘             │
                    │       (bounded channel, latest-wins)     │
                    └─────────────────────────────────────────┘
```

**Key properties:**

1. **Two independent threads** - Logic and Render run on their own clocks
2. **State snapshotting** - Logic creates immutable snapshots for Render
3. **Zero blocking** - Input processing never waits for rendering
4. **Latest-wins semantics** - If Render is slow, it skips to latest state

---

## Thread Architecture

### Main Thread: The Painter

The main thread owns the terminal and runs a pure render loop:

```rust
// zen/src/main.rs

fn render_loop(
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    state_rx: Receiver<RenderState>,
    shutdown: &AtomicBool,
) -> Result<()> {
    let mut current_state = RenderState::default();
    let mut last_rendered_version: u64 = 0;

    loop {
        // Check shutdown
        if shutdown.load(Ordering::SeqCst) {
            break;
        }

        // Non-blocking receive (latest-wins)
        match state_rx.try_recv() {
            Ok(state) => {
                if state.version != last_rendered_version {
                    current_state = state;
                }
            }
            Err(TryRecvError::Empty) => { /* keep current */ }
            Err(TryRecvError::Disconnected) => break,
        }

        // Frame timing (60 FPS cap)
        if elapsed < FRAME_DURATION {
            thread::sleep(Duration::from_micros(500));
            continue;
        }

        // Only render if state changed
        if state_changed {
            terminal.draw(|frame| ui::draw(frame, &current_state))?;
            last_rendered_version = current_state.version;
        }
    }
    Ok(())
}
```

**Responsibilities:**
- Terminal setup/teardown (raw mode, alternate screen)
- Frame rate capping (60 FPS)
- On-change rendering (skip redundant draws)
- Pure drawing - never mutates state

### Logic Thread: The Brain

The logic thread runs a Tokio runtime and handles all state mutations:

```rust
// zen/src/app.rs

impl LogicThread {
    async fn run_async(
        config: Config,
        state_tx: Sender<RenderState>,
        shutdown: Arc<AtomicBool>,
    ) -> Result<()> {
        let mut model = Model::load(config, agent).await?;

        loop {
            // PHASE 1: Keyboard (highest priority, synchronous)
            while event::poll(Duration::ZERO)? {
                if let Event::Key(key) = event::read()? {
                    let cmds = update(&mut model, Message::Key(key));
                    execute_commands(&mut model, cmds).await;

                    if model.dirty {
                        send_state(&state_tx, &model);
                        model.dirty = false;
                    }
                }
            }

            // PHASE 2: Background messages (bounded drain)
            let mut bg_count = 0;
            while let Ok(msg) = msg_rx.try_recv() {
                let cmds = update(&mut model, msg);
                execute_commands(&mut model, cmds).await;

                bg_count += 1;
                if bg_count >= MAX_BG_MESSAGES_PER_TICK {
                    break; // Don't starve keyboard
                }
            }

            // Send state if dirty
            if model.dirty {
                send_state(&state_tx, &model);
            }

            // PHASE 3: Yield
            tokio::time::sleep(Duration::from_micros(500)).await;
        }
    }
}
```

**Responsibilities:**
- Keyboard input polling (zero timeout - never blocks)
- TEA update function execution
- Command execution (spawn async tasks)
- State snapshot creation and sending
- Background actor coordination

---

## State Snapshotting

The communication between threads uses **immutable state snapshots**:

```rust
// zen/src/render.rs

/// Global version counter (atomic, lock-free)
static VERSION_COUNTER: AtomicU64 = AtomicU64::new(0);

pub fn next_version() -> u64 {
    VERSION_COUNTER.fetch_add(1, Ordering::Relaxed)
}

/// Immutable snapshot for rendering
#[derive(Debug, Clone)]
pub struct RenderState {
    pub version: u64,              // For change detection
    pub sessions: Vec<SessionView>, // Session list
    pub selected: usize,           // Selection index
    pub mode: Mode,                // UI mode
    pub preview: Option<String>,   // Terminal preview
    pub diff: Option<(DiffStats, String)>,
    pub input_buffer: String,
    pub error_message: Option<String>,
}
```

### Creating Snapshots

The Model creates snapshots via the `snapshot()` method:

```rust
// zen/src/tea/model.rs

impl Model {
    pub fn snapshot(&self) -> RenderState {
        let sessions: Vec<SessionView> = self.sessions
            .iter()
            .map(|s| SessionView {
                id: s.id,
                name: s.name.clone(),
                branch: s.branch.clone(),
                status: s.status,
                // ... other fields
            })
            .collect();

        RenderState {
            version: next_version(), // Monotonically increasing
            sessions,
            selected: self.selected,
            mode: self.mode,
            preview: self.preview_cache.get(&selected_id).cloned(),
            // ...
        }
    }
}
```

### Version Tracking

The render thread uses version numbers to detect changes:

```rust
// In render loop
if state.version != last_rendered_version {
    state_changed = true;
}

if state_changed {
    terminal.draw(|frame| ui::draw(frame, &current_state))?;
    last_rendered_version = current_state.version;
    state_changed = false;
}
```

This is a **zero-cost optimization** - when no state changes, no rendering occurs.

---

## The Latest-Wins Channel

The state channel uses `crossbeam_channel::bounded(1)`:

```rust
// In main()
let (state_tx, state_rx) = crossbeam_channel::bounded::<RenderState>(1);
```

### Why Bounded(1)?

| Channel Type | Behavior | Problem |
|--------------|----------|---------|
| Unbounded | Never blocks | Memory grows if render is slow |
| Bounded(N) | Blocks when full | Logic thread would wait for render |
| **Bounded(1) + try_send** | Drops old state | **Perfect!** Always latest state |

### The Pattern

```rust
// Sending (logic thread)
fn send_state(state_tx: &Sender<RenderState>, model: &Model) {
    let snapshot = model.snapshot();
    // try_send: if channel full, this fails (but we don't care)
    let _ = state_tx.try_send(snapshot);
}

// Receiving (render thread)
match state_rx.try_recv() {
    Ok(state) => current_state = state,
    Err(TryRecvError::Empty) => { /* use current */ }
    Err(TryRecvError::Disconnected) => break,
}
```

**Result:** The render thread always sees the latest state, never blocks the logic thread, and handles variable render speeds gracefully.

---

## Input Processing: Zero Latency

Input is processed **synchronously** with zero timeout:

```rust
// Poll with Duration::ZERO = non-blocking check
while event::poll(Duration::ZERO)? {
    if let Event::Key(key) = event::read()? {
        // Process immediately
        let cmds = update(&mut model, Message::Key(key));
        // ...
    }
}
```

**Why not async input?**

| Approach | Latency | Problem |
|----------|---------|---------|
| Async EventStream | ~1-5ms | Event buffering, queue delays |
| **Sync polling** | **~0.1ms** | **Direct read, instant response** |

The game loop checks for keyboard input on every iteration, processes it immediately, then yields to background work.

### Priority Hierarchy

1. **Keyboard input** - Always processed first, synchronously
2. **Background messages** - Bounded drain (max 50 per tick)
3. **Async tasks** - Run during yield phase

This ensures input never feels "laggy" even under heavy background load.

---

## Dirty Flag Optimization

State changes set a `dirty` flag to minimize channel traffic:

```rust
// zen/src/tea/update.rs

pub fn update(model: &mut Model, msg: Message) -> Vec<Command> {
    match msg {
        Message::Key(key) => {
            model.dirty = true; // Keyboard always triggers render
            // ...
        }
        Message::PreviewUpdated(id, content) => {
            model.preview_cache.insert(id, content);
            // Only dirty if this affects visible state
            if model.selected_session().map(|s| s.id) == Some(id) {
                model.dirty = true;
            }
        }
        // ...
    }
}
```

**Result:** Background updates for non-selected sessions don't trigger renders.

---

## Frame Timing

The render thread maintains consistent 60 FPS timing:

```rust
const TARGET_FPS: u32 = 60;
const FRAME_DURATION: Duration = Duration::from_micros(1_000_000 / TARGET_FPS as u64);

// In render loop
let elapsed = last_frame.elapsed();
if elapsed < FRAME_DURATION {
    thread::sleep(Duration::from_micros(500)); // Brief yield
    continue;
}
last_frame = Instant::now();
```

**Why 60 FPS?**

- Human perception threshold for smooth motion
- Matches typical monitor refresh rates
- Reasonable CPU usage for terminal apps

**Why not faster?**

- Terminal updates have inherent latency
- Diminishing returns past 60 FPS
- Wastes CPU cycles

---

## Shutdown Coordination

Clean shutdown uses an atomic flag:

```rust
// Shared between threads
let shutdown = Arc::new(AtomicBool::new(false));

// Logic thread sets on quit
if matches!(cmd, Command::Quit) {
    shutdown.store(true, Ordering::SeqCst);
    return Ok(());
}

// Render thread checks
if shutdown.load(Ordering::SeqCst) {
    break;
}

// Main waits for logic thread
let _ = logic_handle.join();
```

This ensures:
1. Logic thread saves state before exit
2. Render thread exits cleanly
3. Terminal is restored properly

---

## Comparison: Traditional vs Game Loop

| Aspect | Traditional TUI | Decoupled Game Loop |
|--------|-----------------|---------------------|
| Threading | Single thread | Two threads |
| Input latency | Depends on render | ~0.1ms (instant) |
| Render rate | Tied to input | Independent (60 FPS) |
| Blocking | Render waits for I/O | Never blocks |
| State sharing | Direct mutation | Immutable snapshots |
| CPU usage | Spiky | Consistent |

---

## Why This Matters

### 1. Responsive Input

Traditional: `wait_for_input() -> process() -> render() -> repeat`

Game Loop: Input processed instantly, independently of render speed

### 2. Smooth Rendering

Traditional: Render when input arrives (irregular timing)

Game Loop: Consistent 60 FPS with on-change optimization

### 3. Non-Blocking I/O

Traditional: Git operations block the UI

Game Loop: Background actors run independently, send messages

### 4. Scalability

Traditional: More features = slower loop

Game Loop: More actors don't affect input latency

---

## Implementation Checklist

When implementing the decoupled game loop:

- [ ] **Main thread owns terminal** - Raw mode, alternate screen
- [ ] **Logic thread runs Tokio** - Async runtime for I/O
- [ ] **Bounded(1) channel** - State snapshots, latest-wins
- [ ] **Synchronous input polling** - Zero timeout, never blocks
- [ ] **Version-tracked state** - Skip redundant renders
- [ ] **Dirty flag optimization** - Minimize channel traffic
- [ ] **Atomic shutdown flag** - Clean coordination
- [ ] **60 FPS cap** - Consistent frame timing

---

## Testing the Architecture

The architecture properties can be tested:

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

    assert!(elapsed.as_millis() < 1, "try_send blocked!");
}

#[test]
fn test_snapshot_versions_increase() {
    let v1 = next_version();
    let v2 = next_version();
    assert!(v2 > v1);
}
```

---

## Summary

The Decoupled Game Loop Architecture provides:

1. **Two-thread design** - Logic and Render are independent
2. **State snapshotting** - Immutable data crosses thread boundary
3. **Latest-wins semantics** - Never stale, never blocking
4. **Zero-latency input** - Synchronous polling, instant response
5. **Consistent rendering** - 60 FPS cap, on-change optimization

This is the "secret sauce" that makes Zen feel responsive even with multiple background operations running.
