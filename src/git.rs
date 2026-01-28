use std::path::{Path, PathBuf};

use git2::{ErrorCode, IndexAddOption, Repository, ResetType, Signature};

use crate::{zlog_debug, zlog_warn, Result};

pub struct GitOps {
    repo_path: PathBuf,
}

impl GitOps {
    pub fn new(repo_path: &Path) -> Result<Self> {
        zlog_debug!("GitOps::new path={}", repo_path.display());
        let _ = Repository::discover(repo_path)?;
        Ok(Self {
            repo_path: repo_path.to_path_buf(),
        })
    }

    fn repo(&self) -> Result<Repository> {
        Ok(Repository::discover(&self.repo_path)?)
    }

    pub fn repo_path(&self) -> &Path {
        &self.repo_path
    }

    pub fn create_worktree(&self, branch: &str, worktree_path: &Path) -> Result<()> {
        zlog_debug!(
            "GitOps::create_worktree branch={} path={}",
            branch,
            worktree_path.display()
        );
        let repo = self.repo()?;
        let head = repo.head()?;
        let commit = head.peel_to_commit()?;
        zlog_debug!("Creating branch {} from commit {}", branch, commit.id());
        let branch_obj = repo.branch(branch, &commit, false)?;
        let branch_ref = branch_obj.into_reference();
        let mut opts = git2::WorktreeAddOptions::new();
        opts.reference(Some(&branch_ref));
        // Use worktree path's folder name as worktree name (branch may contain slashes)
        let worktree_name = worktree_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(branch);
        zlog_debug!("Creating worktree with name: {}", worktree_name);
        repo.worktree(worktree_name, worktree_path, Some(&opts))?;
        zlog_debug!("Worktree created successfully");
        Ok(())
    }

    /// Remove a worktree and clean up all associated resources.
    /// This function attempts cleanup even if some operations fail.
    /// It's critical that we fully disassociate the branch from the worktree,
    /// otherwise unlocking will fail with "branch is already checked out".
    pub fn remove_worktree(&self, worktree_path: &Path) -> Result<()> {
        zlog_debug!("GitOps::remove_worktree path={}", worktree_path.display());
        let repo = self.repo()?;
        let worktrees = repo.worktrees()?;

        // Try to find the worktree by path (may fail due to path canonicalization)
        let worktree_name: Option<String> = worktrees
            .iter()
            .flatten()
            .find(|name| {
                repo.find_worktree(name)
                    .map(|wt| wt.path() == worktree_path)
                    .unwrap_or(false)
            })
            .map(|s| s.to_string());

        // Also try to find by folder name as fallback
        let folder_name = worktree_path
            .file_name()
            .and_then(|n| n.to_str())
            .map(|s| s.to_string());

        let worktree_name = worktree_name.or_else(|| {
            folder_name.as_ref().and_then(|fname| {
                worktrees
                    .iter()
                    .flatten()
                    .find(|name| *name == fname.as_str())
                    .map(|s| s.to_string())
            })
        });

        zlog_debug!(
            "remove_worktree: worktree_name={:?} folder_name={:?}",
            worktree_name,
            folder_name
        );

        // Try to prune via git if worktree is known
        if let Some(ref name) = worktree_name {
            if let Ok(worktree) = repo.find_worktree(name) {
                zlog_debug!("Unlocking and pruning worktree: {}", name);
                // Try to unlock first if locked
                let _ = worktree.unlock();
                // Prune with valid=true to remove even valid worktrees
                let prune_result = worktree.prune(Some(
                    git2::WorktreePruneOptions::new()
                        .valid(true)
                        .working_tree(true)
                        .locked(true),
                ));
                if let Err(e) = prune_result {
                    zlog_warn!("Worktree prune failed for '{}': {}", name, e);
                }
            }
        }

        // Always try to remove the worktree directory if it exists
        if worktree_path.exists() {
            zlog_debug!("Removing worktree directory: {}", worktree_path.display());
            std::fs::remove_dir_all(worktree_path)?;
        }

        // Clean up the git worktree admin directory (e.g., .git/worktrees/<name>)
        // This is CRITICAL - if the admin dir still exists, git thinks the branch is checked out
        if let Some(ref name) = worktree_name {
            self.cleanup_worktree_admin_dir(name);
        }
        // Also try by folder name as backup
        if let Some(ref fname) = folder_name {
            self.cleanup_worktree_admin_dir(fname);
        }

        // Force git to re-scan worktrees by running prune on the entire worktree list
        // This helps clean up any stale references
        drop(repo); // Release the repo handle
        if let Ok(repo) = self.repo() {
            if let Ok(worktrees) = repo.worktrees() {
                for name in worktrees.iter().flatten() {
                    if let Ok(wt) = repo.find_worktree(name) {
                        // Check if this worktree's path no longer exists - if so, prune it
                        if !wt.path().exists() {
                            zlog_debug!("Pruning stale worktree reference: {}", name);
                            let _ = wt.prune(Some(
                                git2::WorktreePruneOptions::new()
                                    .valid(true)
                                    .working_tree(true)
                                    .locked(true),
                            ));
                        }
                    }
                }
            }
        }

        zlog_debug!("Worktree removed successfully");
        Ok(())
    }

    /// Clean up the git worktree admin directory (.git/worktrees/<name>)
    fn cleanup_worktree_admin_dir(&self, worktree_name: &str) {
        if let Ok(repo) = self.repo() {
            let git_dir = repo.path(); // This is the .git directory
            let admin_dir = git_dir.join("worktrees").join(worktree_name);
            if admin_dir.exists() {
                zlog_debug!("Cleaning up worktree admin dir: {}", admin_dir.display());
                let _ = std::fs::remove_dir_all(&admin_dir);
            }
        }
    }

    pub fn commit_all(&self, worktree_path: &Path, message: &str) -> Result<()> {
        zlog_debug!(
            "GitOps::commit_all path={} message={}",
            worktree_path.display(),
            message
        );
        let repo = Repository::open(worktree_path)?;
        let mut index = repo.index()?;
        index.add_all(["."].iter(), IndexAddOption::DEFAULT, None)?;
        index.write()?;

        let tree_id = index.write_tree()?;
        let tree = repo.find_tree(tree_id)?;
        let sig = repo
            .signature()
            .or_else(|_| Signature::now("Zen", "zen@localhost"))?;

        let parent = match repo.head() {
            Ok(head) => Some(head.peel_to_commit()?),
            Err(e) if e.code() == ErrorCode::UnbornBranch => None,
            Err(e) => return Err(e.into()),
        };

        let parents: Vec<&git2::Commit> = parent.iter().collect();
        let commit_id = repo.commit(Some("HEAD"), &sig, &sig, message, &tree, &parents)?;
        zlog_debug!("Commit created: {}", commit_id);
        Ok(())
    }

    pub fn current_head(&self) -> Result<String> {
        let repo = self.repo()?;
        let head = repo.head()?;
        if head.is_branch() {
            if let Some(name) = head.shorthand() {
                return Ok(name.to_string());
            }
        }
        let commit = head.peel_to_commit()?;
        Ok(format!("{:.7}", commit.id()))
    }

    pub fn head_commit(&self) -> Result<String> {
        let repo = self.repo()?;
        let head = repo.head()?;
        let commit = head.peel_to_commit()?;
        Ok(commit.id().to_string())
    }

    pub fn reset_hard(&self, worktree_path: &Path) -> Result<()> {
        let repo = Repository::open(worktree_path)?;
        let head = repo.head()?.peel_to_commit()?;
        repo.reset(head.as_object(), ResetType::Hard, None)?;
        Ok(())
    }

    pub fn list_worktrees(&self) -> Result<Vec<String>> {
        let repo = self.repo()?;
        Ok(repo
            .worktrees()?
            .iter()
            .flatten()
            .map(String::from)
            .collect())
    }

    pub fn branch_exists(&self, branch: &str) -> Result<bool> {
        let repo = self.repo()?;
        let result = repo.find_branch(branch, git2::BranchType::Local);
        match result {
            Ok(_) => Ok(true),
            Err(e) if e.code() == ErrorCode::NotFound => Ok(false),
            Err(e) => Err(e.into()),
        }
    }

    /// Check if a worktree has uncommitted changes (staged or unstaged).
    pub fn is_dirty(&self, worktree_path: &Path) -> Result<bool> {
        let repo = Repository::open(worktree_path)?;
        let statuses = repo.statuses(None)?;
        Ok(!statuses.is_empty())
    }

    /// Create a worktree from an existing branch (for unlocking locked sessions).
    pub fn create_worktree_from_branch(&self, branch: &str, worktree_path: &Path) -> Result<()> {
        let repo = self.repo()?;
        let branch_ref = repo.find_branch(branch, git2::BranchType::Local)?;
        let reference = branch_ref.into_reference();

        let mut opts = git2::WorktreeAddOptions::new();
        opts.reference(Some(&reference));

        let worktree_name = worktree_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(branch);

        repo.worktree(worktree_name, worktree_path, Some(&opts))?;
        Ok(())
    }

    /// Delete a local branch. Returns Ok even if branch doesn't exist.
    /// Logs a warning if deletion fails for other reasons but doesn't error.
    pub fn delete_branch(&self, branch: &str) -> Result<()> {
        zlog_debug!("GitOps::delete_branch branch={}", branch);
        let repo = self.repo()?;
        match repo.find_branch(branch, git2::BranchType::Local) {
            Ok(mut branch_ref) => {
                if let Err(e) = branch_ref.delete() {
                    // Log warning but don't fail - the branch might be checked out elsewhere
                    // or have other issues. The important thing is the worktree is gone.
                    zlog_warn!("Failed to delete branch '{}': {}", branch, e);
                } else {
                    zlog_debug!("Branch deleted: {}", branch);
                }
            }
            Err(e) if e.code() == ErrorCode::NotFound => {
                // Branch doesn't exist - that's fine
                zlog_debug!("Branch '{}' not found (already deleted?)", branch);
            }
            Err(e) => {
                // Log but don't fail for other errors
                zlog_warn!("Error looking up branch '{}': {}", branch, e);
            }
        }
        Ok(())
    }

    /// Force delete a branch reference directly. Use when normal deletion fails.
    pub fn force_delete_branch_ref(&self, branch: &str) -> Result<()> {
        zlog_debug!("GitOps::force_delete_branch_ref branch={}", branch);
        let repo = self.repo()?;
        let refname = format!("refs/heads/{}", branch);
        if let Ok(mut reference) = repo.find_reference(&refname) {
            let _ = reference.delete();
            zlog_debug!("Force deleted branch ref: {}", refname);
        }
        Ok(())
    }

    /// Get the git user name from config, falling back to system username or "user"
    pub fn git_user(&self) -> Result<String> {
        let repo = self.repo()?;
        let config = repo.config()?;

        // Try git config user.name first
        if let Ok(name) = config.get_string("user.name") {
            // Sanitize: lowercase, replace spaces with hyphens
            let sanitized = name
                .trim()
                .to_lowercase()
                .chars()
                .map(|c| if c.is_alphanumeric() { c } else { '-' })
                .collect::<String>();
            if !sanitized.is_empty() && sanitized != "-" {
                return Ok(sanitized);
            }
        }

        // Fall back to system username
        if let Ok(user) = std::env::var("USER").or_else(|_| std::env::var("USERNAME")) {
            return Ok(user.to_lowercase());
        }

        Ok("user".to_string())
    }

    /// Get the repository name from the repo path
    pub fn repo_name(&self) -> String {
        self.repo_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string()
    }

    /// Clean up all zen-related branches (branches matching pattern user/* or zen/*)
    /// Returns the number of branches deleted.
    pub fn cleanup_zen_branches(&self) -> usize {
        zlog_debug!("GitOps::cleanup_zen_branches");
        let Ok(repo) = self.repo() else {
            zlog_warn!("Could not open repository for branch cleanup");
            return 0;
        };

        let Ok(branches) = repo.branches(Some(git2::BranchType::Local)) else {
            zlog_warn!("Could not list branches for cleanup");
            return 0;
        };

        let mut deleted = 0;
        for branch_result in branches {
            let Ok((mut branch, _branch_type)) = branch_result else {
                continue;
            };

            // Get the name first before trying to delete
            let name = branch.name().ok().flatten().map(String::from);

            let Some(branch_name) = name else {
                continue;
            };

            // Delete branches that match zen's naming pattern (user/* or zen/*)
            // Skip remote branches like origin/*
            if branch_name.contains('/')
                && (branch_name.split('/').next() != Some("origin"))
                && branch.delete().is_ok()
            {
                zlog_debug!("Deleted branch during cleanup: {}", branch_name);
                deleted += 1;
            }
        }

        zlog_debug!("cleanup_zen_branches: deleted {} branches", deleted);
        deleted
    }

    /// Prune all git worktree administrative files.
    /// This is important after removing worktree directories directly.
    pub fn prune_worktrees(&self) -> Result<()> {
        zlog_debug!("GitOps::prune_worktrees");
        let repo = self.repo()?;
        let worktrees = repo.worktrees()?;

        let mut pruned = 0;
        for worktree_name in worktrees.iter().flatten() {
            if let Ok(worktree) = repo.find_worktree(worktree_name) {
                if worktree
                    .prune(Some(
                        git2::WorktreePruneOptions::new()
                            .valid(true)
                            .working_tree(true)
                            .locked(true),
                    ))
                    .is_ok()
                {
                    pruned += 1;
                }
            }
        }

        zlog_debug!("prune_worktrees: pruned {} worktrees", pruned);
        Ok(())
    }

    /// Check if a worktree directory has uncommitted changes using git status --porcelain.
    /// Returns true if there are uncommitted changes (dirty), false if clean.
    /// Returns false if the worktree path doesn't exist or isn't a git repository.
    pub fn is_worktree_dirty(worktree_path: &Path) -> bool {
        if !worktree_path.exists() {
            return false;
        }

        // Use git status --porcelain to check for uncommitted changes
        let output = std::process::Command::new("git")
            .arg("status")
            .arg("--porcelain")
            .current_dir(worktree_path)
            .output();

        match output {
            Ok(output) => {
                // If there's any output, the worktree is dirty
                !output.stdout.is_empty()
            }
            Err(_) => {
                // If the command fails, assume it's not dirty (or not a git repo)
                false
            }
        }
    }
}
