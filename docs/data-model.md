# Data Model Design

> Session state, persistence format, and configuration schema.

## Design Principles

1. **Minimal schema** - Only store what's necessary
2. **Human readable** - JSON for easy debugging and manual editing
3. **Fast to parse** - Simple structure, no deep nesting
4. **Never lose work** - Atomic writes, backup on save

---

## Directory Structure

```
~/.zen/
├── config.json          # User configuration (optional)
├── state.json           # Session state (auto-generated)
├── state.json.bak       # Backup of previous state
└── worktrees/           # Git worktrees for sessions
    ├── feature-auth_1705312200/
    ├── bugfix-login_1705312300/
    └── refactor-api_1705312400/
```

---

## Session State

### Rust Types

```rust
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SessionId(pub Uuid);

impl SessionId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub id: SessionId,
    pub name: String,
    pub branch: String,
    pub status: SessionStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub worktree_path: Option<PathBuf>,
    pub base_commit: String,
    pub created_at: DateTime<Utc>,
    #[serde(default = "Utc::now")]
    pub last_active: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum SessionStatus {
    #[default]
    Running,
    Paused,
}
```

### Transient State (Not Persisted)

```rust
#[derive(Debug, Default)]
pub struct SessionRuntime {
    pub diff_stats: Option<DiffStats>,
    pub diff_content: Option<String>,
    pub preview_content: Option<String>,
    pub tmux_name: Option<String>,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct DiffStats {
    pub additions: u32,
    pub deletions: u32,
}
```

---

## State File Format

`~/.zen/state.json`:

```json
{
  "version": 1,
  "sessions": [
    {
      "id": "550e8400-e29b-41d4-a716-446655440000",
      "name": "feature-auth",
      "branch": "alice/feature-auth",
      "status": "running",
      "worktree_path": "/home/alice/.zen/worktrees/feature-auth_1705312200",
      "base_commit": "abc123def456",
      "created_at": "2024-01-15T10:30:00Z",
      "last_active": "2024-01-15T14:22:00Z"
    },
    {
      "id": "550e8400-e29b-41d4-a716-446655440001",
      "name": "bugfix-login",
      "branch": "alice/bugfix-login",
      "status": "paused",
      "base_commit": "def456abc789",
      "created_at": "2024-01-15T09:00:00Z",
      "last_active": "2024-01-15T12:00:00Z"
    }
  ]
}
```

### Schema Definition

```rust
#[derive(Debug, Serialize, Deserialize, Default)]
pub struct State {
    #[serde(default = "default_version")]
    pub version: u32,
    #[serde(default)]
    pub sessions: Vec<Session>,
}

fn default_version() -> u32 { 1 }
```

---

## State Persistence

```rust
impl State {
    fn state_path() -> Result<PathBuf> {
        Ok(dirs::home_dir()
            .ok_or(Error::NoHomeDir)?
            .join(".zen")
            .join("state.json"))
    }

    pub fn load() -> Result<Self> {
        let path = Self::state_path()?;

        if !path.exists() {
            return Ok(Self::default());
        }

        let content = fs::read_to_string(&path)?;
        let state: State = serde_json::from_str(&content)?;

        Ok(state)
    }

    pub fn save(&self) -> Result<()> {
        let path = Self::state_path()?;
        let dir = path.parent().unwrap();

        fs::create_dir_all(dir)?;

        // Backup existing state
        if path.exists() {
            let backup = path.with_extension("json.bak");
            fs::copy(&path, &backup)?;
        }

        // Atomic write via temp file
        let temp_path = path.with_extension("json.tmp");
        let content = serde_json::to_string_pretty(self)?;

        {
            let mut file = fs::File::create(&temp_path)?;
            file.write_all(content.as_bytes())?;
            file.sync_all()?;
        }

        fs::rename(&temp_path, &path)?;

        Ok(())
    }
}
```

---

## Configuration

### Rust Types

```rust
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Config {
    #[serde(default)]
    pub agent: AgentKind,
    #[serde(default)]
    pub auto_yes: bool,
    #[serde(default)]
    pub custom_agents: Vec<CustomAgentConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomAgentConfig {
    pub name: String,
    pub binary: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auto_yes_flag: Option<String>,
}
```

### Config File Format

`~/.zen/config.json`:

```json
{
  "agent": "claude",
  "auto_yes": false
}
```

Extended with custom agent:

```json
{
  "agent": "claude",
  "auto_yes": false,
  "custom_agents": [
    {
      "name": "My Custom Agent",
      "binary": "my-agent",
      "args": ["--cwd", "{cwd}", "--prompt", "{prompt}"],
      "auto_yes_flag": "--auto"
    }
  ]
}
```

### Config Loading

```rust
impl Config {
    fn config_path() -> Result<PathBuf> {
        Ok(dirs::home_dir()
            .ok_or(Error::NoHomeDir)?
            .join(".zen")
            .join("config.json"))
    }

    pub fn load() -> Result<Self> {
        let path = Self::config_path()?;

        if !path.exists() {
            return Ok(Self::default());
        }

        let content = fs::read_to_string(&path)?;
        Ok(serde_json::from_str(&content)?)
    }
}
```

---

## Worktree Naming Convention

```rust
impl Session {
    pub fn worktree_path(name: &str) -> PathBuf {
        let timestamp = Utc::now().timestamp();
        let dirname = format!("{}_{}", sanitize_name(name), timestamp);

        dirs::home_dir()
            .expect("home dir exists")
            .join(".zen")
            .join("worktrees")
            .join(dirname)
    }

    pub fn tmux_name(&self) -> String {
        let short_id = &self.id.0.to_string()[..8];
        format!("zen_{}_{}", sanitize_name(&self.name), short_id)
    }
}

fn sanitize_name(name: &str) -> String {
    name.chars()
        .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
        .collect()
}
```

---

## Data Validation

```rust
impl Session {
    pub fn validate(&self) -> Result<()> {
        if self.name.trim().is_empty() {
            return Err(Error::Validation("Session name cannot be empty".into()));
        }

        if self.name.len() > 64 {
            return Err(Error::Validation("Session name too long".into()));
        }

        if self.status == SessionStatus::Running && self.worktree_path.is_none() {
            return Err(Error::Validation("Running session must have worktree".into()));
        }

        Ok(())
    }
}

impl State {
    pub fn validate(&self) -> Result<()> {
        let mut seen_ids = std::collections::HashSet::new();
        for session in &self.sessions {
            if !seen_ids.insert(session.id) {
                return Err(Error::Validation("Duplicate session ID".into()));
            }
            session.validate()?;
        }

        Ok(())
    }
}
```

---

## Recovery and Reconciliation

```rust
impl State {
    /// Reconcile state with actual filesystem on startup
    pub fn reconcile(&mut self) -> Vec<String> {
        let mut warnings = Vec::new();

        for session in &mut self.sessions {
            if session.status == SessionStatus::Running {
                // Check if worktree still exists
                if let Some(ref path) = session.worktree_path {
                    if !path.exists() {
                        warnings.push(format!(
                            "Session '{}': worktree missing, marking as paused",
                            session.name
                        ));
                        session.status = SessionStatus::Paused;
                        session.worktree_path = None;
                    }
                }

                // Check if tmux session exists
                let tmux_name = session.tmux_name();
                if !Tmux::session_exists(&tmux_name) {
                    warnings.push(format!(
                        "Session '{}': tmux session missing",
                        session.name
                    ));
                }
            }
        }

        warnings
    }

    /// Clean up orphaned worktrees
    pub fn cleanup_orphans(&self) -> Result<Vec<PathBuf>> {
        let worktrees_dir = dirs::home_dir()
            .ok_or(Error::NoHomeDir)?
            .join(".zen")
            .join("worktrees");

        if !worktrees_dir.exists() {
            return Ok(Vec::new());
        }

        let known_paths: std::collections::HashSet<_> = self
            .sessions
            .iter()
            .filter_map(|s| s.worktree_path.as_ref())
            .collect();

        let mut orphans = Vec::new();

        for entry in fs::read_dir(&worktrees_dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.is_dir() && !known_paths.contains(&path) {
                orphans.push(path);
            }
        }

        Ok(orphans)
    }
}
```

---

## Summary

| File | Purpose | Schema |
|------|---------|--------|
| `~/.zen/config.json` | User preferences | `Config` |
| `~/.zen/state.json` | Session persistence | `State` |
| `~/.zen/state.json.bak` | Backup | Same as state |
| `~/.zen/worktrees/` | Git worktrees | Directories |

**Key invariants:**
1. State is always valid JSON
2. Atomic writes prevent corruption
3. Backup enables manual recovery
4. Reconciliation handles external changes
5. Transient data (diffs, previews) never persisted
