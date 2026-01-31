# Zen Improvement Proposal: Parallel Multi-Agent Orchestrator

## Current State Analysis (Ralph)

- 114 lines of bash orchestrating sequential AI agent iterations
- Solves context exhaustion by decomposing features into atomic stories
- Memory persists via git, progress.txt, and prd.json
- Works with Amp and Claude Code

## Critical Insight for Rust Rewrite

Ralph's sequential execution is an implementation choice, not a design requirement. Many user stories are naturally independent (different DB tables, unrelated components, separate API endpoints).

## Proposed Rust Architecture

1. **DAG-based scheduler** - Analyze dependencies, execute independent stories in parallel
2. **Multi-agent pool** - Claude, OpenAI, Gemini, local models working simultaneously
3. **Token budget management** - Rate limiting, cost caps, throughput optimization
4. **Event streaming** - Real-time progress, metrics, live dashboard
5. **Plugin system** - PDD and Code Task Generator as first-class integrations

## Expected Improvements

- 4-10x token throughput through parallelization
- Cross-platform single binary (no bash/jq dependencies)
- Type-safe state machine with proper error handling
- Observable with structured logging and metrics
- Extensible plugin architecture for custom agents/skills
