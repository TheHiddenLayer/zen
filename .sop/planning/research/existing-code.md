# Zen Existing Codebase Research

**Date:** 2026-01-30
**Purpose:** Document existing code structure, patterns, and architecture to inform improvements

---

## 1. Project Overview

Zen is a **TUI-based AI coding session manager** written in Rust. It orchestrates parallel AI coding agents (primarily Claude Code) using git worktrees for isolation and tmux for persistent sessions.

### Key Characteristics
- **Language:** 100% Safe Rust (~20 core files)
- **Architecture:** Decoupled game loop with two-thread design
- **State Management:** TEA (The Elm Architecture) pattern
- **Persistence:** Git-native (refs, notes, JSON state files)
- **UI:** Ratatui-based TUI with 60 FPS rendering

---

## 2. Architecture Summary

### Two-Thread Design (Decoupled Game Loop)

The application uses a sophisticated two-thread architecture:

1. **Main Thread (Render Loop)**
   - Runs at 60 FPS (16.666ms frame time)
   - Pure rendering from immutable snapshots
   - No blocking operations
   - Uses Ratatui + Crossterm

2. **Logic Thread (Tokio Runtime)**
   - Handles all state mutations
   - Processes keyboard input (zero timeout polling)
   - Executes TEA update function
   - Manages background actors
   - Sends state snapshots to render thread via bounded channel

**Communication:** Bounded channel (capacity=1) with "latest-wins" semantics for lock-free state transfer.

### File: `/Users/ubhattachary/.zen/worktrees/improvements-to-zen_1769758773/src/main.rs`
```rust
// Entry point - Sets up render loop
const FRAME_DURATION: Duration = Duration::from_micros(16_666); // 60fps

fn main() -> Result<()> {
    // Parse CLI args (-t/--trust, -d/--debug, reset command)
    let config = Config::load()?;
    let shutdown = Arc::new(AtomicBool::new(false));
    let (state_tx, state_rx) = crossbeam_channel::bounded::<RenderState>(1);

    // Spawn logic thread with Tokio runtime
    let logic_handle = thread::spawn(move || LogicThread::run(...));

    // Initialize terminal and run render loop
    let mut terminal = setup_terminal()?;
    let result = render_loop(&mut terminal, state_rx, &shutdown);

    // Cleanup
    shutdown.store(true, Ordering::SeqCst);
    restore_terminal(&mut terminal)?;
    result
}
```

---

## 3. Module Organization

### Core Library Structure (`src/lib.rs`)

```rust
pub mod agent;         // Agent abstraction
pub mod config;        // Configuration loading
pub mod error;         // Error types
pub mod git;           // Git operations (worktrees, commits)
pub mod git_notes;     // Git notes for JSON state storage
pub mod git_refs;      // Git refs management (refs/zen/*)
pub mod log;           // Debug logging
pub mod session;       // Session CRUD operations
pub mod tmux;          // Terminal multiplexer integration
pub mod util;          // Blocking helpers with timeout

// Decoupled game loop architecture
pub mod actors;        // Background tasks
pub mod app;           // Logic thread
pub mod render;        // State snapshots
pub mod tea;           // TEA pattern implementation
pub mod ui;            // Rendering code
```

### Key Module Responsibilities

#### `src/session.rs` (1530 lines)
- **SessionId:** UUID-based unique identifier with short form
- **SessionStatus:** `Running` | `Locked`
- **Session:** Core data model with fields:
  - `id`, `name`, `branch`, `status`
  - `worktree_path`, `base_commit`, `base_branch`
  - `created_at`, `last_active`
  - `agent`, `project`
- **State:** Container for sessions with persistence
- **Operations:**
  - `Session::create()` - Creates worktree, branch, tmux session
  - `Session::lock()` - Auto-commits, removes worktree (preserves tmux)
  - `Session::unlock()` - Recreates worktree from branch
  - `Session::delete()` - Cleans up all resources
  - `State::reconcile()` - Recovers from inconsistent state
  - `State::cleanup_orphaned_*()` - Removes orphaned resources

```rust
// Example: Session creation workflow
pub async fn create(
    name: &str,
    repo_path: &Path,
    agent: &Agent,
    prompt: Option<&str>,
) -> Result<Self> {
    // 1. Validate name and create worktree path
    // 2. Get git user, create branch as {user}/{task-name}
    // 3. Create git worktree
    // 4. Create tmux session with agent command
    // 5. Return Session struct
}
```

#### `src/git.rs` (440 lines)
Git operations wrapper using `git2` crate:

```rust
pub struct GitOps {
    repo_path: PathBuf,
}

impl GitOps {
    pub fn create_worktree(&self, branch: &str, worktree_path: &Path) -> Result<()>
    pub fn remove_worktree(&self, worktree_path: &Path) -> Result<()>
    pub fn commit_all(&self, worktree_path: &Path, message: &str) -> Result<()>
    pub fn current_head(&self) -> Result<String>
    pub fn branch_exists(&self, branch: &str) -> Result<bool>
    pub fn is_dirty(&self, worktree_path: &Path) -> Result<bool>
    pub fn delete_branch(&self, branch: &str) -> Result<()>
    pub fn git_user(&self) -> Result<String>
    pub fn prune_worktrees(&self) -> Result<()>
}
```

**Critical Implementation Detail:** `remove_worktree()` is complex and defensive:
1. Finds worktree by path or folder name
2. Unlocks and prunes via git2 API
3. Removes directory if it exists
4. **CRITICAL:** Cleans up `.git/worktrees/<name>` admin directory
5. Force-prunes all stale worktree references

This is necessary to prevent "branch is already checked out" errors on unlock.

#### `src/git_refs.rs` (351 lines) - **NEW MODULE**
Low-level primitives for git refs under `refs/zen/` namespace:

```rust
pub struct GitRefs {
    repo_path: PathBuf,
}

impl GitRefs {
    pub fn ref_exists(&self, name: &str) -> Result<bool>
    pub fn create_ref(&self, name: &str, target: &str) -> Result<()>
    pub fn read_ref(&self, name: &str) -> Result<Option<String>>
    pub fn update_ref(&self, name: &str, target: &str) -> Result<()>
    pub fn delete_ref(&self, name: &str) -> Result<()>
    pub fn list_refs(&self, prefix: Option<&str>) -> Result<Vec<String>>
}
```

All refs are stored under `refs/zen/{name}` namespace. The module handles:
- Creating refs that point to commit SHAs
- Reading/updating/deleting refs
- Listing refs with optional prefix filtering
- Idempotent deletion (no error if ref doesn't exist)
- Error handling for duplicate refs

**Comprehensive test coverage** (176 lines of tests).

#### `src/git_notes.rs` (372 lines) - **NEW MODULE**
JSON-serialized data attachment to commits via git notes:

```rust
pub struct GitNotes {
    repo_path: PathBuf,
}

impl GitNotes {
    pub fn note_exists(&self, commit: &str, namespace: &str) -> Result<bool>
    pub fn set_note<T: Serialize>(&self, commit: &str, namespace: &str, data: &T) -> Result<()>
    pub fn get_note<T: DeserializeOwned>(&self, commit: &str, namespace: &str) -> Result<Option<T>>
    pub fn delete_note(&self, commit: &str, namespace: &str) -> Result<()>
    pub fn list_notes(&self, namespace: &str) -> Result<Vec<String>>
}
```

Notes are stored under `refs/notes/zen/{namespace}`. Features:
- Type-safe JSON serialization/deserialization
- Namespace isolation for different data types
- Automatic overwrite of existing notes
- Idempotent deletion
- Lists all commits with notes in a namespace

**Comprehensive test coverage** (217 lines of tests) including multi-namespace isolation tests.

#### `src/tmux.rs`
Tmux integration for persistent agent sessions:

```rust
impl Tmux {
    pub fn create_session(name: &str, cwd: &Path, cmd: &[&str]) -> Result<()>
    pub fn kill_session(name: &str) -> Result<()>
    pub fn capture_pane(name: &str) -> Result<String>  // With ANSI
    pub fn capture_pane_plain(name: &str) -> Result<String>  // Without ANSI
    pub fn attach(name: &str) -> Result<()>
    pub fn session_exists(name: &str) -> bool
    pub fn inside_tmux() -> bool
    pub fn list_zen_sessions() -> Result<Vec<String>>
    pub fn session_name(sanitized_name: &str, short_id: &str) -> String
}
```

Naming convention: `zen_{sanitized-name}_{short-id}`

#### `src/agent.rs` (116 lines)
Simple agent abstraction:

```rust
pub struct Agent {
    base_command: Vec<String>,
}

impl Agent {
    pub fn from_config(config: &Config) -> Self
    pub fn name(&self) -> &'static str
    pub fn binary(&self) -> &str
    pub fn command(&self, prompt: Option<&str>) -> Vec<String>
    pub fn is_available(&self) -> bool
    pub fn prompt_pattern(&self) -> Option<&'static str>
}
```

Currently detects Claude based on command string. Returns:
- `name()` - "Claude" or "Unknown"
- `prompt_pattern()` - "Do you want" for Claude (for prompt detection)
- `command(prompt)` - Full command with optional prompt appended

**Simple and extensible** - ready for multi-agent support.

#### `src/config.rs` (139 lines)
Configuration management:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Config {
    pub trust: bool,
    pub worktree_dir: Option<String>,
    pub command: Option<String>,
}

impl Config {
    pub fn zen_dir() -> Result<PathBuf>            // ~/.zen
    pub fn config_path() -> Result<PathBuf>         // ~/.zen/zen.toml
    pub fn state_path() -> Result<PathBuf>          // ~/.zen/state.json
    pub fn worktrees_dir() -> Result<PathBuf>       // Configurable or ~/.zen/worktrees
    pub fn effective_command(&self) -> &str         // Defaults to "claude"
    pub fn load() -> Result<Self>
    pub fn save(&self) -> Result<()>
    pub fn ensure_dirs() -> Result<()>
}
```

Configuration file: `~/.zen/zen.toml`
```toml
trust = false
worktree_dir = "~/worktrees"
command = "claude --dangerously-skip-permissions"
```

#### `src/error.rs` (66 lines)
Unified error handling with `thiserror`:

```rust
#[derive(Error, Debug)]
pub enum Error {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Git error: {0}")]
    Git(#[from] git2::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("TOML parse error: {0}")]
    TomlParse(#[from] toml::de::Error),
    #[error("Tmux error: {0}")]
    Tmux(String),
    #[error("Session not found: {0}")]
    SessionNotFound(String),
    #[error("Validation error: {0}")]
    Validation(String),
    #[error("Operation timed out after {0:?}")]
    Timeout(std::time::Duration),
    #[error("Ref already exists: {0}")]
    RefExists(String),
    #[error("Ref not found: {0}")]
    RefNotFound(String),
    // ... more variants
}
```

---

## 4. TEA (The Elm Architecture) Pattern

Zen implements a pure functional state management pattern inspired by Elm.

### Structure (`src/tea/`)

```
tea/
├── mod.rs         # Exports
├── model.rs       # Pure application state
├── message.rs     # Input events
├── command.rs     # Side effects
└── update.rs      # Pure update function
```

### `src/tea/model.rs` (310 lines)

```rust
pub struct Model {
    // Core state
    pub sessions: Vec<Session>,
    pub selected: usize,
    pub mode: Mode,

    // Caches (updated by background actors)
    pub preview_cache: HashMap<SessionId, String>,
    pub prompt_cache: HashMap<SessionId, PromptState>,
    pub activity_cache: HashMap<SessionId, (u64, std::time::Instant)>,

    // Input state
    pub input_buffer: String,
    pub notification: Option<Notification>,
    pub pending_delete: Option<SessionId>,
    pub pending_session_name: Option<String>,
    pub pending_prompt: Option<String>,

    // UI toggles
    pub show_keymap: bool,

    // Dirty flag
    pub dirty: bool,

    // Config (immutable after init)
    pub config: Config,
    pub repo_path: Option<PathBuf>,
    pub agent: Arc<Agent>,
}

pub enum Mode {
    List,
    Input(InputKind),
}

pub enum InputKind {
    SessionName,
    Prompt,
    Confirm,
}

pub struct Notification {
    pub level: NotificationLevel,
    pub message: String,
}
```

Key method:
```rust
pub fn snapshot(&self) -> RenderState {
    // Creates immutable snapshot for render thread
    // Includes activity detection with 1500ms grace period
}
```

### Message Types

```rust
pub enum Message {
    // Keyboard input
    Key(KeyEvent),

    // Background actor updates
    PreviewUpdated { session_id: SessionId, content: String },
    PromptDetected { session_id: SessionId, has_prompt: bool },
    ActivityUpdated { session_id: SessionId, timestamp: u64 },

    // Session lifecycle
    SessionCreated(Session),
    SessionCreateFailed(String, String),
    SessionDeleted(SessionId),
    SessionDeleteFailed(SessionId, String),
    SessionLocked(SessionId),
    SessionUnlocked(SessionId),

    // State operations
    StateSaved,
    StateSaveFailed(String),
}
```

### Update Function Pattern

```rust
pub fn update(model: &mut Model, message: Message) -> Vec<Command> {
    match message {
        Message::Key(key) => {
            // Process keyboard input
            // Return commands for side effects
        }
        Message::SessionCreated(session) => {
            model.sessions.push(session);
            model.dirty = true;
            vec![Command::SaveState]
        }
        // ... more message handlers
    }
}
```

**Pure function:** `(Model, Message) -> Commands`
- No I/O in update function
- Side effects returned as commands
- State mutations happen synchronously
- Commands executed asynchronously

---

## 5. Background Actor System

### `src/actors/` Structure

```
actors/
├── mod.rs         # ActorHandle, SessionInfo
├── preview.rs     # Tmux pane capture (250ms interval)
├── prompt.rs      # Prompt detection (500ms interval)
└── (future) diff.rs, activity.rs
```

### Actor Pattern

```rust
pub struct ActorHandle {
    cancel_token: CancellationToken,
    join_handle: JoinHandle<()>,
}

pub struct SessionInfo {
    pub id: SessionId,
    pub tmux_name: String,
    pub repo_path: Option<PathBuf>,
    pub worktree_path: Option<PathBuf>,
    pub prompt_pattern: Option<String>,
}
```

Each actor:
1. Receives `Arc<RwLock<Vec<SessionInfo>>>` - shared session list
2. Runs on interval (250-1000ms)
3. Sends `Message` updates via unbounded channel
4. Respects `CancellationToken` for clean shutdown

Example: `PreviewActor` (250ms interval)
```rust
impl PreviewActor {
    pub fn spawn(...) -> ActorHandle {
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = cancel_token.cancelled() => break,
                    _ = tokio::time::sleep(Duration::from_millis(250)) => {
                        // Capture tmux pane for each session
                        // Send Message::PreviewUpdated
                    }
                }
            }
        });
    }
}
```

---

## 6. Render System

### `src/render.rs`
Immutable state snapshots for render thread:

```rust
#[derive(Debug, Clone, Default)]
pub struct RenderState {
    pub version: u64,              // Monotonically increasing
    pub sessions: Vec<SessionView>,
    pub selected: usize,
    pub mode: Mode,
    pub preview: Option<String>,
    pub input_buffer: String,
    pub notification: Option<Notification>,
    pub show_keymap: bool,
    pub trust_enabled: bool,
}

pub struct SessionView {
    pub id: SessionId,
    pub name: String,
    pub project: String,
    pub branch: String,
    pub base_branch: String,
    pub base_commit: String,
    pub agent: String,
    pub status: SessionStatus,
    pub last_active: DateTime<Utc>,
    pub is_active: Option<bool>,  // None = loading, Some(bool) = known state
}
```

Version tracking:
```rust
static VERSION_COUNTER: AtomicU64 = AtomicU64::new(1);

pub fn next_version() -> u64 {
    VERSION_COUNTER.fetch_add(1, Ordering::Relaxed)
}
```

### `src/ui.rs`
Ratatui-based UI rendering (expects to be < 500 lines):
- Session list with colors for status
- Preview pane with ANSI color support
- Input prompts
- Keymap help (toggled by `?`)
- Notification messages

---

## 7. State Persistence

### Current Approach: JSON State File

**File:** `~/.zen/state.json`

```json
{
  "version": 1,
  "sessions": [
    {
      "id": "550e8400-e29b-41d4-a716-446655440000",
      "name": "feature-auth",
      "branch": "alice/feature-auth",
      "status": "running",
      "worktree_path": "/Users/alice/.zen/worktrees/feature-auth_1705312200",
      "base_commit": "abc123def456",
      "base_branch": "main",
      "created_at": "2024-01-15T10:30:00Z",
      "last_active": "2024-01-15T14:22:00Z",
      "agent": "Claude",
      "project": "my-app"
    }
  ]
}
```

**Atomic writes:**
1. Write to `state.json.tmp`
2. Backup existing to `state.json.bak`
3. Rename `state.json.tmp` → `state.json`

### Git-Native Modules (Ready for Migration)

The **git_refs** and **git_notes** modules are production-ready for git-native state storage:

#### `GitRefs` - For References
Use cases:
- Store workflow execution refs: `refs/zen/workflows/{workflow-id}`
- Store task tracking refs: `refs/zen/tasks/{task-id}`
- Store session refs: `refs/zen/sessions/{session-id}`

#### `GitNotes` - For Metadata
Use cases:
- Store session metadata on commits: `refs/notes/zen/sessions`
- Store workflow state on commits: `refs/notes/zen/workflows`
- Store task status on commits: `refs/notes/zen/tasks`

**Example Migration Path:**
```rust
// Current: JSON file
State::load() -> Result<State>
State::save(&self) -> Result<()>

// Future: Git-native
GitStateManager::load_sessions() -> Result<Vec<Session>>
GitStateManager::save_session(&self, session: &Session) -> Result<()>
```

---

## 8. CLI Interface

### Command Structure

```bash
# Main TUI
zen
zen --trust          # Auto-approve agent prompts
zen --debug          # Enable debug logging to ~/.zen/zen.log

# Reset command
zen reset            # Delete sessions (skip dirty worktrees)
zen reset --force    # Delete all sessions including dirty
```

### Environment Variables
- `ZEN_DEBUG=1` - Enable debug logging (alternative to `--debug`)

---

## 9. Dependencies (`Cargo.toml`)

### TUI & Terminal
- **ratatui** 0.29 - TUI framework (stable, avoiding 0.30+ breaking changes)
- **crossterm** 0.29 - Cross-platform terminal manipulation
- **ansi-to-tui** 7 - ANSI color parsing for tmux capture

### Async Runtime
- **tokio** 1.49 - Multi-threaded runtime
  - Features: rt, rt-multi-thread, sync, time, process, fs, macros
- **tokio-util** 0.7 - CancellationToken for actors
- **futures** 0.3 - Future combinators

### Git
- **git2** 0.20 - libgit2 bindings

### Serialization
- **serde** 1 - Serialization framework
- **serde_json** 1 - JSON support
- **toml** 0.8 - TOML config files

### Error Handling
- **thiserror** 2 - Derive Error trait

### Concurrency
- **crossbeam-channel** 0.5 - Bounded channels for render state

### Utilities
- **chrono** 0.4 - Date/time handling
- **dirs** 6 - Home directory detection
- **uuid** 1 - Session IDs (v4)
- **which** 8 - Binary path resolution

---

## 10. Testing Strategy

### Current Coverage

#### Unit Tests
- **session.rs**: 473 lines of tests (30% of file)
  - SessionId creation, serialization
  - SessionStatus validation
  - Session validation, TMux naming
  - State operations (add, find, remove)
  - Sanitization edge cases
- **git_refs.rs**: 176 lines of tests (50% of file)
  - CRUD operations on refs
  - Error handling (duplicate, not found)
  - List operations with prefixes
- **git_notes.rs**: 217 lines of tests (58% of file)
  - JSON serialization round-trips
  - Namespace isolation
  - Overwrite behavior
  - List operations
- **config.rs**: 38 lines of tests
- **agent.rs**: 48 lines of tests
- **error.rs**: 12 lines of tests

#### Architecture Tests (`lib.rs`)
```rust
#[test]
fn test_frame_duration_is_60fps()
fn test_render_state_default_is_cheap()
fn test_version_generation_is_fast()
fn test_version_monotonicity()
fn test_bounded_channel_latest_wins()
fn test_try_send_never_blocks_on_full_channel()
fn test_render_state_clone_performance()
fn test_rapid_state_updates_dont_block()
```

These tests verify:
- 60 FPS frame timing
- Lock-free channel semantics
- Version monotonicity
- Performance characteristics

### Missing Coverage
- Integration tests for full session lifecycle
- Tmux operations (currently manual testing)
- Git worktree edge cases
- Actor system behavior
- TEA update function edge cases

---

## 11. Logging & Debugging

### Log Macros (`src/log.rs`)

```rust
zlog!("message");              // Always logs
zlog_debug!("message");        // Only when ZEN_DEBUG=1
zlog_warn!("message");         // Warning level
zlog_error!("message");        // Error level
```

Logs written to: `~/.zen/zen.log`

Initialization:
```rust
zen::log::init_with_debug(debug: bool);
```

---

## 12. Key Design Patterns

### 1. Decoupled Game Loop
- Two independent threads (render + logic)
- Lock-free communication via bounded channel
- Latest-wins semantics for state snapshots
- No blocking in render path

### 2. TEA (The Elm Architecture)
- Pure functional state management
- Messages as input, Commands as output
- All side effects explicit as commands
- Testable update function

### 3. Actor-Based Background Tasks
- Independent polling actors
- Shared session info via Arc<RwLock<...>>
- Cancellation tokens for clean shutdown
- Bounded message drain to prevent starvation

### 4. Git-Native Primitives
- Low-level refs and notes modules
- Idempotent operations
- Comprehensive error handling
- Ready for git-native state migration

### 5. Defensive Resource Management
- Extensive cleanup in Session::delete()
- Orphan detection and recovery
- Multiple fallback strategies (e.g., worktree removal)
- Reconciliation on startup

---

## 13. Current Capabilities

### What Works Today

✅ **Session Management**
- Create sessions with auto-generated branches (user/task-name)
- Lock/unlock sessions (preserves tmux, removes/recreates worktree)
- Delete sessions with comprehensive cleanup
- List running and locked sessions

✅ **Git Integration**
- Worktree creation/removal
- Branch management
- Auto-commit on lock
- Orphan cleanup

✅ **Tmux Integration**
- Session creation with agent command
- Attach/detach (with and without tmux context)
- Pane capture for preview
- Session listing

✅ **TUI Interface**
- 60 FPS rendering
- Session list with status colors
- Preview pane with ANSI colors
- Input prompts (name, prompt, confirm)
- Keymap help (toggle with `?`)
- Notification system

✅ **Background Actors**
- Preview capture (250ms)
- Prompt detection (500ms)
- Activity tracking

✅ **State Persistence**
- JSON state file with atomic writes
- Backup on save
- State reconciliation on startup
- Orphan cleanup

✅ **Configuration**
- TOML config file
- Custom worktree directory
- Custom agent command
- Trust mode (auto-approve prompts)

### What's Missing (vs. Vision)

❌ **Multi-Agent Orchestration**
- Only single-session support today
- No parallel agent execution
- No dependency inference
- No task decomposition

❌ **Skills Integration**
- No /pdd, /code-task-generator, /code-assist integration
- Agents don't have access to Skills

❌ **Reactive Planning**
- No auto-replan on design changes
- No conflict resolution agents
- No task reassignment

❌ **Quality of Life**
- No stuck agent detection/restart
- No merged worktree cleanup
- No cost tracking

❌ **Git-Native State**
- Still using JSON state file
- GitRefs and GitNotes modules ready but not integrated

---

## 14. Code Quality Observations

### Strengths

✅ **Clean Architecture**
- Clear separation of concerns
- Well-documented modules
- Consistent patterns throughout

✅ **Type Safety**
- No `unsafe` blocks
- Strong typing with newtype wrappers (SessionId)
- Comprehensive error handling

✅ **Performance Focus**
- Lock-free rendering
- Zero-allocation in hot paths
- Bounded message draining
- Architecture tests verify performance

✅ **Testing**
- Comprehensive unit tests for core modules
- Architecture verification tests
- Good test coverage for git_refs and git_notes

✅ **Documentation**
- Extensive inline documentation
- Separate design documents
- Architecture diagrams

### Areas for Improvement

⚠️ **Test Coverage**
- Missing integration tests
- No tmux operation tests
- Limited TEA update function tests

⚠️ **Actor System**
- No retry logic
- No backpressure handling
- Actor errors logged but not surfaced

⚠️ **State Migration**
- GitRefs and GitNotes ready but unused
- Need migration path from JSON to git-native

⚠️ **Configuration**
- Limited validation
- No schema versioning
- Missing agent adapter configuration

---

## 15. Next Steps Recommendations

Based on this research, here are suggested priorities:

### Phase 1: Foundation Improvements
1. **Git-Native State Migration**
   - Implement `GitStateManager` using GitRefs + GitNotes
   - Migrate from JSON to git-native storage
   - Add migration tool for existing users

2. **Enhanced Agent System**
   - Make Agent trait-based for extensibility
   - Add agent adapter configuration
   - Support multiple agent types

3. **Improved Actor System**
   - Add retry logic
   - Implement backpressure
   - Surface actor errors to UI

### Phase 2: Multi-Agent Orchestration
1. **Task Management**
   - Implement Task model
   - Add task dependency tracking
   - Build task scheduler

2. **Parallel Execution**
   - Multi-session support
   - Parallel agent spawning
   - Resource management (token budgets)

3. **Skills Integration**
   - Integrate PDD skill
   - Integrate Code Task Generator
   - Make skills available to agents

### Phase 3: Advanced Features
1. **Reactive Planning**
   - Implement plan watcher
   - Add conflict resolution agent
   - Build task reassignment logic

2. **Quality of Life**
   - Stuck agent detection
   - Auto-cleanup merged worktrees
   - Cost tracking dashboard

---

## 16. File Paths Reference

All paths referenced in this document:

### Core Source Files
- `/Users/ubhattachary/.zen/worktrees/improvements-to-zen_1769758773/src/main.rs`
- `/Users/ubhattachary/.zen/worktrees/improvements-to-zen_1769758773/src/lib.rs`
- `/Users/ubhattachary/.zen/worktrees/improvements-to-zen_1769758773/src/session.rs`
- `/Users/ubhattachary/.zen/worktrees/improvements-to-zen_1769758773/src/git.rs`
- `/Users/ubhattachary/.zen/worktrees/improvements-to-zen_1769758773/src/git_refs.rs`
- `/Users/ubhattachary/.zen/worktrees/improvements-to-zen_1769758773/src/git_notes.rs`
- `/Users/ubhattachary/.zen/worktrees/improvements-to-zen_1769758773/src/tmux.rs`
- `/Users/ubhattachary/.zen/worktrees/improvements-to-zen_1769758773/src/agent.rs`
- `/Users/ubhattachary/.zen/worktrees/improvements-to-zen_1769758773/src/config.rs`
- `/Users/ubhattachary/.zen/worktrees/improvements-to-zen_1769758773/src/error.rs`
- `/Users/ubhattachary/.zen/worktrees/improvements-to-zen_1769758773/src/app.rs`

### TEA Pattern Files
- `/Users/ubhattachary/.zen/worktrees/improvements-to-zen_1769758773/src/tea/mod.rs`
- `/Users/ubhattachary/.zen/worktrees/improvements-to-zen_1769758773/src/tea/model.rs`
- `/Users/ubhattachary/.zen/worktrees/improvements-to-zen_1769758773/src/tea/message.rs`
- `/Users/ubhattachary/.zen/worktrees/improvements-to-zen_1769758773/src/tea/command.rs`
- `/Users/ubhattachary/.zen/worktrees/improvements-to-zen_1769758773/src/tea/update.rs`

### Actor System Files
- `/Users/ubhattachary/.zen/worktrees/improvements-to-zen_1769758773/src/actors/mod.rs`
- `/Users/ubhattachary/.zen/worktrees/improvements-to-zen_1769758773/src/actors/preview.rs`
- `/Users/ubhattachary/.zen/worktrees/improvements-to-zen_1769758773/src/actors/prompt.rs`

### Documentation Files
- `/Users/ubhattachary/.zen/worktrees/improvements-to-zen_1769758773/readme`
- `/Users/ubhattachary/.zen/worktrees/improvements-to-zen_1769758773/docs/architecture.md`
- `/Users/ubhattachary/.zen/worktrees/improvements-to-zen_1769758773/docs/data-model.md`
- `/Users/ubhattachary/.zen/worktrees/improvements-to-zen_1769758773/docs/tea-pattern.md`
- `/Users/ubhattachary/.zen/worktrees/improvements-to-zen_1769758773/docs/game-loop-architecture.md`
- `/Users/ubhattachary/.zen/worktrees/improvements-to-zen_1769758773/docs/actors.md`

### Planning Files
- `/Users/ubhattachary/.zen/worktrees/improvements-to-zen_1769758773/.sop/planning/rough-idea.md`
- `/Users/ubhattachary/.zen/worktrees/improvements-to-zen_1769758773/.sop/planning/idea-honing.md`

### Build Files
- `/Users/ubhattachary/.zen/worktrees/improvements-to-zen_1769758773/Cargo.toml`
- `/Users/ubhattachary/.zen/worktrees/improvements-to-zen_1769758773/Cargo.lock`

---

## Summary

Zen has a **solid foundation** with clean architecture, type-safe design, and performance-focused implementation. The codebase demonstrates:

1. **Mature architectural patterns** - Decoupled game loop, TEA, actor system
2. **Production-ready git primitives** - GitRefs and GitNotes modules ready for migration
3. **Defensive resource management** - Comprehensive cleanup and recovery
4. **Strong testing culture** - Architecture tests, unit tests with good coverage
5. **Clear extensibility points** - Agent abstraction, actor system, command pattern

The main gap is **multi-agent orchestration** - the current implementation manages single sessions well, but lacks:
- Task decomposition and dependency tracking
- Parallel agent execution
- Reactive planning and conflict resolution
- Skills integration

The next phase should focus on building the orchestration layer on top of this solid foundation, potentially leveraging the existing git primitives for distributed state management.
