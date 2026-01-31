# Task: Implement Orphan Detection and Background Cleanup

## Description
Implement orphan resource detection and a background cleanup actor that periodically removes stale resources.

## Background
Orphaned resources (worktrees, tmux sessions, branches) can accumulate from interrupted workflows or bugs. Detection and optional cleanup keeps the system healthy.

## Reference Documentation
**Required:**
- Design: .sop/planning/design/detailed-design.md (Section 4.7 Git State Manager)
- Research: .sop/planning/research/existing-code.md (cleanup_orphaned methods)

**Note:** You MUST read both documents to understand existing patterns.

## Technical Requirements
1. Add to CleanupManager:
   - `detect_orphaned_worktrees(&self) -> Vec<PathBuf>`
   - `detect_orphaned_tmux(&self) -> Vec<String>`
   - `detect_orphaned_branches(&self) -> Vec<String>`
2. Create background cleanup actor:
   - Runs every 5 minutes
   - Reports orphans (doesn't auto-delete by default)
   - Configurable auto-cleanup option
3. Add `zen cleanup` command for manual cleanup

## Dependencies
- CleanupManager from task-01
- Existing Session reconciliation patterns
- Actor system patterns

## Implementation Approach
1. Implement worktree orphan detection (no matching workflow)
2. Implement tmux session orphan detection (no matching agent)
3. Implement branch orphan detection (zen/* without workflow)
4. Create CleanupActor following existing actor patterns
5. Add zen cleanup CLI command
6. Add interactive confirmation for cleanup
7. Add tests for detection logic

## Acceptance Criteria

1. **Worktree Orphan Detection**
   - Given worktree without linked workflow
   - When detect_orphaned_worktrees() runs
   - Then orphan path is returned

2. **Tmux Orphan Detection**
   - Given tmux session "zen_old_task_abc"
   - When detect_orphaned_tmux() runs
   - Then session name is returned

3. **Background Actor**
   - Given cleanup actor running
   - When 5 minutes pass
   - Then orphan detection runs and reports

4. **Manual Cleanup Command**
   - Given `zen cleanup`
   - When executed
   - Then orphans are listed and user can confirm deletion

5. **Safe Default**
   - Given orphans detected
   - When auto-cleanup is disabled (default)
   - Then orphans are reported but not deleted

## Metadata
- **Complexity**: Medium
- **Labels**: Cleanup, Orphan, Actor, Background
- **Required Skills**: Rust, actor pattern, resource management
