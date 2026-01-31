# Idea Honing: Zen Improvement

This document captures the requirements clarification process through Q&A.

---

## Q1: What is the primary success metric for Zen?

**Answer:** Scrappy, lightweight but insanely powerful - "like a sledgehammer." Key characteristics:

1. **Skills Integration** - Uses agent-sop Skills (/pdd, /code-task-generator, /code-assist, /codebase-summary) in a format Ralph can orchestrate
2. **Git Worktrees** - Parallel work isolation with elegant AI-assisted conflict resolution
3. **tmux-based** - Work survives disconnects, sessions can be attached/detached
4. **Reactive System** - Modifying plan/implementation/design propagates changes to all implementing agents
5. **Quality of Life** - Free cleanup, management, interruption handling, and other ergonomic features

---

## Q2: What is the relationship between Zen and Ralph?

**Answer:** Complete rewrite - Ralph's concepts reimplemented in Rust from scratch.

---

## Q3: How does Zen interact with AI agents?

**Answer:** Spawns existing CLI agents (Claude Code, Amp, Aider, etc.) in tmux panes. Leverages their existing tool ecosystems rather than reimplementing.

---

## Q4: Which agent CLIs must be supported at launch?

**Answer:** Claude Code is the must-have. Agent adapter should be pluggable so new CLIs (Amp, Aider, etc.) can be added easily.

---

## Q5: How does the reactive plan system work?

**Answer:** Automatically re-plan and reassign work. When user edits plan/design or agent work creates conflicts, Zen automatically adapts without requiring manual intervention.

---

## Q6: How does AI-assisted conflict resolution work?

**Answer:** Resolve on merge - standard git merge, then spawn a dedicated "resolver" agent to handle conflicts. Not the original agent, not user intervention - a specialized agent for conflict resolution.

---

## Q7: How do Skills integrate into the workflow?

**Answer:** Agent toolkit model - Agents are given Skills to use at their discretion. Zen provides the Skills as available tools, but agents decide when/how to invoke them during task execution.

---

## Q8: What does the user interface look like?

**Answer:** TUI (Terminal UI) - Interactive interface with panels and keyboard navigation (like htop/lazygit). User can also attach directly to individual agent panes to chat mid-task if desired.

---

## Q9: What Quality of Life features are essential?

**Answer:** Essential for v1:
- **Worktree cleanup** - Auto-delete merged worktrees
- **Agent health** - Detect stuck/looping agents, auto-restart

Other features (cost tracking, pause/resume, rollback, etc.) are nice-to-have for later.

---

## Q10: How does Zen persist state?

**Answer:** Git-native - Store all state in git (refs, notes, commits). Portable, versioned, no external dependencies. Fits the scrappy philosophy.

---

## Q11: How are task dependencies determined?

**Answer:** AI-inferred - Zen analyzes task descriptions and infers likely dependencies (e.g., "add user table" must come before "add user API"). Smart parallelization without manual dependency specification.

---

## Q12: What's the typical user workflow?

**Answer:**
1. **Input:** Natural language via CLI - `zen "build user authentication"`. Also supports headless mode so agents themselves can invoke zen (meta-orchestration).
2. **During execution:** User goes to do other work. Zen runs autonomously.
3. **On completion:** (Design decision) Work merges to a staging branch. User runs `zen review` to see summary, `zen accept` to merge to main, or `zen reject <task>` to rollback specific tasks.

---

## Q13: What happens when an agent fails or gets stuck?

**Answer:** AI-driven judgment. Zen uses AI to assess the situation and make the best decision given context - restart, reassign, decompose the task differently, or escalate. No rigid rules; intelligent autonomous recovery like a capable human would do if they couldn't reach their manager.

---

## Q14: What's explicitly OUT of scope for v1?

**Answer:** Explicitly deferred for v1:
- Multi-agent providers (Claude Code only, no Amp/Aider adapters)
- Cost tracking/budgets (no token limits or spend caps)
- Web UI (TUI only)
- Remote execution (local only)
- CI/CD integration (no GitHub Actions, webhooks)

---

## Status: COMPLETE
