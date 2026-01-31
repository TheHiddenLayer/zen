# Context: Workflow Core Types Implementation

## Project Overview

**Project:** Zen - Parallel AI Agent Orchestrator (Rust TUI application)
**Task:** Create foundational workflow type definitions

## Requirements

### Functional Requirements

1. **WorkflowId** - UUID-based newtype for uniquely identifying workflows
   - `WorkflowId::new()` - Create new random UUID-based ID
   - `WorkflowId::short()` - Return first 8 characters of UUID for display
   - Implement `Display`, `FromStr` traits
   - Derive: `Debug`, `Clone`, `Copy`, `PartialEq`, `Eq`, `Hash`, `Serialize`, `Deserialize`

2. **WorkflowPhase** - Enum representing workflow execution phases
   - Variants: `Planning`, `TaskGeneration`, `Implementation`, `Merging`, `Documentation`, `Complete`
   - Must support ordering (PartialOrd, Ord) for phase comparison
   - Implement `Display` for human-readable output
   - Derive: `Debug`, `Clone`, `Copy`, `PartialEq`, `Eq`, `Hash`, `Serialize`, `Deserialize`

3. **WorkflowStatus** - Enum representing workflow lifecycle status
   - Variants: `Pending`, `Running`, `Paused`, `Completed`, `Failed`
   - Implement `Display` for human-readable output
   - Derive: `Debug`, `Clone`, `Copy`, `PartialEq`, `Eq`, `Hash`, `Serialize`, `Deserialize`

### Acceptance Criteria

1. `WorkflowId::new()` generates unique UUIDs with short form display
2. WorkflowPhase ordering: Planning < TaskGeneration < Implementation < Merging < Documentation < Complete
3. All types round-trip through JSON serialization correctly
4. `cargo test workflow` passes all tests

## Existing Patterns

### SessionId Pattern (from src/session.rs)

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SessionId(pub Uuid);

impl SessionId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    pub fn short(&self) -> String {
        self.0.to_string()[..8].to_string()
    }
}

impl Default for SessionId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for SessionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::str::FromStr for SessionId {
    type Err = uuid::Error;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        Ok(Self(Uuid::parse_str(s)?))
    }
}
```

### SessionStatus Pattern (from src/session.rs)

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum SessionStatus {
    #[default]
    Running,
    Locked,
}

impl std::fmt::Display for SessionStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SessionStatus::Running => write!(f, "running"),
            SessionStatus::Locked => write!(f, "locked"),
        }
    }
}
```

## Dependencies

- `uuid` (v1 with v4 and serde features) - already in Cargo.toml
- `serde` (v1 with derive feature) - already in Cargo.toml
- `serde_json` (v1) - already in Cargo.toml for testing

## Implementation Path

1. Create `src/workflow/mod.rs` - Module exports
2. Create `src/workflow/types.rs` - Type definitions with tests
3. Update `src/lib.rs` - Add workflow module

## Module Structure

```
src/
  workflow/
    mod.rs      # Module exports (pub mod types; pub use types::*)
    types.rs    # WorkflowId, WorkflowPhase, WorkflowStatus definitions + tests
```
