//! Session management for the Zen TUI.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use uuid::Uuid;

use crate::actors::SessionInfo;
use crate::agent::Agent;
use crate::config::Config;
use crate::git::GitOps;
use crate::tmux::Tmux;
use crate::util::blocking;
use crate::{zlog, zlog_debug, zlog_error, zlog_warn, Error, Result};

const MAX_SESSION_NAME_LENGTH: usize = 64;
const STATE_VERSION: u32 = 1;

/// Summary of resources cleaned up during orphan cleanup.
#[derive(Debug, Default)]
pub struct CleanupSummary {
    /// Names of tmux sessions that were killed
    pub tmux_sessions: Vec<String>,
    /// Paths of worktree directories that were removed
    pub worktrees: Vec<PathBuf>,
}

impl CleanupSummary {
    pub fn is_empty(&self) -> bool {
        self.tmux_sessions.is_empty() && self.worktrees.is_empty()
    }

    pub fn total_cleaned(&self) -> usize {
        self.tmux_sessions.len() + self.worktrees.len()
    }
}

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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub id: SessionId,
    pub name: String,
    pub branch: String,
    pub status: SessionStatus,
    pub worktree_path: Option<PathBuf>,
    pub base_commit: String,
    /// Base branch name (e.g., "main", "master", "develop")
    #[serde(default)]
    pub base_branch: String,
    pub created_at: DateTime<Utc>,
    pub last_active: DateTime<Utc>,
    pub agent: String,
    /// Repository/project name (e.g., "my-app")
    #[serde(default)]
    pub project: String,
}

impl Session {
    pub async fn create(
        name: &str,
        repo_path: &Path,
        agent: &Agent,
        prompt: Option<&str>,
    ) -> Result<Self> {
        zlog!(
            "Session::create name={} repo={} agent={} prompt={:?}",
            name,
            repo_path.display(),
            agent.name(),
            prompt.map(|s| s.chars().take(50).collect::<String>())
        );

        validate_session_name(name)?;
        blocking(Config::ensure_dirs).await?;

        let id = SessionId::new();
        let sanitized_name = sanitize_name(name);
        let timestamp = Utc::now().timestamp();
        let worktree_name = format!("{}_{}", sanitized_name, timestamp);
        let worktree_path = blocking(Config::worktrees_dir).await?.join(&worktree_name);
        let tmux_name = Tmux::session_name(&sanitized_name, &id.short());

        zlog_debug!(
            "Session '{}' id={} tmux_name={} worktree={}",
            name,
            id.short(),
            tmux_name,
            worktree_path.display()
        );

        let repo_path_owned = repo_path.to_path_buf();
        let sanitized_name_clone = sanitized_name.clone();
        let worktree_path_clone = worktree_path.clone();

        // Get git user and create branch as {user}/{task-name}
        let (base_commit, base_branch, branch, project) = blocking(move || {
            let git = GitOps::new(&repo_path_owned)?;
            let git_user = git.git_user()?;
            let branch = format!("{}/{}", git_user, sanitized_name_clone);
            let base_commit = git.head_commit()?;
            let base_branch = git.current_head()?;
            let project = git.repo_name();
            zlog_debug!(
                "Creating worktree: branch={} base_branch={} project={}",
                branch,
                base_branch,
                project
            );
            git.create_worktree(&branch, &worktree_path_clone)?;
            Ok((base_commit, base_branch, branch, project))
        })
        .await?;

        let cmd = agent.command(prompt);
        zlog_debug!("Session '{}' tmux cmd={:?}", name, cmd);

        let tmux_name_clone = tmux_name.clone();
        let worktree_path_clone = worktree_path.clone();

        blocking(move || Tmux::create_session(&tmux_name_clone, &worktree_path_clone, &cmd))
            .await?;

        let now = Utc::now();

        zlog!(
            "Session created: name={} id={} branch={} project={}",
            name,
            id.short(),
            branch,
            project
        );

        Ok(Self {
            id,
            name: name.to_string(),
            branch,
            status: SessionStatus::Running,
            worktree_path: Some(worktree_path),
            base_commit,
            base_branch,
            created_at: now,
            last_active: now,
            agent: agent.name().to_string(),
            project,
        })
    }

    pub async fn lock(&mut self, git: &GitOps) -> Result<()> {
        zlog!("Session::lock name={} id={}", self.name, self.id.short());

        if self.status == SessionStatus::Locked {
            zlog_debug!("Session '{}' already locked", self.name);
            return Ok(());
        }

        let worktree_path = self
            .worktree_path
            .as_ref()
            .ok_or_else(|| Error::Validation("Session has no worktree path".to_string()))?
            .clone();

        // Check if worktree exists
        if !worktree_path.exists() {
            zlog_warn!(
                "Session '{}' worktree does not exist: {}",
                self.name,
                worktree_path.display()
            );
            return Err(Error::Validation(format!(
                "Worktree path does not exist: {}",
                worktree_path.display()
            )));
        }

        // Auto-commit if dirty (abort lock if commit fails)
        if git.is_dirty(&worktree_path)? {
            zlog_debug!(
                "Session '{}' has dirty worktree, auto-committing",
                self.name
            );
            let commit_msg = format!(
                "[zen] update from '{}' on {} (locked)",
                self.name,
                chrono::Local::now().format("%d %b %y %H:%M %Z")
            );
            git.commit_all(&worktree_path, &commit_msg)?;
        }

        // Remove worktree (but keep branch)
        zlog_debug!("Session '{}' removing worktree", self.name);
        git.remove_worktree(&worktree_path)?;

        // NOTE: We intentionally do NOT kill the tmux session
        // This preserves scrollback history and agent context

        self.status = SessionStatus::Locked;
        self.last_active = Utc::now();

        zlog!("Session locked: name={} id={}", self.name, self.id.short());
        Ok(())
    }

    pub async fn unlock(&mut self, git: &GitOps, agent: &Agent) -> Result<()> {
        zlog!("Session::unlock name={} id={}", self.name, self.id.short());

        if self.status == SessionStatus::Running {
            zlog_debug!("Session '{}' already running", self.name);
            return Ok(());
        }

        let worktree_path = self
            .worktree_path
            .as_ref()
            .ok_or_else(|| Error::Validation("Session has no worktree path".to_string()))?
            .clone();

        // Check branch exists
        if !git.branch_exists(&self.branch)? {
            zlog_warn!(
                "Session '{}' branch no longer exists: {}",
                self.name,
                self.branch
            );
            return Err(Error::Validation(format!(
                "Branch '{}' no longer exists",
                self.branch
            )));
        }

        // Recreate worktree from existing branch
        zlog_debug!(
            "Session '{}' recreating worktree from branch {}",
            self.name,
            self.branch
        );
        git.create_worktree_from_branch(&self.branch, &worktree_path)?;

        let tmux_name = self.tmux_name();

        // Check if tmux session survived
        if !Tmux::session_exists(&tmux_name) {
            zlog_debug!(
                "Session '{}' tmux session died, creating new one",
                self.name
            );
            // Create new tmux session
            let cmd = agent.command(None);

            let tmux_name_for_create = tmux_name.clone();
            let worktree_path_clone = worktree_path.clone();

            blocking(move || {
                Tmux::create_session(&tmux_name_for_create, &worktree_path_clone, &cmd)
            })
            .await?;
        } else {
            zlog_debug!(
                "Session '{}' tmux session survived, preserving context",
                self.name
            );
        }

        self.status = SessionStatus::Running;
        self.last_active = Utc::now();

        zlog!(
            "Session unlocked: name={} id={}",
            self.name,
            self.id.short()
        );
        Ok(())
    }

    /// Delete a session and clean up all associated resources.
    ///
    /// This function attempts to clean up all resources even if some operations fail:
    /// - Kills the tmux session
    /// - Removes the git worktree directory
    /// - Deletes the git branch
    ///
    /// Errors are collected and reported, but cleanup continues for all resources.
    pub async fn delete(self, repo_path: &Path) -> Result<()> {
        zlog!(
            "Session::delete name={} id={} repo={}",
            self.name,
            self.id.short(),
            repo_path.display()
        );

        let tmux_name = self.tmux_name();
        let worktree_path = self.worktree_path.clone();
        let branch = self.branch.clone();
        let repo_path_owned = repo_path.to_path_buf();
        let session_name = self.name.clone();

        blocking(move || {
            let mut errors: Vec<String> = Vec::new();

            // 1. Kill tmux session first to stop any running processes
            zlog_debug!(
                "Session '{}': killing tmux session '{}'",
                session_name,
                tmux_name
            );
            if let Err(e) = Tmux::kill_session(&tmux_name) {
                zlog_warn!("Failed to kill tmux session '{}': {}", tmux_name, e);
                // Don't add to errors - session might already be dead
            }

            // 2. Initialize git operations
            let git = match GitOps::new(&repo_path_owned) {
                Ok(g) => Some(g),
                Err(e) => {
                    let msg = format!("Failed to initialize git operations: {}", e);
                    zlog_error!("{}", msg);
                    errors.push(msg);
                    None
                }
            };

            // 3. Remove worktree directory
            if let Some(wt_path) = &worktree_path {
                zlog_debug!(
                    "Session '{}': removing worktree '{}'",
                    session_name,
                    wt_path.display()
                );
                if wt_path.exists() {
                    if let Some(ref git) = git {
                        if let Err(e) = git.remove_worktree(wt_path) {
                            let msg =
                                format!("Failed to remove worktree '{}': {}", wt_path.display(), e);
                            zlog_error!("{}", msg);
                            errors.push(msg);

                            // Try direct removal as fallback
                            zlog_debug!("Attempting direct removal of worktree directory");
                            if let Err(e) = std::fs::remove_dir_all(wt_path) {
                                let msg =
                                    format!("Failed to directly remove worktree directory: {}", e);
                                zlog_error!("{}", msg);
                                errors.push(msg);
                            }
                        }
                    } else {
                        // No git ops available, try direct removal
                        if let Err(e) = std::fs::remove_dir_all(wt_path) {
                            let msg = format!(
                                "Failed to remove worktree directory '{}': {}",
                                wt_path.display(),
                                e
                            );
                            zlog_error!("{}", msg);
                            errors.push(msg);
                        }
                    }
                }
            }

            // 4. Delete git branch
            if let Some(ref git) = git {
                zlog_debug!("Session '{}': deleting branch '{}'", session_name, branch);
                if let Err(e) = git.delete_branch(&branch) {
                    let msg = format!("Failed to delete branch '{}': {}", branch, e);
                    zlog_error!("{}", msg);
                    errors.push(msg);

                    // Try force delete as fallback
                    if let Err(e) = git.force_delete_branch_ref(&branch) {
                        zlog_warn!("Force delete branch ref also failed: {}", e);
                    }
                }
            }

            // Report any errors that occurred
            if errors.is_empty() {
                zlog!("Session deleted: name={}", session_name);
                Ok(())
            } else {
                let combined_error = errors.join("; ");
                zlog_warn!(
                    "Session '{}' deletion completed with errors: {}",
                    session_name,
                    combined_error
                );
                // Return Ok even with partial failures - resources are cleaned up as much as possible
                // The errors are logged for debugging
                Ok(())
            }
        })
        .await
    }

    pub fn tmux_name(&self) -> String {
        let sanitized = sanitize_name(&self.name);
        Tmux::session_name(&sanitized, &self.id.short())
    }

    pub fn validate(&self) -> Result<()> {
        validate_session_name(&self.name)?;

        if self.branch.is_empty() {
            return Err(Error::Validation("Branch cannot be empty".to_string()));
        }

        if self.base_commit.is_empty() {
            return Err(Error::Validation("Base commit cannot be empty".to_string()));
        }

        Ok(())
    }

    pub fn touch(&mut self) {
        self.last_active = Utc::now();
    }

    pub fn is_tmux_alive(&self) -> bool {
        Tmux::session_exists(&self.tmux_name())
    }

    pub async fn is_tmux_alive_async(&self) -> bool {
        let tmux_name = self.tmux_name();
        blocking(move || Ok(Tmux::session_exists(&tmux_name)))
            .await
            .unwrap_or(false)
    }

    pub fn to_refresh_info(
        &self,
        repo_path: Option<&std::path::Path>,
        prompt_pattern: Option<&str>,
    ) -> SessionInfo {
        SessionInfo {
            id: self.id,
            tmux_name: self.tmux_name(),
            repo_path: repo_path.map(|p| p.to_path_buf()),
            worktree_path: self.worktree_path.clone(),
            prompt_pattern: prompt_pattern.map(|s| s.to_string()),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct State {
    pub version: u32,
    pub sessions: Vec<Session>,
}

impl Default for State {
    fn default() -> Self {
        Self::new()
    }
}

impl State {
    pub fn new() -> Self {
        Self {
            version: STATE_VERSION,
            sessions: Vec::new(),
        }
    }

    pub async fn load() -> Result<Self> {
        blocking(Self::load_sync).await
    }

    pub fn load_sync() -> Result<Self> {
        let state_path = Config::state_path()?;
        zlog_debug!("State::load_sync path={}", state_path.display());

        if !state_path.exists() {
            zlog_debug!("State file not found, returning empty state");
            return Ok(Self::new());
        }

        let contents = fs::read_to_string(&state_path)?;
        let state: State = serde_json::from_str(&contents)?;
        zlog_debug!("State loaded: {} sessions", state.sessions.len());
        Ok(state)
    }

    pub async fn save(&self) -> Result<()> {
        zlog_debug!("State::save sessions={}", self.sessions.len());
        let contents = serde_json::to_string_pretty(self)?;

        blocking(move || {
            let state_path = Config::state_path()?;
            let zen_dir = Config::zen_dir()?;

            if !zen_dir.exists() {
                zlog_debug!("Creating zen directory: {}", zen_dir.display());
                fs::create_dir_all(&zen_dir)?;
            }

            if state_path.exists() {
                let backup_path = state_path.with_extension("json.bak");
                zlog_debug!("Creating state backup: {}", backup_path.display());
                fs::copy(&state_path, &backup_path)?;
            }

            let temp_path = state_path.with_extension("json.tmp");
            fs::write(&temp_path, &contents)?;
            fs::rename(&temp_path, &state_path)?;
            zlog_debug!("State saved: {}", state_path.display());

            Ok(())
        })
        .await
    }

    pub fn save_sync(&self) -> Result<()> {
        let state_path = Config::state_path()?;
        let zen_dir = Config::zen_dir()?;

        if !zen_dir.exists() {
            fs::create_dir_all(&zen_dir)?;
        }

        if state_path.exists() {
            let backup_path = state_path.with_extension("json.bak");
            fs::copy(&state_path, &backup_path)?;
        }

        let temp_path = state_path.with_extension("json.tmp");
        let contents = serde_json::to_string_pretty(self)?;
        fs::write(&temp_path, &contents)?;
        fs::rename(&temp_path, &state_path)?;

        Ok(())
    }

    pub async fn reconcile(&mut self) -> Vec<String> {
        let mut warnings = Vec::new();

        let sessions_info: Vec<(usize, String, SessionStatus, Option<PathBuf>)> = self
            .sessions
            .iter()
            .enumerate()
            .map(|(i, s)| (i, s.tmux_name(), s.status, s.worktree_path.clone()))
            .collect();

        let known_tmux_names: Vec<String> = self.sessions.iter().map(|s| s.tmux_name()).collect();

        let reconcile_result = blocking(move || {
            let mut warnings = Vec::new();
            let mut status_updates: Vec<(usize, SessionStatus)> = Vec::new();

            for (idx, tmux_name, status, worktree_path) in &sessions_info {
                let tmux_exists = Tmux::session_exists(tmux_name);

                if *status == SessionStatus::Running && !tmux_exists {
                    warnings.push(format!(
                        "Session at index {} was marked as running but tmux session not found, marking as locked",
                        idx
                    ));
                    status_updates.push((*idx, SessionStatus::Locked));
                }

                // Check worktree existence
                if let Some(wt_path) = worktree_path {
                    if !wt_path.exists() {
                        warnings.push(format!(
                            "Session at index {} worktree not found at '{}'",
                            idx,
                            wt_path.display()
                        ));
                    }
                }
            }

            if let Ok(tmux_sessions) = Tmux::list_zen_sessions() {
                for tmux_name in tmux_sessions {
                    if !known_tmux_names.contains(&tmux_name) {
                        warnings.push(format!("Orphaned tmux session found: '{}'", tmux_name));
                    }
                }
            }

            Ok::<_, Error>((warnings, status_updates))
        })
        .await;

        if let Ok((blocking_warnings, status_updates)) = reconcile_result {
            // Apply status updates
            for (idx, new_status) in status_updates {
                if let Some(session) = self.sessions.get_mut(idx) {
                    let name = session.name.clone();
                    session.status = new_status;
                    warnings.push(format!(
                        "Session '{}' was marked as running but tmux session not found, marking as locked",
                        name
                    ));
                }
            }

            for warning in blocking_warnings {
                if warning.contains("Orphaned") || warning.contains("worktree not found") {
                    warnings.push(warning);
                }
            }
        }

        warnings
    }

    pub fn reconcile_sync(&mut self) -> Vec<String> {
        let mut warnings = Vec::new();

        for session in &mut self.sessions {
            let tmux_exists = session.is_tmux_alive();

            if session.status == SessionStatus::Running && !tmux_exists {
                warnings.push(format!(
                    "Session '{}' was marked as running but tmux session not found, marking as locked",
                    session.name
                ));
                session.status = SessionStatus::Locked;
            }

            if let Some(worktree_path) = &session.worktree_path {
                if !worktree_path.exists() {
                    warnings.push(format!(
                        "Session '{}' worktree not found at '{}'",
                        session.name,
                        worktree_path.display()
                    ));
                }
            }
        }

        if let Ok(tmux_sessions) = Tmux::list_zen_sessions() {
            let known_tmux_names: Vec<String> =
                self.sessions.iter().map(|s| s.tmux_name()).collect();

            for tmux_name in tmux_sessions {
                if !known_tmux_names.contains(&tmux_name) {
                    warnings.push(format!("Orphaned tmux session found: '{}'", tmux_name));
                }
            }
        }

        warnings
    }

    /// Clean up orphaned tmux sessions that are not tracked in state.
    /// Returns the names of sessions that were cleaned up.
    pub fn cleanup_orphaned_tmux_sessions(&self) -> Vec<String> {
        let mut cleaned = Vec::new();

        if let Ok(tmux_sessions) = Tmux::list_zen_sessions() {
            let known_tmux_names: Vec<String> =
                self.sessions.iter().map(|s| s.tmux_name()).collect();

            for tmux_name in tmux_sessions {
                if !known_tmux_names.contains(&tmux_name) {
                    crate::zlog!("Cleaning up orphaned tmux session: '{}'", tmux_name);
                    if Tmux::kill_session(&tmux_name).is_ok() {
                        cleaned.push(tmux_name);
                    }
                }
            }
        }

        cleaned
    }

    /// Clean up orphaned worktree directories that are not tracked in state.
    /// Returns the paths of directories that were cleaned up.
    pub fn cleanup_orphaned_worktrees(&self) -> Vec<PathBuf> {
        let mut cleaned = Vec::new();

        let worktrees_dir = match Config::worktrees_dir() {
            Ok(dir) => dir,
            Err(_) => return cleaned,
        };

        if !worktrees_dir.exists() {
            return cleaned;
        }

        // Get known worktree paths
        let known_worktree_paths: Vec<PathBuf> = self
            .sessions
            .iter()
            .filter_map(|s| s.worktree_path.clone())
            .collect();

        // List all directories in worktrees dir
        if let Ok(entries) = std::fs::read_dir(&worktrees_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() && !known_worktree_paths.contains(&path) {
                    crate::zlog!(
                        "Cleaning up orphaned worktree directory: '{}'",
                        path.display()
                    );
                    if std::fs::remove_dir_all(&path).is_ok() {
                        cleaned.push(path);
                    }
                }
            }
        }

        cleaned
    }

    /// Perform full cleanup of all orphaned resources.
    /// Returns a summary of what was cleaned up.
    pub fn cleanup_all_orphans(&self) -> CleanupSummary {
        let tmux_sessions = self.cleanup_orphaned_tmux_sessions();
        let worktrees = self.cleanup_orphaned_worktrees();

        CleanupSummary {
            tmux_sessions,
            worktrees,
        }
    }

    pub fn find(&self, id: SessionId) -> Option<&Session> {
        self.sessions.iter().find(|s| s.id == id)
    }

    pub fn find_mut(&mut self, id: SessionId) -> Option<&mut Session> {
        self.sessions.iter_mut().find(|s| s.id == id)
    }

    pub fn find_by_name(&self, name: &str) -> Option<&Session> {
        self.sessions.iter().find(|s| s.name == name)
    }

    pub fn find_by_name_mut(&mut self, name: &str) -> Option<&mut Session> {
        self.sessions.iter_mut().find(|s| s.name == name)
    }

    pub fn add(&mut self, session: Session) {
        self.sessions.push(session);
    }

    pub fn remove(&mut self, id: SessionId) -> Option<Session> {
        self.sessions
            .iter()
            .position(|s| s.id == id)
            .map(|pos| self.sessions.remove(pos))
    }

    pub fn len(&self) -> usize {
        self.sessions.len()
    }

    pub fn is_empty(&self) -> bool {
        self.sessions.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = &Session> {
        self.sessions.iter()
    }

    /// Perform a complete reset of all zen resources.
    /// This deletes all sessions, kills all tmux sessions, removes all worktrees,
    /// and deletes all zen-related branches.
    ///
    /// If force is false, sessions with uncommitted work will be skipped.
    /// If force is true, all sessions are deleted regardless of dirty state.
    ///
    /// Returns a tuple of (sessions_deleted, tmux_count, worktrees_count, branches_count, skipped_session_names).
    pub fn reset_all(force: bool) -> Result<(usize, usize, usize, usize, Vec<String>)> {
        use crate::git::GitOps;
        use crate::tmux::Tmux;

        // Load current state to get session info
        let state = State::load_sync()?;

        // Check for dirty worktrees if not forcing
        let mut skipped_sessions = Vec::new();
        let mut sessions_to_delete = Vec::new();
        let mut sessions_to_skip = Vec::new();

        if !force {
            for session in &state.sessions {
                if let Some(worktree_path) = &session.worktree_path {
                    if GitOps::is_worktree_dirty(worktree_path) {
                        crate::zlog!(
                            "Skipping dirty worktree: {} ({})",
                            session.name,
                            worktree_path.display()
                        );
                        skipped_sessions.push(session.name.clone());
                        sessions_to_skip.push(session.clone());
                        continue;
                    }
                }
                sessions_to_delete.push(session.clone());
            }
        } else {
            sessions_to_delete = state.sessions.clone();
        }

        let session_count = sessions_to_delete.len();

        // 1. Kill tmux sessions for sessions being deleted
        let mut tmux_count = 0;
        for session in &sessions_to_delete {
            let tmux_name = session.tmux_name();
            if Tmux::kill_session(&tmux_name).is_ok() {
                tmux_count += 1;
            }
        }

        // 2. Clean up worktree directories for sessions being deleted
        let mut worktree_count = 0;
        for session in &sessions_to_delete {
            if let Some(worktree_path) = &session.worktree_path {
                if worktree_path.exists() && std::fs::remove_dir_all(worktree_path).is_ok() {
                    worktree_count += 1;
                }
            }
        }

        // 3. Clean up zen-related branches for deleted sessions and prune worktrees
        let repo_path = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let mut branches_count = 0;
        if let Ok(git) = GitOps::new(&repo_path) {
            // Prune worktree administrative files first
            let _ = git.prune_worktrees();

            // Delete branches for sessions being deleted
            for session in &sessions_to_delete {
                if git.delete_branch(&session.branch).is_ok() {
                    branches_count += 1;
                }
            }
        }

        // 4. Update state file - keep skipped sessions
        let mut new_state = State::new();
        for session in sessions_to_skip {
            new_state.add(session);
        }
        new_state.save_sync()?;

        Ok((
            session_count,
            tmux_count,
            worktree_count,
            branches_count,
            skipped_sessions,
        ))
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut Session> {
        self.sessions.iter_mut()
    }

    pub fn running(&self) -> Vec<&Session> {
        self.sessions
            .iter()
            .filter(|s| s.status == SessionStatus::Running)
            .collect()
    }

    pub fn locked(&self) -> Vec<&Session> {
        self.sessions
            .iter()
            .filter(|s| s.status == SessionStatus::Locked)
            .collect()
    }
}

fn validate_session_name(name: &str) -> Result<()> {
    if name.is_empty() {
        return Err(Error::Validation(
            "Session name cannot be empty".to_string(),
        ));
    }

    if name.len() > MAX_SESSION_NAME_LENGTH {
        return Err(Error::Validation(format!(
            "Session name too long (max {} characters)",
            MAX_SESSION_NAME_LENGTH
        )));
    }

    if name.chars().any(|c| c.is_control()) {
        return Err(Error::Validation(
            "Session name cannot contain control characters".to_string(),
        ));
    }

    Ok(())
}

/// Sanitize a session name for use in branch names (spaces become hyphens)
fn sanitize_name(name: &str) -> String {
    name.trim()
        .to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

#[cfg(test)]
mod tests {
    use super::*;

    // SessionId tests
    #[test]
    fn test_session_id_new() {
        let id1 = SessionId::new();
        let id2 = SessionId::new();
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_session_id_default() {
        let id = SessionId::default();
        assert!(!id.0.is_nil());
    }

    #[test]
    fn test_session_id_short() {
        let id = SessionId::new();
        let short = id.short();
        assert_eq!(short.len(), 8);
    }

    #[test]
    fn test_session_id_display() {
        let id = SessionId::new();
        let display = format!("{}", id);
        assert_eq!(display, id.0.to_string());
    }

    #[test]
    fn test_session_id_from_str() {
        let id = SessionId::new();
        let s = id.to_string();
        let parsed: SessionId = s.parse().unwrap();
        assert_eq!(id, parsed);
    }

    #[test]
    fn test_session_id_from_str_invalid() {
        let result: std::result::Result<SessionId, _> = "invalid".parse();
        assert!(result.is_err());
    }

    #[test]
    fn test_session_id_serialization() {
        let id = SessionId::new();
        let json = serde_json::to_string(&id).unwrap();
        let parsed: SessionId = serde_json::from_str(&json).unwrap();
        assert_eq!(id, parsed);
    }

    // SessionStatus tests
    #[test]
    fn test_session_status_default() {
        let status = SessionStatus::default();
        assert_eq!(status, SessionStatus::Running);
    }

    #[test]
    fn test_session_status_display() {
        assert_eq!(format!("{}", SessionStatus::Running), "running");
        assert_eq!(format!("{}", SessionStatus::Locked), "locked");
    }

    #[test]
    fn test_session_status_serialization() {
        let running = SessionStatus::Running;
        let json = serde_json::to_string(&running).unwrap();
        assert_eq!(json, r#""running""#);

        let locked = SessionStatus::Locked;
        let json = serde_json::to_string(&locked).unwrap();
        assert_eq!(json, r#""locked""#);
    }

    #[test]
    fn test_session_status_deserialization() {
        let running: SessionStatus = serde_json::from_str(r#""running""#).unwrap();
        assert_eq!(running, SessionStatus::Running);

        let locked: SessionStatus = serde_json::from_str(r#""locked""#).unwrap();
        assert_eq!(locked, SessionStatus::Locked);
    }

    // Session tests
    #[test]
    fn test_session_tmux_name() {
        let session = Session {
            id: SessionId(Uuid::parse_str("12345678-1234-1234-1234-123456789012").unwrap()),
            name: "my session".to_string(),
            branch: "zen/my_session".to_string(),
            status: SessionStatus::Running,
            worktree_path: Some(PathBuf::from("/tmp/worktree")),
            base_commit: "abc123".to_string(),
            base_branch: "main".to_string(),
            created_at: Utc::now(),
            last_active: Utc::now(),
            agent: "claude".to_string(),
            project: "test-repo".to_string(),
        };

        let tmux_name = session.tmux_name();
        assert_eq!(tmux_name, "zen_my-session_12345678");
    }

    #[test]
    fn test_session_validate_valid() {
        let session = Session {
            id: SessionId::new(),
            name: "valid-session".to_string(),
            branch: "zen/valid-session".to_string(),
            status: SessionStatus::Running,
            worktree_path: Some(PathBuf::from("/tmp/worktree")),
            base_commit: "abc123def456".to_string(),
            base_branch: "main".to_string(),
            created_at: Utc::now(),
            last_active: Utc::now(),
            agent: "claude".to_string(),
            project: "test-repo".to_string(),
        };

        assert!(session.validate().is_ok());
    }

    #[test]
    fn test_session_validate_empty_name() {
        let session = Session {
            id: SessionId::new(),
            name: "".to_string(),
            branch: "zen/test".to_string(),
            status: SessionStatus::Running,
            worktree_path: None,
            base_commit: "abc123".to_string(),
            base_branch: "main".to_string(),
            created_at: Utc::now(),
            last_active: Utc::now(),
            agent: "claude".to_string(),
            project: "test-repo".to_string(),
        };

        let result = session.validate();
        assert!(result.is_err());
        assert!(matches!(result, Err(Error::Validation(_))));
    }

    #[test]
    fn test_session_validate_empty_branch() {
        let session = Session {
            id: SessionId::new(),
            name: "test".to_string(),
            branch: "".to_string(),
            status: SessionStatus::Running,
            worktree_path: None,
            base_commit: "abc123".to_string(),
            base_branch: "main".to_string(),
            created_at: Utc::now(),
            last_active: Utc::now(),
            agent: "claude".to_string(),
            project: "test-repo".to_string(),
        };

        let result = session.validate();
        assert!(result.is_err());
    }

    #[test]
    fn test_session_validate_empty_base_commit() {
        let session = Session {
            id: SessionId::new(),
            name: "test".to_string(),
            branch: "zen/test".to_string(),
            status: SessionStatus::Running,
            worktree_path: None,
            base_commit: "".to_string(),
            base_branch: "main".to_string(),
            created_at: Utc::now(),
            last_active: Utc::now(),
            agent: "claude".to_string(),
            project: "test-repo".to_string(),
        };

        let result = session.validate();
        assert!(result.is_err());
    }

    #[test]
    fn test_session_serialization() {
        let session = Session {
            id: SessionId::new(),
            name: "test-session".to_string(),
            branch: "zen/test-session".to_string(),
            status: SessionStatus::Running,
            worktree_path: Some(PathBuf::from("/tmp/worktree")),
            base_commit: "abc123def456".to_string(),
            base_branch: "main".to_string(),
            created_at: Utc::now(),
            last_active: Utc::now(),
            agent: "claude".to_string(),
            project: "test-repo".to_string(),
        };

        let json = serde_json::to_string(&session).unwrap();
        let parsed: Session = serde_json::from_str(&json).unwrap();

        assert_eq!(session.id, parsed.id);
        assert_eq!(session.name, parsed.name);
        assert_eq!(session.branch, parsed.branch);
        assert_eq!(session.status, parsed.status);
        assert_eq!(session.worktree_path, parsed.worktree_path);
        assert_eq!(session.base_commit, parsed.base_commit);
        assert_eq!(session.base_branch, parsed.base_branch);
        assert_eq!(session.agent, parsed.agent);
    }

    #[test]
    fn test_session_touch() {
        let mut session = Session {
            id: SessionId::new(),
            name: "test".to_string(),
            branch: "zen/test".to_string(),
            status: SessionStatus::Running,
            worktree_path: None,
            base_commit: "abc123".to_string(),
            base_branch: "main".to_string(),
            created_at: Utc::now(),
            last_active: Utc::now() - chrono::Duration::hours(1),
            agent: "claude".to_string(),
            project: "test-repo".to_string(),
        };

        let before = session.last_active;
        session.touch();
        assert!(session.last_active > before);
    }

    // State tests
    #[test]
    fn test_state_new() {
        let state = State::new();
        assert_eq!(state.version, STATE_VERSION);
        assert!(state.sessions.is_empty());
    }

    #[test]
    fn test_state_default() {
        let state = State::default();
        assert!(state.sessions.is_empty());
    }

    #[test]
    fn test_state_add_and_find() {
        let mut state = State::new();
        let session = Session {
            id: SessionId::new(),
            name: "test".to_string(),
            branch: "zen/test".to_string(),
            status: SessionStatus::Running,
            worktree_path: None,
            base_commit: "abc123".to_string(),
            base_branch: "main".to_string(),
            created_at: Utc::now(),
            last_active: Utc::now(),
            agent: "claude".to_string(),
            project: "test-repo".to_string(),
        };

        let id = session.id;
        state.add(session);

        assert_eq!(state.len(), 1);
        assert!(!state.is_empty());
        assert!(state.find(id).is_some());
    }

    #[test]
    fn test_state_find_mut() {
        let mut state = State::new();
        let session = Session {
            id: SessionId::new(),
            name: "test".to_string(),
            branch: "zen/test".to_string(),
            status: SessionStatus::Running,
            worktree_path: None,
            base_commit: "abc123".to_string(),
            base_branch: "main".to_string(),
            created_at: Utc::now(),
            last_active: Utc::now(),
            agent: "claude".to_string(),
            project: "test-repo".to_string(),
        };

        let id = session.id;
        state.add(session);

        if let Some(s) = state.find_mut(id) {
            s.status = SessionStatus::Locked;
        }

        assert_eq!(state.find(id).unwrap().status, SessionStatus::Locked);
    }

    #[test]
    fn test_state_find_by_name() {
        let mut state = State::new();
        let session = Session {
            id: SessionId::new(),
            name: "my-session".to_string(),
            branch: "zen/my-session".to_string(),
            status: SessionStatus::Running,
            worktree_path: None,
            base_commit: "abc123".to_string(),
            base_branch: "main".to_string(),
            created_at: Utc::now(),
            last_active: Utc::now(),
            agent: "claude".to_string(),
            project: "test-repo".to_string(),
        };

        state.add(session);

        assert!(state.find_by_name("my-session").is_some());
        assert!(state.find_by_name("nonexistent").is_none());
    }

    #[test]
    fn test_state_remove() {
        let mut state = State::new();
        let session = Session {
            id: SessionId::new(),
            name: "test".to_string(),
            branch: "zen/test".to_string(),
            status: SessionStatus::Running,
            worktree_path: None,
            base_commit: "abc123".to_string(),
            base_branch: "main".to_string(),
            created_at: Utc::now(),
            last_active: Utc::now(),
            agent: "claude".to_string(),
            project: "test-repo".to_string(),
        };

        let id = session.id;
        state.add(session);

        let removed = state.remove(id);
        assert!(removed.is_some());
        assert_eq!(removed.unwrap().name, "test");
        assert!(state.is_empty());
    }

    #[test]
    fn test_state_remove_nonexistent() {
        let mut state = State::new();
        let id = SessionId::new();
        let removed = state.remove(id);
        assert!(removed.is_none());
    }

    #[test]
    fn test_state_running_and_locked() {
        let mut state = State::new();

        let running_session = Session {
            id: SessionId::new(),
            name: "running".to_string(),
            branch: "zen/running".to_string(),
            status: SessionStatus::Running,
            worktree_path: None,
            base_commit: "abc123".to_string(),
            base_branch: "main".to_string(),
            created_at: Utc::now(),
            last_active: Utc::now(),
            agent: "claude".to_string(),
            project: "test-repo".to_string(),
        };

        let locked_session = Session {
            id: SessionId::new(),
            name: "locked".to_string(),
            branch: "zen/locked".to_string(),
            status: SessionStatus::Locked,
            worktree_path: None,
            base_commit: "abc123".to_string(),
            base_branch: "main".to_string(),
            created_at: Utc::now(),
            last_active: Utc::now(),
            agent: "claude".to_string(),
            project: "test-repo".to_string(),
        };

        state.add(running_session);
        state.add(locked_session);

        let running = state.running();
        let locked = state.locked();

        assert_eq!(running.len(), 1);
        assert_eq!(running[0].name, "running");

        assert_eq!(locked.len(), 1);
        assert_eq!(locked[0].name, "locked");
    }

    #[test]
    fn test_state_serialization() {
        let mut state = State::new();
        let session = Session {
            id: SessionId::new(),
            name: "test".to_string(),
            branch: "zen/test".to_string(),
            status: SessionStatus::Running,
            worktree_path: Some(PathBuf::from("/tmp/test")),
            base_commit: "abc123".to_string(),
            base_branch: "main".to_string(),
            created_at: Utc::now(),
            last_active: Utc::now(),
            agent: "claude".to_string(),
            project: "test-repo".to_string(),
        };

        state.add(session);

        let json = serde_json::to_string_pretty(&state).unwrap();
        let parsed: State = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.version, state.version);
        assert_eq!(parsed.sessions.len(), 1);
        assert_eq!(parsed.sessions[0].name, "test");
    }

    #[test]
    fn test_state_iter() {
        let mut state = State::new();

        for i in 0..3 {
            state.add(Session {
                id: SessionId::new(),
                name: format!("session-{}", i),
                branch: format!("zen/session-{}", i),
                status: SessionStatus::Running,
                worktree_path: None,
                base_commit: "abc123".to_string(),
                base_branch: "main".to_string(),
                created_at: Utc::now(),
                last_active: Utc::now(),
                agent: "claude".to_string(),
                project: "test-repo".to_string(),
            });
        }

        let names: Vec<_> = state.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(names, vec!["session-0", "session-1", "session-2"]);
    }

    // Validation tests
    #[test]
    fn test_validate_session_name_valid() {
        assert!(validate_session_name("valid-name").is_ok());
        assert!(validate_session_name("valid_name").is_ok());
        assert!(validate_session_name("valid name 123").is_ok());
        assert!(validate_session_name("a").is_ok());
    }

    #[test]
    fn test_validate_session_name_empty() {
        let result = validate_session_name("");
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_session_name_too_long() {
        let long_name = "a".repeat(MAX_SESSION_NAME_LENGTH + 1);
        let result = validate_session_name(&long_name);
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_session_name_max_length() {
        let max_name = "a".repeat(MAX_SESSION_NAME_LENGTH);
        assert!(validate_session_name(&max_name).is_ok());
    }

    #[test]
    fn test_validate_session_name_control_chars() {
        let result = validate_session_name("name\nwith\nnewlines");
        assert!(result.is_err());

        let result = validate_session_name("name\twith\ttabs");
        assert!(result.is_err());
    }

    // Sanitization tests
    #[test]
    fn test_sanitize_name() {
        assert_eq!(sanitize_name("hello"), "hello");
        assert_eq!(sanitize_name("hello world"), "hello-world");
        assert_eq!(sanitize_name("Hello World"), "hello-world");
        assert_eq!(sanitize_name("hello-world"), "hello-world");
        assert_eq!(sanitize_name("hello_world"), "hello-world");
        assert_eq!(sanitize_name("hello.world!"), "hello-world");
        assert_eq!(sanitize_name("test@123#456"), "test-123-456");
        assert_eq!(sanitize_name("Fix Auth"), "fix-auth");
        assert_eq!(sanitize_name("  spaces  around  "), "spaces-around");
    }

    #[test]
    fn test_sanitize_name_empty() {
        assert_eq!(sanitize_name(""), "");
    }

    #[test]
    fn test_sanitize_name_all_special() {
        assert_eq!(sanitize_name("@#$%^&*"), "");
    }

    #[test]
    fn test_sanitize_name_unicode() {
        // Unicode letters are alphanumeric, so they should be kept
        assert_eq!(sanitize_name("cafe"), "cafe");
    }

    // to_refresh_info tests
    #[test]
    fn test_to_refresh_info_with_repo_path() {
        let session = Session {
            id: SessionId(Uuid::parse_str("12345678-1234-1234-1234-123456789012").unwrap()),
            name: "test session".to_string(),
            branch: "zen/test_session".to_string(),
            status: SessionStatus::Running,
            worktree_path: Some(PathBuf::from("/tmp/worktree")),
            base_commit: "abc123".to_string(),
            base_branch: "main".to_string(),
            created_at: Utc::now(),
            last_active: Utc::now(),
            agent: "claude".to_string(),
            project: "test-repo".to_string(),
        };

        let repo_path = PathBuf::from("/path/to/repo");
        let info = session.to_refresh_info(Some(&repo_path), Some("Do you want"));

        assert_eq!(info.id, session.id);
        assert_eq!(info.tmux_name, "zen_test-session_12345678");
        assert_eq!(info.repo_path, Some(repo_path));
        assert_eq!(info.worktree_path, Some(PathBuf::from("/tmp/worktree")));
        assert_eq!(info.prompt_pattern, Some("Do you want".to_string()));
    }

    #[test]
    fn test_to_refresh_info_without_repo_path() {
        let session = Session {
            id: SessionId::new(),
            name: "test".to_string(),
            branch: "zen/test".to_string(),
            status: SessionStatus::Running,
            worktree_path: None,
            base_commit: "abc123".to_string(),
            base_branch: "main".to_string(),
            created_at: Utc::now(),
            last_active: Utc::now(),
            agent: "claude".to_string(),
            project: "test-repo".to_string(),
        };

        let info = session.to_refresh_info(None, None);

        assert_eq!(info.id, session.id);
        assert!(info.repo_path.is_none());
        assert!(info.worktree_path.is_none());
        assert!(info.prompt_pattern.is_none());
    }
}
