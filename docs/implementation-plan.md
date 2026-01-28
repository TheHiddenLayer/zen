# Implementation Plan

> Parallelized execution strategy for building Zen as fast as possible.

## Overview

The implementation is divided into **4 phases** with **10 parallel work streams**. Each phase unlocks the next based on dependency resolution.

**Total estimated modules:** 10 files, ~1,650 lines of Rust

---

## Dependency Graph

```
                         ┌──────────┐
                         │ main.rs  │
                         └────┬─────┘
                              │
                         ┌────▼─────┐
                         │  app.rs  │
                         └────┬─────┘
            ┌─────────────────┼─────────────────┐
            │                 │                 │
       ┌────▼────┐      ┌─────▼─────┐     ┌─────▼────┐
       │  ui.rs  │      │session.rs │     │ event.rs │
       └─────────┘      └─────┬─────┘     └──────────┘
                    ┌─────────┼─────────┐
                    │         │         │
               ┌────▼───┐ ┌───▼────┐ ┌──▼──────┐
               │ git.rs │ │tmux.rs │ │agent.rs │
               └────────┘ └────────┘ └─────────┘
                              │
                         ┌────▼─────┐
                         │ error.rs │
                         │config.rs │
                         └──────────┘
```

---

## Phase Breakdown

### Phase 1: Foundation (5 parallel agents)

**Prerequisite:** None
**Unlocks:** Phase 2

| Agent | Files | Est. Lines | Description |
|-------|-------|------------|-------------|
| **A** | `Cargo.toml`, `error.rs`, `lib.rs` | ~80 | Project setup, error types |
| **B** | `config.rs` | ~100 | Configuration loading |
| **C** | `agent.rs` | ~200 | Agent trait and implementations |
| **D** | `git.rs` | ~250 | Git worktree and diff operations |
| **E** | `tmux.rs` | ~150 | Tmux session management |

**All 5 agents run in parallel.** No dependencies between them.

---

### Phase 2: Core Logic (2 parallel agents)

**Prerequisite:** Phase 1 complete
**Unlocks:** Phase 3

| Agent | Files | Est. Lines | Depends On |
|-------|-------|------------|------------|
| **F** | `session.rs` | ~250 | git.rs, tmux.rs, agent.rs |
| **G** | `event.rs` | ~180 | error.rs only |

**Agent G can start in Phase 1** since it only depends on error.rs.

---

### Phase 3: Application (2 parallel agents)

**Prerequisite:** Phase 2 complete
**Unlocks:** Phase 4

| Agent | Files | Est. Lines | Depends On |
|-------|-------|------------|------------|
| **H** | `app.rs` | ~350 | session.rs, event.rs, config.rs |
| **I** | `ui.rs` | ~400 | app.rs (types only) |

**Agent I can start early** with stub types from app.rs.

---

### Phase 4: Integration (1 agent)

**Prerequisite:** Phase 3 complete
**Unlocks:** Testing

| Agent | Files | Est. Lines | Description |
|-------|-------|------------|-------------|
| **J** | `main.rs` | ~80 | CLI args, terminal setup, wire components |

---

## Execution Timeline

```
Time ──────────────────────────────────────────────────────────────────►

Phase 1:  [A: error+setup] [B: config] [C: agent] [D: git] [E: tmux]
          ══════════════════════════════════════════════════════════

Phase 2:                              [F: session] [G: event]
                                      ════════════════════════

Phase 3:                                           [H: app] [I: ui]
                                                   ═════════════════

Phase 4:                                                     [J: main]
                                                             ═════════
```

**Optimized timeline** (G and I start early):

```
Time ──────────────────────────────────────────────────────────────────►

Phase 1:  [A: error] [B: config] [C: agent] [D: git] [E: tmux] [G: event]
          ════════════════════════════════════════════════════════════════

Phase 2:                         [F: session]        [I: ui stubs]
                                 ═══════════════     ══════════════

Phase 3:                                    [H: app] [I: ui complete]
                                            ══════════════════════════

Phase 4:                                                      [J: main]
                                                              ═════════
```

---

## Detailed Task Specifications

### Agent A: Foundation

**Files:** `Cargo.toml`, `src/lib.rs`, `src/error.rs`

**Cargo.toml:**
```toml
[package]
name = "zen"
version = "0.1.0"
edition = "2021"

[dependencies]
# TUI
ratatui = "0.28"
crossterm = { version = "0.28", features = ["event-stream"] }

# Async
tokio = { version = "1", features = ["rt", "sync", "time", "process", "fs"] }
tokio-util = "0.7"
futures = "0.3"

# Git
git2 = "0.19"

# Serialization
serde = { version = "1", features = ["derive"] }
serde_json = "1"

# Error handling
thiserror = "1"

# Utilities
chrono = { version = "0.4", features = ["serde"] }
dirs = "5"
uuid = { version = "1", features = ["v4"] }
which = "6"
```

**src/lib.rs:**
```rust
pub mod error;
pub mod config;
pub mod agent;
pub mod git;
pub mod tmux;
pub mod session;
pub mod event;
pub mod app;
pub mod ui;

pub use error::{Error, Result};
```

**src/error.rs:**
```rust
use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Git error: {0}")]
    Git(#[from] git2::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Tmux error: {0}")]
    Tmux(String),

    #[error("Session not found: {0}")]
    SessionNotFound(String),

    #[error("Session already exists: {0}")]
    SessionExists(String),

    #[error("No home directory")]
    NoHomeDir,

    #[error("Validation error: {0}")]
    Validation(String),

    #[error("Agent not available: {0}")]
    AgentNotAvailable(String),
}

pub type Result<T> = std::result::Result<T, Error>;
```

---

### Agent B: Configuration

**File:** `src/config.rs`

**Key types:**
```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum AgentKind {
    #[default]
    Claude,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Config {
    #[serde(default)]
    pub agent: AgentKind,
    #[serde(default)]
    pub auto_yes: bool,
}
```

**Functions:**
- `Config::load() -> Result<Self>`
- `Config::zen_dir() -> Result<PathBuf>` - returns `~/.zen`
- `Config::worktrees_dir() -> Result<PathBuf>` - returns `~/.zen/worktrees`

---

### Agent C: Agents

**File:** `src/agent.rs`

**Core trait:**
```rust
pub trait Agent: Send + Sync {
    fn name(&self) -> &'static str;
    fn binary_name(&self) -> &'static str;
    fn command(&self, cwd: &Path, prompt: Option<&str>) -> Vec<String>;
    fn command_auto_yes(&self, cwd: &Path, prompt: Option<&str>) -> Vec<String> {
        self.command(cwd, prompt)
    }
    fn is_available(&self) -> bool {
        which::which(self.binary_name()).is_ok()
    }
}
```

**Implementation:**
- `ClaudeAgent` - Claude Code CLI

**Registry:**
```rust
pub struct AgentRegistry {
    agents: HashMap<AgentKind, Arc<dyn Agent>>,
}

impl AgentRegistry {
    pub fn new() -> Self;
    pub fn get(&self, kind: AgentKind) -> Option<Arc<dyn Agent>>;
    pub fn default_agent(&self) -> Arc<dyn Agent>;
}
```

---

### Agent D: Git Operations

**File:** `src/git.rs`

**Key struct:**
```rust
pub struct GitOps {
    repo_path: PathBuf,
}

impl GitOps {
    pub fn new(repo_path: &Path) -> Result<Self>;
    pub fn create_worktree(&self, branch: &str, worktree_path: &Path) -> Result<()>;
    pub fn remove_worktree(&self, worktree_path: &Path) -> Result<()>;
    pub fn diff_stats(&self, worktree_path: &Path) -> Result<DiffStats>;
    pub fn diff_content(&self, worktree_path: &Path) -> Result<String>;
    pub fn commit_all(&self, worktree_path: &Path, message: &str) -> Result<()>;
    pub fn push(&self, branch: &str) -> Result<()>;
    pub fn current_head(&self) -> Result<String>;
}

#[derive(Debug, Clone, Copy, Default)]
pub struct DiffStats {
    pub additions: u32,
    pub deletions: u32,
}
```

**Notes:**
- Use `git2` crate exclusively (no shell commands)
- Handle detached HEAD gracefully

---

### Agent E: Tmux Management

**File:** `src/tmux.rs`

**Key struct:**
```rust
pub struct Tmux;

impl Tmux {
    pub fn create_session(name: &str, cwd: &Path, cmd: &[String]) -> Result<()>;
    pub fn kill_session(name: &str) -> Result<()>;
    pub fn capture_pane(name: &str) -> Result<String>;
    pub fn session_exists(name: &str) -> bool;
    pub fn attach(name: &str) -> Result<()>;
    pub fn send_keys(name: &str, keys: &str) -> Result<()>;
}
```

**Notes:**
- Shell out to `tmux` command
- Session naming: `zen_{session_name}_{short_id}`

---

### Agent F: Session Management

**File:** `src/session.rs`

**Key types:**
```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SessionId(pub Uuid);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum SessionStatus {
    #[default]
    Running,
    Paused,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub id: SessionId,
    pub name: String,
    pub branch: String,
    pub status: SessionStatus,
    pub worktree_path: Option<PathBuf>,
    pub base_commit: String,
    pub created_at: DateTime<Utc>,
    pub last_active: DateTime<Utc>,
    // HUD display fields
    pub agent: String,          // Agent name for AGENT column (e.g., "claude", "gpt4")
}

// The "idle" duration is computed from App.last_activity HashMap.

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct State {
    pub version: u32,
    pub sessions: Vec<Session>,
}
```

**Session operations:**
```rust
impl Session {
    pub async fn create(name: &str, agent: &dyn Agent, prompt: Option<&str>, auto_yes: bool) -> Result<Self>;
    pub async fn pause(&mut self) -> Result<()>;
    pub async fn resume(&mut self, agent: &dyn Agent, auto_yes: bool) -> Result<()>;
    pub async fn push(&self) -> Result<String>;
    pub async fn delete(self) -> Result<()>;
    pub fn tmux_name(&self) -> String;
}
```

**State operations:**
```rust
impl State {
    pub fn load() -> Result<Self>;
    pub fn save(&self) -> Result<()>;
    pub fn reconcile(&mut self) -> Vec<String>;
}
```

---

### Agent G: Event Handling

**File:** `src/event.rs`

**Key types:**
```rust
#[derive(Debug)]
pub enum Event {
    Key(KeyEvent),
    Resize(u16, u16),
    Tick,
    Action(Action),
}

#[derive(Debug)]
pub enum Action {
    // Navigation (selection IS scrolling in Pilot's Seat design)
    SelectNext,
    SelectPrev,
    CreateSession(String, Option<String>),
    SessionCreated(Session),
    SessionCreateFailed(String, String),
    DeleteSession(SessionId),
    SessionDeleted(SessionId),
    PauseSession(SessionId),
    ResumeSession(SessionId),
    PushSession(SessionId),
    SessionPushed(SessionId, String),
    Attach,
    Detach,
    DiffUpdated(SessionId, DiffStats),
    PreviewUpdated(SessionId, String),
    TogglePreviewDiff,
    ToggleHelp,
    EnterInput(InputKind),
    ExitInput,
    SubmitInput(String),
    Quit,
    Render,
}

#[derive(Debug, Clone, Copy)]
pub enum InputKind {
    SessionName,
    Prompt,
    Confirm,
}
```

**EventHandler:**
```rust
pub struct EventHandler {
    rx: mpsc::UnboundedReceiver<Event>,
    tx: mpsc::UnboundedSender<Event>,
}

impl EventHandler {
    pub fn new(tick_rate: Duration) -> Self;
    pub fn sender(&self) -> mpsc::UnboundedSender<Event>;
    pub async fn next(&mut self) -> Option<Event>;
}
```

---

### Agent H: Application State

**File:** `src/app.rs`

**Key types:**
```rust
pub struct App {
    pub sessions: Vec<Session>,
    pub selected: usize,
    pub mode: Mode,
    pub preview_mode: PreviewMode,
    pub preview_cache: HashMap<SessionId, String>,
    pub diff_cache: HashMap<SessionId, (DiffStats, String)>,
    pub config: Config,
    pub agent: Arc<dyn Agent>,
    pub event_tx: mpsc::UnboundedSender<Event>,
    pub needs_render: bool,
    // Idleness tracking - see "Idleness Metric" section below
    pub last_activity: HashMap<SessionId, Instant>,
    // Animation state for Swiss-Style typography effects
    pub animation_state: AnimationState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Mode {
    #[default]
    List,
    Input(InputKind),
    Help,
    Attached,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PreviewMode {
    #[default]
    Terminal,
    Diff,
}
```

**Core functions:**
```rust
impl App {
    pub async fn new(config: Config) -> Result<Self>;
    pub async fn run(&mut self, tui: &mut Tui) -> Result<()>;
    async fn handle_event(&mut self, event: Event) -> Result<bool>;
    async fn handle_key(&mut self, key: KeyEvent) -> Result<bool>;
    async fn handle_action(&mut self, action: Action) -> Result<bool>;
    pub fn save_state(&self) -> Result<()>;
    // Idleness tracking
    pub fn get_idle_duration(&self, session_id: SessionId) -> Duration;
    pub fn record_activity(&mut self, session_id: SessionId);
}
```

---

### Idleness Metric

**What it is**: How long since the agent last typed a key, updated a file, or emitted a log.

**Why it matters**:
- `Idle: 2s` - It's actively working
- `Idle: 15m` - Might be stuck/looping/waiting for you

**Implementation approach**:

The idleness metric is tracked at the App level, not in Session state. This is because:
1. It's runtime-only data (doesn't need to be persisted)
2. It's updated frequently (every time we capture tmux output)
3. It's used for display/UX, not session logic

**Note**: This is distinct from `Session.last_active` which is the persisted timestamp of when the session was last interacted with (paused/resumed/created). The idleness metric is a live, real-time measurement updated every render frame.

```rust
// In App struct
pub last_activity: HashMap<SessionId, Instant>,

// Update activity when capturing preview
impl App {
    async fn refresh_preview(&mut self, session_id: SessionId) {
        let old_preview = self.preview_cache.get(&session_id);
        let new_preview = capture_tmux_pane(session_id).await;

        // If content changed, update activity timestamp
        if old_preview != Some(&new_preview) {
            self.last_activity.insert(session_id, Instant::now());
        }

        self.preview_cache.insert(session_id, new_preview);
    }

    fn get_idle_duration(&self, session_id: SessionId) -> Duration {
        self.last_activity
            .get(&session_id)
            .map(|instant| instant.elapsed())
            .unwrap_or(Duration::ZERO)
    }
}
```

**Alternative: Query tmux activity time**

tmux provides `#{session_activity}` format variable which returns Unix timestamp of last activity:

```bash
tmux display-message -p -t zen_session_id '#{session_activity}'
```

This could be used instead of tracking changes in captured content.

---

### Swiss-Style Animation System

**Typography states** convey status through text treatment, not icons:

| State | Effect | When |
|-------|--------|------|
| ACTIVE | Shimmer (wave of brightness) | Recent activity (< 10s) |
| DONE | Bold + Pulse (breathing) | Task complete, needs attention |
| IDLE | Normal text | Running but waiting |
| PAUSED | Muted (50% opacity) | Session paused |

**Implementation**:

```rust
// Animation state stored in App
pub struct AnimationState {
    pub shimmers: HashMap<SessionId, Shimmer>,
    pub pulses: HashMap<SessionId, Pulse>,
    pub enabled: bool,  // For reduced motion accessibility
}

// Determine typography state from session + activity
pub enum TypographyState {
    Active,   // Shimmer effect
    Done,     // Bold + pulse
    Idle,     // Normal
    Paused,   // Muted
}

impl TypographyState {
    pub fn from_session(session: &Session, idle_duration: Duration) -> Self {
        match session.status {
            SessionStatus::Paused => Self::Paused,
            SessionStatus::Running => {
                if idle_duration.as_secs() < 10 {
                    Self::Active
                } else {
                    Self::Idle
                }
            }
        }
    }
}
```

See [Animation System](./animation-system.md) for full implementation details.

---

### Agent I: User Interface

**File:** `src/ui.rs`

**Target: <500 lines**

**HUD Column Layout** (4 columns, status via typography):

| Column | Width | Content |
|--------|-------|---------|
| PROJECT | ~12ch | Repository name (always dimmed) |
| SESSION | Flex | Session title (becomes branch, color = status) |
| AGENT | ~8ch | Agent type (always dimmed) |
| IDLE | ~6ch | Time since activity (color = urgency) |

**Structure:**
```rust
mod colors {
    pub const DEFAULT: Color = Color::Reset;
    pub const ADDITION: Color = Color::Green;
    pub const DELETION: Color = Color::Red;
    pub const DIM: Color = Color::DarkGray;
    // Status colors
    pub const STATUS_ACTIVE: Color = Color::Cyan;
    pub const STATUS_ERROR: Color = Color::Red;
    // Idle column thresholds
    pub const IDLE_HEALTHY: Color = Color::DarkGray;
    pub const IDLE_WARNING: Color = Color::Yellow;
    pub const IDLE_CRITICAL: Color = Color::Red;
}

impl App {
    pub fn ui(&self, f: &mut Frame);
    fn render_hud(&self, f: &mut Frame, area: Rect);
    fn render_session_row(&self, session: &Session, is_selected: bool) -> Line;
    fn render_viewport(&self, f: &mut Frame, area: Rect);
    fn render_diff(&self, f: &mut Frame, area: Rect);
    fn render_input_overlay(&self, f: &mut Frame);
    fn render_help_overlay(&self, f: &mut Frame);
}

// Typography effects
fn render_with_shimmer(text: &str, shimmer: &Shimmer) -> Vec<Span>;
fn render_with_pulse(text: &str, pulse: &Pulse) -> Span;
fn render_muted(text: &str) -> Span;

// Formatting helpers
fn format_idle_duration(duration: Duration) -> String;
fn idle_duration_color(duration: Duration) -> Color;
fn truncate(s: &str, max_len: usize) -> String;
fn centered_rect(percent_x: u16, height: u16, area: Rect) -> Rect;
fn strip_ansi(s: &str) -> String;
```

---

### Agent J: Main Entry Point

**File:** `src/main.rs`

**Structure:**
```rust
#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    let auto_yes = std::env::args().any(|a| a == "-y" || a == "--auto-yes");

    let mut config = Config::load()?;
    if auto_yes {
        config.auto_yes = true;
    }

    let mut tui = Tui::new()?;
    tui.enter()?;

    let mut app = App::new(config).await?;
    let result = app.run(&mut tui).await;

    tui.exit()?;

    result
}

struct Tui {
    terminal: Terminal<CrosstermBackend<Stdout>>,
}

impl Tui {
    fn new() -> Result<Self>;
    fn enter(&mut self) -> Result<()>;
    fn exit(&mut self) -> Result<()>;
    fn draw<F>(&mut self, f: F) -> Result<()>
    where
        F: FnOnce(&mut Frame);
}
```

---

## Interface Contracts

All agents must use these shared type definitions:

```rust
// From error.rs
pub type Result<T> = std::result::Result<T, Error>;

// From session.rs
pub struct SessionId(Uuid);
pub enum SessionStatus { Running, Paused }
pub struct Session { ... }
pub struct State { ... }

// From git.rs
pub struct DiffStats { additions: u32, deletions: u32 }

// From agent.rs
pub trait Agent: Send + Sync { ... }

// From event.rs
pub enum Event { ... }
pub enum Action { ... }
pub struct EventHandler { ... }

// From config.rs
pub struct Config { ... }
pub enum AgentKind { ... }
```

---

## Parallel Execution Commands

**Phase 1 (spawn all 5 in parallel):**
```
Agent A: "Implement zen foundation: Cargo.toml, src/lib.rs, src/error.rs per implementation-plan.md"
Agent B: "Implement zen/src/config.rs per implementation-plan.md"
Agent C: "Implement zen/src/agent.rs per implementation-plan.md"
Agent D: "Implement zen/src/git.rs per implementation-plan.md"
Agent E: "Implement zen/src/tmux.rs per implementation-plan.md"
```

**Phase 2 (after Phase 1):**
```
Agent F: "Implement zen/src/session.rs per implementation-plan.md"
Agent G: "Implement zen/src/event.rs per implementation-plan.md"
```

**Phase 3 (after Phase 2):**
```
Agent H: "Implement zen/src/app.rs per implementation-plan.md"
Agent I: "Implement zen/src/ui.rs per implementation-plan.md"
```

**Phase 4 (after Phase 3):**
```
Agent J: "Implement zen/src/main.rs per implementation-plan.md"
```

---

## Success Criteria

Before marking implementation complete:

1. **Startup < 50ms** - Measure with `time zen`
2. **Key response < 16ms** - No perceptible lag
3. **10 sessions, no lag** - Test with concurrent sessions
4. **State survives restart** - Kill and relaunch
5. **Clean shutdown** - Terminal restored properly
6. **`cargo clippy` clean** - No warnings
7. **`cargo test` passes** - All unit tests green
