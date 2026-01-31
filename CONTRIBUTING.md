# Contributing to Zen

Thank you for your interest in contributing to Zen! This guide will help you get started with development.

---

## Development Setup

### Prerequisites

- **Rust 1.70+** - Install via [rustup](https://rustup.rs/)
- **Git** - For version control and worktree operations
- **Tmux** - Terminal multiplexer for agent sessions
- **Claude Code CLI** - For running AI agents (optional for tests)

### Clone and Build

```bash
# Clone the repository
git clone https://github.com/your-org/zen.git
cd zen

# Build in debug mode
cargo build

# Build in release mode
cargo build --release

# Run tests
cargo test

# Run with debug logging
RUST_LOG=debug cargo run
```

### Development Tools

```bash
# Format code
cargo fmt

# Run linter
cargo clippy

# Check without building
cargo check

# Generate documentation
cargo doc --open
```

---

## Code Style Guide

### General Principles

1. **100% Safe Rust** - No `unsafe` blocks unless absolutely necessary
2. **Explicit over implicit** - Avoid magic, prefer clear code
3. **Fail fast** - Propagate errors early, use `?` operator
4. **Small functions** - Each function does one thing well
5. **Descriptive names** - Variables and functions should be self-documenting

### Naming Conventions

```rust
// Structs: PascalCase
pub struct TaskDAG { ... }

// Functions/methods: snake_case
pub fn ready_tasks(&self) -> Vec<&Task> { ... }

// Constants: SCREAMING_SNAKE_CASE
const MAX_RETRIES: u32 = 3;

// Modules: snake_case
mod orchestration;

// Type parameters: single uppercase letter or PascalCase
impl<T: Agent> Pool<T> { ... }
```

### Error Handling

```rust
// Use thiserror for custom errors
#[derive(Error, Debug)]
pub enum ZenError {
    #[error("Task not found: {0}")]
    TaskNotFound(TaskId),

    #[error("Git operation failed: {0}")]
    Git(#[from] git2::Error),
}

// Use Result with ? operator
pub fn load_task(&self, id: &TaskId) -> Result<Task> {
    let data = self.read_note(id)?;
    let task: Task = serde_json::from_str(&data)?;
    Ok(task)
}
```

### Documentation

```rust
/// Short description of what this function does.
///
/// More detailed explanation if needed. Include examples
/// for complex functions.
///
/// # Arguments
///
/// * `task` - The task to execute
///
/// # Returns
///
/// The execution result with commit hash on success.
///
/// # Errors
///
/// Returns `ZenError::TaskFailed` if the agent fails.
///
/// # Example
///
/// ```
/// let result = scheduler.execute(&task)?;
/// println!("Commit: {}", result.commit_hash);
/// ```
pub fn execute(&self, task: &Task) -> Result<ExecResult> {
    // ...
}
```

### Module Organization

```rust
// mod.rs - Re-export public items
pub mod scheduler;
pub mod planner;

pub use scheduler::{Scheduler, SchedulerEvent};
pub use planner::{ReactivePlanner, PlanEvent};

// Keep modules focused - one concept per file
// orchestration/scheduler.rs - only scheduler logic
// orchestration/planner.rs - only planner logic
```

### Import Order

```rust
// 1. Standard library
use std::collections::HashMap;
use std::path::PathBuf;

// 2. External crates
use petgraph::graph::DiGraph;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

// 3. Internal modules
use crate::core::task::Task;
use crate::error::Result;
```

---

## Testing Requirements

### Test Categories

| Category | Location | Command |
|----------|----------|---------|
| Unit tests | Inline in `src/**/*.rs` | `cargo test` |
| Integration tests | `tests/integration/` | `cargo test --test integration` |
| Performance tests | `tests/integration/performance.rs` | `cargo test performance` |

### Writing Unit Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_task_status_transitions() {
        let mut task = Task::new("test", "description");
        assert_eq!(task.status, TaskStatus::Pending);

        task.start();
        assert_eq!(task.status, TaskStatus::Running);

        task.complete("abc123".to_string());
        assert_eq!(task.status, TaskStatus::Completed);
    }

    #[tokio::test]
    async fn test_async_operation() {
        let result = some_async_fn().await;
        assert!(result.is_ok());
    }
}
```

### Writing Integration Tests

Integration tests go in `tests/integration/`:

```rust
// tests/integration/workflow_e2e.rs
use zen::orchestration::Scheduler;
use zen::core::dag::TaskDAG;

#[tokio::test]
async fn test_e2e_happy_path() {
    let repo = TestRepo::new();
    let dag = create_test_dag();
    let scheduler = Scheduler::new(dag, 4, &repo.path);

    let results = scheduler.run().await.unwrap();
    assert_eq!(results.len(), 3);
}
```

### Test Coverage Goals

| Module | Target |
|--------|--------|
| `core/dag.rs` | 90%+ |
| `core/task.rs` | 90%+ |
| `orchestration/scheduler.rs` | 80%+ |
| `orchestration/pool.rs` | 75%+ |
| `state/manager.rs` | 85%+ |

### CI-Safe Tests

Tests must run without external dependencies:

```rust
// GOOD: Uses mock/stub
#[test]
fn test_with_mock_claude() {
    let mock = MockClaudeResponder::new();
    // ...
}

// BAD: Requires real Claude API
#[test]
fn test_with_real_claude() {
    let claude = ClaudeHeadless::new(); // Fails in CI
    // ...
}
```

---

## Pull Request Process

### Before Submitting

1. **Run all tests:**
   ```bash
   cargo test
   cargo test --test integration
   ```

2. **Format and lint:**
   ```bash
   cargo fmt
   cargo clippy -- -D warnings
   ```

3. **Update documentation** if you changed public APIs

4. **Add tests** for new functionality

### PR Checklist

- [ ] Code compiles without warnings (`cargo check`)
- [ ] All tests pass (`cargo test`)
- [ ] Code is formatted (`cargo fmt --check`)
- [ ] No clippy warnings (`cargo clippy -- -D warnings`)
- [ ] Documentation updated for public API changes
- [ ] Tests added for new functionality
- [ ] Commit messages follow conventional commits

### Commit Message Format

Use [Conventional Commits](https://www.conventionalcommits.org/):

```
<type>(<scope>): <description>

[optional body]

[optional footer]
```

**Types:**
- `feat` - New feature
- `fix` - Bug fix
- `docs` - Documentation only
- `style` - Formatting, no code change
- `refactor` - Code restructuring
- `test` - Adding tests
- `chore` - Maintenance tasks

**Examples:**
```
feat(scheduler): add priority-based task ordering

fix(resolver): handle empty conflict list gracefully

docs: add skills integration guide

test(dag): add cycle detection tests
```

### Review Process

1. **Create PR** with clear description
2. **CI checks** must pass
3. **Code review** by at least one maintainer
4. **Address feedback** via additional commits
5. **Squash and merge** when approved

---

## Architecture Guidelines

### TEA Pattern

Follow The Elm Architecture for state management:

```rust
// Model - Application state
pub struct Model {
    pub sessions: Vec<Session>,
    pub selected: usize,
    pub mode: Mode,
}

// Message - Events that can change state
pub enum Message {
    KeyPress(KeyEvent),
    SessionCreated(Session),
    Error(String),
}

// Update - Pure function: (Model, Message) -> Commands
pub fn update(model: &mut Model, msg: Message) -> Vec<Command> {
    match msg {
        Message::KeyPress(key) => handle_key(model, key),
        Message::SessionCreated(s) => {
            model.sessions.push(s);
            vec![]
        }
        // ...
    }
}
```

### Two-Thread Design

Never block the render thread:

```rust
// GOOD: Async operation in logic thread
async fn load_data() -> Result<Data> {
    tokio::spawn_blocking(|| {
        // Heavy operation
    }).await?
}

// BAD: Blocking in render path
fn render(state: &RenderState) {
    let data = std::fs::read_to_string("file"); // BLOCKS!
}
```

### Git-Native State

Store state in git, not external files:

```rust
// GOOD: Git notes for metadata
self.notes.set(&task.id, &serde_json::to_string(&task)?)?;

// BAD: External JSON file
std::fs::write("tasks.json", serde_json::to_string(&tasks)?)?;
```

---

## Getting Help

- **Issues:** Report bugs or request features on GitHub
- **Discussions:** Ask questions in GitHub Discussions
- **Code:** Look at existing code for patterns and conventions

---

## License

By contributing, you agree that your contributions will be licensed under the MIT License.
