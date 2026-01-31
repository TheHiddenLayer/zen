# Rust Ecosystem Research for Zen

**Date**: 2026-01-30
**Purpose**: Evaluate Rust libraries and tools for building Zen - a parallel AI agent orchestrator

---

## 1. DAG Scheduling Libraries

### 1.1 petgraph

**Crate**: [`petgraph`](https://crates.io/crates/petgraph)
**Version**: 0.6.x (stable)

**Key Features**:
- General-purpose graph data structure library
- Supports both directed and undirected graphs
- Multiple graph representations: `Graph`, `StableGraph`, `GraphMap`
- Built-in algorithms: topological sort, cycle detection, shortest paths, DFS/BFS
- Zero-cost abstractions with excellent performance
- Well-maintained and widely used (~10M downloads)

**Relevant for Zen**:
```rust
use petgraph::graph::{DiGraph, NodeIndex};
use petgraph::algo::toposort;
use petgraph::visit::Topo;

// Create a DAG for task dependencies
let mut dag = DiGraph::<&str, ()>::new();

// Add agent tasks as nodes
let task_a = dag.add_node("agent_task_a");
let task_b = dag.add_node("agent_task_b");
let task_c = dag.add_node("agent_task_c");

// Define dependencies (edges)
dag.add_edge(task_a, task_c, ()); // C depends on A
dag.add_edge(task_b, task_c, ()); // C depends on B

// Topological sort for execution order
match toposort(&dag, None) {
    Ok(order) => {
        for node in order {
            println!("Execute: {}", dag[node]);
        }
    }
    Err(_) => println!("Cycle detected in task graph!"),
}

// Iterator-based traversal for parallel execution
let mut topo = Topo::new(&dag);
while let Some(node) = topo.next(&dag) {
    // Can execute all nodes at same level in parallel
    println!("Ready to execute: {}", dag[node]);
}
```

**Pros**:
- Mature, stable, and well-documented
- Efficient memory usage and performance
- Rich algorithm library out of the box
- Active community and maintenance
- Serialization support via serde

**Cons**:
- Learning curve for graph terminology
- No built-in parallel execution scheduler (need to build on top)
- API can be verbose for simple use cases

**Recommendation**: ✅ **Highly Recommended** - petgraph is the de facto standard for graph operations in Rust. Use it as the foundation for task dependency management.

---

### 1.2 daggy

**Crate**: [`daggy`](https://crates.io/crates/daggy)
**Version**: 0.8.x

**Key Features**:
- Built specifically for DAGs (not general graphs)
- Guarantees acyclic property at compile time where possible
- Based on petgraph internally
- Walker API for traversing dependencies
- Simpler API focused on DAG use cases

**Relevant for Zen**:
```rust
use daggy::{Dag, Walker};

// Create a DAG with task metadata
let mut dag = Dag::<&str, &str>::new();

// Add nodes
let a = dag.add_node("task_a");
let b = dag.add_node("task_b");
let c = dag.add_node("task_c");

// Add edges with dependency labels
dag.add_edge(a, c, "output_file").unwrap(); // Returns Err if cycle
dag.add_edge(b, c, "config_data").unwrap();

// Walk children (dependents)
let mut children = dag.children(a).iter(&dag);
while let Some((edge, node)) = children.next(&dag) {
    println!("{} depends on task_a via {}", dag[node], dag[edge]);
}

// Walk parents (dependencies)
let parents = dag.parents(c);
println!("task_c has {} dependencies", parents.iter(&dag).count());
```

**Pros**:
- API designed specifically for DAG workflows
- Cycle prevention at the API level
- Cleaner, more intuitive for DAG use cases
- Good documentation with examples

**Cons**:
- Less flexible than petgraph (DAG-only)
- Smaller community and fewer downloads (~1M)
- Less frequent updates (but stable)
- Built on petgraph anyway, so adds a layer

**Recommendation**: ⚠️ **Consider** - Good if you want a simpler DAG-focused API, but petgraph is more versatile and maintained. Start with petgraph unless the DAG-specific guarantees are critical.

---

### 1.3 Comparison and Recommendation

| Feature | petgraph | daggy |
|---------|----------|-------|
| Maturity | ⭐⭐⭐⭐⭐ | ⭐⭐⭐⭐ |
| Performance | Excellent | Good (wraps petgraph) |
| API Simplicity | Moderate | High (for DAGs) |
| Flexibility | High | Low (DAG-only) |
| Community | Large | Small |
| Maintenance | Active | Stable |

**Final Recommendation**: Use **petgraph** as the primary library. It provides all needed functionality for DAG scheduling while maintaining flexibility for future features.

---

## 2. TUI Frameworks

### 2.1 ratatui

**Crate**: [`ratatui`](https://crates.io/crates/ratatui)
**Version**: 0.28.x (actively developed)

**Key Features**:
- Fork of the original `tui-rs` (now maintained and actively developed)
- Immediate mode rendering
- Widget-based architecture
- Layout system with constraints
- Built-in widgets: List, Table, Chart, Gauge, Paragraph, Block, etc.
- Backend agnostic (works with crossterm, termion, termwiz)
- Event-driven architecture support
- Excellent for htop/lazygit-style interfaces

**Relevant for Zen**:
```rust
use ratatui::{
    backend::CrosstermBackend,
    widgets::{Block, Borders, List, ListItem, Gauge},
    layout::{Layout, Constraint, Direction},
    Terminal,
};
use crossterm::{
    event::{self, Event, KeyCode},
    terminal::{enable_raw_mode, EnterAlternateScreen},
    ExecutableCommand,
};

fn run_agent_dashboard() -> Result<(), Box<dyn std::error::Error>> {
    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = std::io::stdout();
    stdout.execute(EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    loop {
        terminal.draw(|f| {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(3),  // Header
                    Constraint::Min(10),    // Agent list
                    Constraint::Length(3),  // Status bar
                ])
                .split(f.area());

            // Agent list widget
            let agents = vec![
                ListItem::new("Agent 1: Running (PID: 1234)"),
                ListItem::new("Agent 2: Waiting (Dependencies: A, B)"),
                ListItem::new("Agent 3: Completed ✓"),
            ];
            let list = List::new(agents)
                .block(Block::default().borders(Borders::ALL).title("Active Agents"));
            f.render_widget(list, chunks[1]);

            // Progress gauge
            let gauge = Gauge::default()
                .block(Block::default().borders(Borders::ALL).title("Overall Progress"))
                .gauge_style(Style::default().fg(Color::Green))
                .percent(67);
            f.render_widget(gauge, chunks[2]);
        })?;

        // Event handling
        if event::poll(std::time::Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                if key.code == KeyCode::Char('q') {
                    break;
                }
            }
        }
    }

    Ok(())
}
```

**Pros**:
- Modern, actively maintained (took over from abandoned tui-rs)
- Extensive widget library
- Flexible layout system
- Great documentation and examples
- Large community with good support
- Used by many popular TUI apps (gitui, bottom, etc.)
- Excellent performance

**Cons**:
- Immediate mode can be verbose for complex UIs
- Requires managing state carefully
- Learning curve for layout system

**Recommendation**: ✅ **Highly Recommended** - The best choice for building htop/lazygit-style interfaces in Rust.

---

### 2.2 crossterm

**Crate**: [`crossterm`](https://crates.io/crates/crossterm)
**Version**: 0.28.x

**Key Features**:
- Cross-platform terminal manipulation
- Works on Windows, Linux, macOS
- Low-level control: cursor, colors, input, terminal properties
- Event system for keyboard/mouse input
- Async support (via tokio)
- Most commonly used as backend for ratatui

**Relevant for Zen**:
```rust
use crossterm::{
    cursor, execute, queue,
    style::{Color, Print, SetForegroundColor, SetBackgroundColor},
    terminal::{Clear, ClearType},
    event::{read, Event, KeyCode},
};
use std::io::{stdout, Write};

fn display_agent_status() -> crossterm::Result<()> {
    let mut stdout = stdout();

    // Clear screen and position cursor
    execute!(
        stdout,
        Clear(ClearType::All),
        cursor::MoveTo(0, 0)
    )?;

    // Colored output
    queue!(
        stdout,
        SetForegroundColor(Color::Green),
        Print("✓ Agent 1: Completed\n"),
        SetForegroundColor(Color::Yellow),
        Print("⟳ Agent 2: Running...\n"),
        SetForegroundColor(Color::Red),
        Print("✗ Agent 3: Failed\n"),
    )?;

    stdout.flush()?;

    // Wait for keypress
    loop {
        if let Event::Key(key_event) = read()? {
            if key_event.code == KeyCode::Char('q') {
                break;
            }
        }
    }

    Ok(())
}
```

**Pros**:
- True cross-platform support
- Low-level control when needed
- Well-maintained and popular
- Works great with ratatui
- Async-friendly

**Cons**:
- Low-level - need to build UI abstractions yourself
- More verbose than high-level frameworks
- Not suitable for complex UIs without additional layers

**Recommendation**: ✅ **Use with ratatui** - Essential as the backend, but use ratatui for the UI layer.

---

### 2.3 TUI Recommendation for Zen

**Recommended Stack**:
```
┌─────────────────────────────┐
│  Zen Application Logic      │
├─────────────────────────────┤
│  ratatui (UI Widgets)       │
├─────────────────────────────┤
│  crossterm (Terminal I/O)   │
└─────────────────────────────┘
```

**Example Integration**:
```rust
use ratatui::{backend::CrosstermBackend, Terminal};
use crossterm::{
    terminal::{enable_raw_mode, disable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    ExecutableCommand,
};

pub struct ZenTUI {
    terminal: Terminal<CrosstermBackend<std::io::Stdout>>,
}

impl ZenTUI {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        enable_raw_mode()?;
        let mut stdout = std::io::stdout();
        stdout.execute(EnterAlternateScreen)?;
        let backend = CrosstermBackend::new(stdout);
        let terminal = Terminal::new(backend)?;
        Ok(Self { terminal })
    }

    pub fn run(&mut self, agents: &[Agent]) -> Result<(), Box<dyn std::error::Error>> {
        // Main render loop
        self.terminal.draw(|f| {
            render_agent_dashboard(f, agents);
        })?;
        Ok(())
    }
}

impl Drop for ZenTUI {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = std::io::stdout().execute(LeaveAlternateScreen);
    }
}
```

---

## 3. tmux Integration

### 3.1 tmux Programmatic Control

**Library**: [`tmux_interface`](https://crates.io/crates/tmux_interface)
**Version**: 0.3.x

**Key Features**:
- Rust bindings for tmux CLI commands
- Type-safe API for tmux operations
- Session, window, and pane management
- Command sending to panes
- Status monitoring

**Relevant for Zen**:
```rust
use tmux_interface::{Tmux, TmuxCommand, NewSession, SplitWindow, SendKeys};

fn setup_zen_session() -> Result<(), Box<dyn std::error::Error>> {
    // Create new tmux session
    TmuxCommand::new()
        .new_session()
        .session_name("zen-orchestrator")
        .detached()
        .output()?;

    // Split window into panes for multiple agents
    TmuxCommand::new()
        .split_window()
        .target("zen-orchestrator")
        .horizontal()
        .output()?;

    TmuxCommand::new()
        .split_window()
        .target("zen-orchestrator")
        .vertical()
        .output()?;

    // Send commands to specific panes
    TmuxCommand::new()
        .send_keys()
        .target("zen-orchestrator:0.0")
        .key("echo 'Agent 1 starting...'")
        .output()?;

    TmuxCommand::new()
        .send_keys()
        .target("zen-orchestrator:0.1")
        .key("tail -f agent2.log")
        .output()?;

    Ok(())
}

fn execute_in_pane(session: &str, pane: usize, command: &str) -> Result<(), Box<dyn std::error::Error>> {
    TmuxCommand::new()
        .send_keys()
        .target(&format!("{}:{}.{}", session, 0, pane))
        .key(command)
        .output()?;

    // Send Enter key
    TmuxCommand::new()
        .send_keys()
        .target(&format!("{}:{}.{}", session, 0, pane))
        .key("")  // Empty string = Enter
        .output()?;

    Ok(())
}

fn capture_pane_output(session: &str, pane: usize) -> Result<String, Box<dyn std::error::Error>> {
    let output = TmuxCommand::new()
        .capture_pane()
        .print()
        .target(&format!("{}:{}.{}", session, 0, pane))
        .output()?;

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}
```

**Alternative: Direct CLI Calls**:
```rust
use std::process::Command;

fn tmux_create_session(name: &str) -> Result<(), std::io::Error> {
    Command::new("tmux")
        .args(["new-session", "-d", "-s", name])
        .output()?;
    Ok(())
}

fn tmux_split_horizontal(target: &str) -> Result<(), std::io::Error> {
    Command::new("tmux")
        .args(["split-window", "-h", "-t", target])
        .output()?;
    Ok(())
}

fn tmux_send_command(target: &str, cmd: &str) -> Result<(), std::io::Error> {
    Command::new("tmux")
        .args(["send-keys", "-t", target, cmd, "C-m"])
        .output()?;
    Ok(())
}

fn tmux_attach(session: &str) -> Result<(), std::io::Error> {
    Command::new("tmux")
        .args(["attach-session", "-t", session])
        .status()?;
    Ok(())
}
```

**Pros**:
- Full control over tmux sessions
- Can create complex layouts programmatically
- Capture output from panes
- Send commands and interact with running processes
- Works with existing tmux installations

**Cons**:
- Requires tmux to be installed
- Platform-specific (Unix-like systems only)
- More complex than managing processes directly
- Debugging can be challenging

**Recommendation**: ✅ **Recommended for Advanced Use** - Excellent for providing a familiar tmux interface for developers who want to inspect/interact with agents. Consider making it optional (fallback to direct process management).

---

### 3.2 tmux Integration Architecture for Zen

```rust
pub enum ExecutionMode {
    Direct,      // Spawn processes directly
    Tmux,        // Use tmux sessions for visibility
}

pub struct AgentExecutor {
    mode: ExecutionMode,
    tmux_session: Option<String>,
}

impl AgentExecutor {
    pub fn spawn_agent(&self, agent: &Agent) -> Result<AgentHandle, Error> {
        match self.mode {
            ExecutionMode::Direct => {
                // Use tokio::process::Command
                self.spawn_direct(agent)
            }
            ExecutionMode::Tmux => {
                // Create tmux pane and execute
                self.spawn_in_tmux(agent)
            }
        }
    }

    fn spawn_in_tmux(&self, agent: &Agent) -> Result<AgentHandle, Error> {
        let session = self.tmux_session.as_ref().unwrap();
        let pane_id = self.create_pane(session)?;

        // Send agent command to pane
        tmux_send_command(&format!("{}:{}", session, pane_id), &agent.command)?;

        Ok(AgentHandle {
            id: agent.id,
            pane_id: Some(pane_id),
            process: None,
        })
    }
}
```

---

## 4. Git Operations

### 4.1 git2-rs (libgit2)

**Crate**: [`git2`](https://crates.io/crates/git2)
**Version**: 0.19.x

**Key Features**:
- Rust bindings to libgit2
- Complete Git operations without Git CLI
- Worktree management
- References (refs) manipulation
- Notes support
- Branch, commit, tag operations
- Repository status and diff
- No Git CLI dependency

**Relevant for Zen**:
```rust
use git2::{Repository, Worktree, WorktreeAddOptions, Note};
use std::path::Path;

// Open repository
fn open_repo(path: &str) -> Result<Repository, git2::Error> {
    Repository::open(path)
}

// Create worktree
fn create_worktree(
    repo: &Repository,
    name: &str,
    path: &Path,
    branch: &str,
) -> Result<Worktree, git2::Error> {
    let commit = repo.head()?.peel_to_commit()?;
    let branch_ref = repo.branch(branch, &commit, false)?;

    let mut opts = WorktreeAddOptions::new();
    opts.reference(Some(branch_ref.get()));

    repo.worktree(name, path, Some(&opts))
}

// List all worktrees
fn list_worktrees(repo: &Repository) -> Result<Vec<String>, git2::Error> {
    let worktrees = repo.worktrees()?;
    Ok(worktrees.iter()
        .filter_map(|w| w.map(|s| s.to_string()))
        .collect())
}

// Manage git notes (for state storage)
fn add_note(
    repo: &Repository,
    namespace: &str,
    commit_oid: git2::Oid,
    note_content: &str,
) -> Result<git2::Oid, git2::Error> {
    let sig = repo.signature()?;
    repo.note(
        &sig,
        &sig,
        Some(namespace),
        commit_oid,
        note_content,
        false,
    )
}

fn read_note(
    repo: &Repository,
    namespace: &str,
    commit_oid: git2::Oid,
) -> Result<String, git2::Error> {
    let note = repo.find_note(Some(namespace), commit_oid)?;
    Ok(note.message().unwrap_or("").to_string())
}

// Create and manage refs under refs/zen/
fn create_zen_ref(
    repo: &Repository,
    ref_name: &str,
    commit_oid: git2::Oid,
) -> Result<(), git2::Error> {
    let full_ref = format!("refs/zen/{}", ref_name);
    repo.reference(&full_ref, commit_oid, false, "Create zen ref")?;
    Ok(())
}

fn read_zen_refs(repo: &Repository) -> Result<Vec<String>, git2::Error> {
    let refs = repo.references_glob("refs/zen/**")?;
    let mut zen_refs = Vec::new();

    for ref_result in refs {
        let reference = ref_result?;
        if let Some(name) = reference.name() {
            zen_refs.push(name.to_string());
        }
    }

    Ok(zen_refs)
}

// Example: Store agent state in git notes
fn store_agent_state(
    repo: &Repository,
    agent_id: &str,
    state: &AgentState,
) -> Result<(), Box<dyn std::error::Error>> {
    let head = repo.head()?.peel_to_commit()?;
    let state_json = serde_json::to_string(state)?;
    let namespace = format!("refs/notes/zen/agents/{}", agent_id);

    add_note(repo, &namespace, head.id(), &state_json)?;
    Ok(())
}
```

**Pros**:
- Pure Rust, no Git CLI required
- Type-safe API
- Comprehensive Git functionality
- Good performance
- Well-maintained (used by cargo, GitHub Desktop, etc.)
- Perfect for worktrees, refs, and notes

**Cons**:
- Learning curve (need to understand libgit2 concepts)
- Error handling can be verbose
- Some advanced Git features may require CLI fallback

**Recommendation**: ✅ **Highly Recommended** - Essential for Zen's git-based state management. Use it for worktrees, refs under `refs/zen/`, and notes under `refs/notes/zen/`.

---

### 4.2 Git Integration Architecture for Zen

```rust
use git2::Repository;
use serde::{Deserialize, Serialize};

pub struct ZenGitManager {
    repo: Repository,
    worktree_base: PathBuf,
}

impl ZenGitManager {
    pub fn new(repo_path: &str) -> Result<Self, git2::Error> {
        let repo = Repository::open(repo_path)?;
        let worktree_base = PathBuf::from(repo_path)
            .parent()
            .unwrap()
            .join("zen-worktrees");

        std::fs::create_dir_all(&worktree_base)?;

        Ok(Self { repo, worktree_base })
    }

    pub fn create_agent_worktree(&self, agent_id: &str) -> Result<PathBuf, git2::Error> {
        let branch_name = format!("zen/agent/{}", agent_id);
        let worktree_path = self.worktree_base.join(agent_id);

        create_worktree(&self.repo, agent_id, &worktree_path, &branch_name)?;
        Ok(worktree_path)
    }

    pub fn save_agent_state(&self, agent_id: &str, state: &AgentState) -> Result<(), Error> {
        let namespace = format!("refs/notes/zen/agents/{}", agent_id);
        let state_json = serde_json::to_string(state)?;
        let head = self.repo.head()?.peel_to_commit()?;

        add_note(&self.repo, &namespace, head.id(), &state_json)?;
        Ok(())
    }

    pub fn load_agent_state(&self, agent_id: &str) -> Result<AgentState, Error> {
        let namespace = format!("refs/notes/zen/agents/{}", agent_id);
        let head = self.repo.head()?.peel_to_commit()?;

        let state_json = read_note(&self.repo, &namespace, head.id())?;
        let state: AgentState = serde_json::from_str(&state_json)?;
        Ok(state)
    }
}

#[derive(Serialize, Deserialize)]
pub struct AgentState {
    pub status: String,
    pub started_at: u64,
    pub completed_at: Option<u64>,
    pub exit_code: Option<i32>,
    pub dependencies: Vec<String>,
}
```

---

## 5. Process Management (async/tokio)

### 5.1 tokio

**Crate**: [`tokio`](https://crates.io/crates/tokio)
**Version**: 1.x (stable)

**Key Features**:
- Industry-standard async runtime for Rust
- Multi-threaded work-stealing scheduler
- Async process spawning and management
- Channels for inter-task communication
- Timers and timeouts
- Async filesystem operations
- Signals handling
- Used by most major Rust async projects

**Relevant for Zen**:
```rust
use tokio::process::Command;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::sync::mpsc;
use tokio::time::{sleep, Duration};

// Spawn an agent process asynchronously
async fn spawn_agent(agent_id: &str, command: &str) -> Result<AgentHandle, std::io::Error> {
    let mut child = Command::new("sh")
        .arg("-c")
        .arg(command)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()?;

    let stdout = child.stdout.take().unwrap();
    let stderr = child.stderr.take().unwrap();

    // Stream stdout
    tokio::spawn(async move {
        let reader = BufReader::new(stdout);
        let mut lines = reader.lines();
        while let Ok(Some(line)) = lines.next_line().await {
            println!("[{}] {}", agent_id, line);
        }
    });

    // Stream stderr
    tokio::spawn(async move {
        let reader = BufReader::new(stderr);
        let mut lines = reader.lines();
        while let Ok(Some(line)) = lines.next_line().await {
            eprintln!("[{}] ERROR: {}", agent_id, line);
        }
    });

    Ok(AgentHandle {
        id: agent_id.to_string(),
        child,
    })
}

// Parallel agent execution with dependency management
async fn execute_dag(dag: &AgentDAG) -> Result<(), Box<dyn std::error::Error>> {
    let (tx, mut rx) = mpsc::channel::<AgentResult>(100);
    let mut active_agents = HashMap::new();
    let mut completed = HashSet::new();

    loop {
        // Find ready agents (all dependencies completed)
        let ready = dag.get_ready_agents(&completed);

        if ready.is_empty() && active_agents.is_empty() {
            break; // All done
        }

        // Spawn ready agents
        for agent in ready {
            let tx = tx.clone();
            let handle = tokio::spawn(async move {
                let result = run_agent(&agent).await;
                let _ = tx.send(result).await;
            });
            active_agents.insert(agent.id.clone(), handle);
        }

        // Wait for any agent to complete
        if let Some(result) = rx.recv().await {
            completed.insert(result.agent_id.clone());
            active_agents.remove(&result.agent_id);

            if result.success {
                println!("✓ Agent {} completed", result.agent_id);
            } else {
                eprintln!("✗ Agent {} failed", result.agent_id);
                // Handle failure: cancel dependents, retry, etc.
            }
        }
    }

    Ok(())
}

// Monitor agent with timeout
async fn run_agent_with_timeout(
    agent: &Agent,
    timeout: Duration,
) -> Result<AgentResult, Box<dyn std::error::Error>> {
    tokio::select! {
        result = run_agent(agent) => Ok(result),
        _ = sleep(timeout) => Err("Agent timeout".into()),
    }
}

// Inter-agent communication via channels
async fn agent_coordinator() {
    let (tx, mut rx) = mpsc::channel::<AgentMessage>(100);

    // Agent 1: Producer
    let tx1 = tx.clone();
    tokio::spawn(async move {
        let data = compute_something().await;
        tx1.send(AgentMessage::DataReady(data)).await.unwrap();
    });

    // Agent 2: Consumer
    tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            match msg {
                AgentMessage::DataReady(data) => {
                    process_data(data).await;
                }
                _ => {}
            }
        }
    });
}
```

**Pros**:
- Industry standard, extremely mature
- Excellent performance and scalability
- Rich ecosystem (tokio-util, tokio-stream, etc.)
- Great documentation and community
- Built-in process management
- Perfect for parallel agent orchestration
- Async/await syntax is ergonomic

**Cons**:
- Adds complexity vs synchronous code
- Runtime overhead (minimal but present)
- Requires understanding async concepts

**Recommendation**: ✅ **Essential** - tokio is the foundation for async Rust. Use it for all process management, parallel execution, and I/O operations in Zen.

---

### 5.2 Process Management Architecture for Zen

```rust
use tokio::process::{Child, Command};
use tokio::sync::{mpsc, RwLock};
use std::collections::HashMap;
use std::sync::Arc;

pub struct ZenOrchestrator {
    agents: Arc<RwLock<HashMap<String, AgentHandle>>>,
    event_tx: mpsc::Sender<OrchestratorEvent>,
}

pub struct AgentHandle {
    id: String,
    process: Child,
    status: AgentStatus,
    started_at: std::time::Instant,
}

#[derive(Debug, Clone)]
pub enum AgentStatus {
    Pending,
    Running,
    Completed { exit_code: i32 },
    Failed { error: String },
}

#[derive(Debug)]
pub enum OrchestratorEvent {
    AgentStarted(String),
    AgentCompleted { id: String, exit_code: i32 },
    AgentFailed { id: String, error: String },
    OutputLine { id: String, line: String },
}

impl ZenOrchestrator {
    pub fn new() -> (Self, mpsc::Receiver<OrchestratorEvent>) {
        let (event_tx, event_rx) = mpsc::channel(1000);
        let agents = Arc::new(RwLock::new(HashMap::new()));

        (
            Self { agents, event_tx },
            event_rx,
        )
    }

    pub async fn spawn_agent(
        &self,
        id: String,
        command: String,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut child = Command::new("sh")
            .arg("-c")
            .arg(&command)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()?;

        // Capture stdout
        let stdout = child.stdout.take().unwrap();
        let id_clone = id.clone();
        let tx_clone = self.event_tx.clone();
        tokio::spawn(async move {
            let reader = BufReader::new(stdout);
            let mut lines = reader.lines();
            while let Ok(Some(line)) = lines.next_line().await {
                let _ = tx_clone
                    .send(OrchestratorEvent::OutputLine {
                        id: id_clone.clone(),
                        line,
                    })
                    .await;
            }
        });

        let handle = AgentHandle {
            id: id.clone(),
            process: child,
            status: AgentStatus::Running,
            started_at: std::time::Instant::now(),
        };

        self.agents.write().await.insert(id.clone(), handle);
        self.event_tx
            .send(OrchestratorEvent::AgentStarted(id))
            .await?;

        Ok(())
    }

    pub async fn wait_for_agent(&self, id: &str) -> Result<i32, Box<dyn std::error::Error>> {
        let mut agents = self.agents.write().await;
        let handle = agents.get_mut(id).ok_or("Agent not found")?;

        let exit_status = handle.process.wait().await?;
        let exit_code = exit_status.code().unwrap_or(-1);

        handle.status = AgentStatus::Completed { exit_code };

        self.event_tx
            .send(OrchestratorEvent::AgentCompleted {
                id: id.to_string(),
                exit_code,
            })
            .await?;

        Ok(exit_code)
    }

    pub async fn execute_parallel(&self, agents: Vec<AgentSpec>) -> Result<(), Box<dyn std::error::Error>> {
        let mut handles = Vec::new();

        for agent in agents {
            let orch = self.clone();
            let handle = tokio::spawn(async move {
                orch.spawn_agent(agent.id.clone(), agent.command).await?;
                orch.wait_for_agent(&agent.id).await
            });
            handles.push(handle);
        }

        // Wait for all agents
        for handle in handles {
            handle.await??;
        }

        Ok(())
    }
}
```

---

## 6. Additional Considerations

### 6.1 Structured Logging

**Crate**: [`tracing`](https://crates.io/crates/tracing)

```rust
use tracing::{info, warn, error, instrument};

#[instrument]
async fn execute_agent(id: &str) {
    info!(agent_id = %id, "Starting agent execution");
    // Agent logic
    info!(agent_id = %id, "Agent completed");
}
```

### 6.2 Configuration Management

**Crate**: [`serde`](https://crates.io/crates/serde) + [`toml`](https://crates.io/crates/toml) / [`yaml`](https://crates.io/crates/serde_yaml)

```rust
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize)]
struct ZenConfig {
    max_parallel_agents: usize,
    default_timeout: u64,
    tmux_enabled: bool,
    git_worktree_base: String,
}
```

### 6.3 Error Handling

**Crate**: [`thiserror`](https://crates.io/crates/thiserror) + [`anyhow`](https://crates.io/crates/anyhow)

```rust
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ZenError {
    #[error("Agent {0} failed with exit code {1}")]
    AgentFailed(String, i32),

    #[error("Circular dependency detected")]
    CircularDependency,

    #[error("Git operation failed: {0}")]
    GitError(#[from] git2::Error),
}
```

---

## 7. Summary and Recommended Stack

### Core Dependencies

```toml
[dependencies]
# DAG Management
petgraph = "0.6"
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"

# TUI
ratatui = "0.28"
crossterm = "0.28"

# Git Operations
git2 = "0.19"

# Async Runtime & Process Management
tokio = { version = "1", features = ["full"] }

# Error Handling
thiserror = "1.0"
anyhow = "1.0"

# Logging
tracing = "0.1"
tracing-subscriber = "0.3"

# Configuration
toml = "0.8"

# Optional: tmux integration
tmux_interface = "0.3"
```

### Architecture Overview

```
┌─────────────────────────────────────────────────────┐
│                   Zen CLI / TUI                     │
│              (ratatui + crossterm)                  │
├─────────────────────────────────────────────────────┤
│              ZenOrchestrator (tokio)                │
│  - DAG Scheduler (petgraph)                        │
│  - Process Manager (tokio::process)                │
│  - Event System (mpsc channels)                    │
├─────────────────────────────────────────────────────┤
│         Git Integration (git2)                      │
│  - Worktrees (refs/zen/worktrees/*)                │
│  - State (refs/notes/zen/*)                        │
│  - Refs (refs/zen/*)                               │
├─────────────────────────────────────────────────────┤
│    Optional: tmux Integration                       │
│  - Session Management                               │
│  - Pane Layout                                      │
│  - Command Execution                                │
└─────────────────────────────────────────────────────┘
```

### Key Design Principles

1. **Async by Default**: Use tokio for all I/O and process management
2. **Type Safety**: Leverage Rust's type system and petgraph's strongly-typed DAG
3. **Composable UI**: Build TUI with ratatui widgets that can be combined
4. **Git-Native State**: Use git2 for all repository operations (worktrees, refs, notes)
5. **Optional tmux**: Support tmux mode for advanced users, fallback to direct processes
6. **Event-Driven**: Use mpsc channels for orchestrator events and agent communication
7. **Structured Logging**: Use tracing for observability

---

## 8. Next Steps

1. **Prototype DAG Scheduler**: Build basic petgraph DAG with topological execution
2. **Implement Process Manager**: Create tokio-based agent spawning and monitoring
3. **Git Integration Layer**: Implement worktree creation and state management with git2
4. **Basic TUI**: Create ratatui dashboard showing agent status
5. **tmux Integration**: Add optional tmux mode for pane-based agent execution
6. **Testing**: Unit tests for DAG logic, integration tests for full orchestration

---

**Document Version**: 1.0
**Last Updated**: 2026-01-30
**Status**: Ready for implementation
