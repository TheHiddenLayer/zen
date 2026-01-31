# Skills Integration Guide

This document explains how Zen orchestrates Claude Code Skills, the AI-as-Human pattern, and how to extend the system with new skills.

---

## Overview

Zen uses a **Skills-Driven Workflow** where existing Claude Code skills handle the heavy lifting:

```
User Prompt ─► /pdd ─► /code-task-generator ─► /code-assist (parallel) ─► Merge
```

Instead of implementing planning and coding logic directly, Zen orchestrates proven skills and uses an **AI-as-Human Proxy** to answer clarification questions autonomously.

---

## Skills-Driven Workflow

### Phase 1: Planning (`/pdd`)

The PDD (Prompt-Driven Development) skill transforms a rough idea into a detailed design.

**Zen's Role:**
- Spawns an agent with the `/pdd` skill
- Monitors output for clarification questions
- Uses AI-as-Human to answer questions autonomously
- Collects artifacts: `detailed-design.md`, `plan.md`

```rust
// SkillsOrchestrator::run_pdd_phase
agent.send(&format!("/pdd\n\nrough_idea: {}", prompt)).await?;

loop {
    match agent.read_output().await? {
        AgentOutput::Question(q) => {
            let answer = self.ai_human.answer_question(&q).await;
            agent.send(&answer).await?;
        }
        AgentOutput::Completed => break,
        AgentOutput::Error(e) => return Err(e.into()),
        _ => continue,
    }
}
```

### Phase 2: Task Generation (`/code-task-generator`)

Breaks the plan into parallelizable tasks.

**Zen's Role:**
- Feeds PDD artifacts to the skill
- Monitors for approval/adjustment questions
- Collects generated `.code-task.md` files

**Output Format:**
```markdown
# Task: Create User Model

## Description
Create the User model with email, password hash, and timestamps.

## Acceptance Criteria
- [ ] User struct with required fields
- [ ] Password hashing implementation
- [ ] Unit tests for model

## Dependencies
- task-01-database-setup

## Metadata
- **Complexity**: Medium
```

### Phase 3: Implementation (`/code-assist`)

Executes tasks in parallel using the DAG scheduler.

**Zen's Role:**
- Builds TaskDAG from generated tasks
- Spawns parallel agents in isolated worktrees
- Each agent runs `/code-assist` in auto mode
- Monitors progress and collects commits

```
┌────────────────────────────────────────────────────────┐
│              Phase 3: Parallel Implementation           │
│                                                         │
│   DAG: A ──► C                                         │
│         B ──┘                                          │
│                                                         │
│   Step 1: Spawn A, B (independent)                     │
│   ┌───────────┐  ┌───────────┐                        │
│   │ Worktree A │  │ Worktree B │                        │
│   │ /code-assist│  │ /code-assist│                        │
│   └─────┬─────┘  └─────┬─────┘                        │
│         │              │                               │
│   Step 2: Wait for A, B to complete                    │
│         └──────┬───────┘                               │
│                ▼                                        │
│   Step 3: Spawn C (dependencies satisfied)             │
│   ┌───────────┐                                        │
│   │ Worktree C │                                        │
│   │ /code-assist│                                        │
│   └───────────┘                                        │
└────────────────────────────────────────────────────────┘
```

### Phase 4: Merge & Resolve

Merges completed worktrees to a staging branch.

**Zen's Role:**
- Merges each worktree sequentially
- Detects merge conflicts
- Spawns resolver agent to fix conflicts
- Creates unified staging branch

### Phase 5: Documentation (`/codebase-summary`)

Optional phase to update project documentation.

**Zen's Role:**
- Controlled by `WorkflowConfig.update_docs`
- Runs `/codebase-summary` skill
- Updates AGENTS.md, README.md, etc.

---

## AI-as-Human Pattern

Skills are designed for human interaction - they ask clarifying questions one at a time. Zen implements an **AI-as-Human Proxy** that answers these questions autonomously.

### How It Works

```
┌────────────────────────────────────────────────────────┐
│                   AI-as-Human Pattern                   │
│                                                         │
│   Agent Running /pdd                                    │
│   ┌─────────────────────────────────────────────────┐  │
│   │ "What database should we use?"                   │  │
│   │    1. PostgreSQL                                 │  │
│   │    2. MySQL                                      │  │
│   │    3. SQLite                                     │  │
│   └─────────────────────────────────────────────────┘  │
│                         │                               │
│                         ▼                               │
│   ┌─────────────────────────────────────────────────┐  │
│   │              AIHumanProxy                        │  │
│   │                                                  │  │
│   │  Original Prompt: "Build user auth with OAuth"   │  │
│   │  Context: Previous Q&A history                   │  │
│   │                                                  │  │
│   │  → Generates answer: "1" (PostgreSQL)            │  │
│   │  → Records decision in context                   │  │
│   └─────────────────────────────────────────────────┘  │
│                         │                               │
│                         ▼                               │
│   Agent continues with PostgreSQL                      │
└────────────────────────────────────────────────────────┘
```

### Key Components

**AIHumanProxy** (`orchestration/ai_human.rs`):
```rust
pub struct AIHumanProxy {
    original_prompt: String,
    context: Arc<RwLock<ConversationContext>>,
    model: String,  // Fast model for quick responses
}
```

**ConversationContext**:
- Tracks Q&A history for consistent follow-up answers
- Extracts key decisions (naming, database, technology choices)
- Provides summary for context in future questions

### Answer Generation Strategy

The proxy generates answers based on:
1. **Original user intent** - What did the user ask for?
2. **Accumulated context** - What decisions were already made?
3. **Best practices** - What's the sensible default?

```rust
let prompt = format!(
    r#"You are acting as a decisive human user who originally requested:
"{}"

The AI assistant is now asking you this clarification question:
{}

INSTRUCTIONS:
- Answer based on the original request and best practices
- Be CONCISE and DECISIVE - pick the most sensible option
- If given numbered options, respond with just the number
- If asked yes/no, respond with just "yes" or "no"
- Don't explain your reasoning, just answer

Previous conversation context:
{}

YOUR ANSWER:"#,
    self.original_prompt,
    question,
    context.summary()
);
```

### Escalation

Some questions need real human input. The proxy detects these:

```rust
fn needs_escalation(&self, question: &str) -> bool {
    let ambiguous_patterns = [
        "which approach do you prefer",
        "what style do you want",
        "personal preference",
    ];
    ambiguous_patterns.iter().any(|p| question.to_lowercase().contains(p))
}
```

When escalation is needed, Zen pauses and prompts the user.

---

## Question Detection

Zen monitors agent output for questions using pattern matching:

**Detection Patterns** (`orchestration/detection.rs`):
- Direct questions (`?` at end of line)
- Numbered options (`1. Option A`, `2. Option B`)
- Yes/No prompts (`(y/n)`, `[Y/n]`)
- Input prompts (`Enter`, `Provide`, `Specify`)

```rust
pub fn is_question(text: &str) -> bool {
    // Check for question marks
    if text.trim().ends_with('?') {
        return true;
    }
    // Check for numbered options
    if NUMBERED_OPTIONS.is_match(text) {
        return true;
    }
    // Check for yes/no patterns
    if YES_NO_PATTERN.is_match(text) {
        return true;
    }
    false
}
```

---

## Adding New Skills

To integrate a new skill into the workflow:

### 1. Create Phase Runner

Add a method to `SkillsOrchestrator`:

```rust
async fn run_my_skill_phase(&self, input: &MyInput) -> Result<MyOutput> {
    // Spawn agent for the skill
    let agent = self.agent_pool.write().await
        .spawn_for_skill("my-skill").await?;

    // Send skill command with input
    agent.send(&format!("/my-skill\n\ninput: {}", input.to_string())).await?;

    // Monitor and answer questions
    loop {
        match agent.read_output().await? {
            AgentOutput::Question(q) => {
                let answer = self.ai_human.answer_question(&q).await;
                agent.send(&answer).await?;
            }
            AgentOutput::Completed => break,
            AgentOutput::Error(e) => return Err(e.into()),
            _ => continue,
        }
    }

    // Parse and return output
    Ok(MyOutput::from_directory(&agent.worktree_path())?)
}
```

### 2. Define Output Parser

Create a struct to parse skill artifacts:

```rust
pub struct MyOutput {
    pub artifact_path: PathBuf,
    pub metadata: MyMetadata,
}

impl MyOutput {
    pub fn from_directory(path: &Path) -> Result<Self> {
        // Parse artifacts from the directory
    }
}
```

### 3. Integrate into Workflow

Add the phase to `SkillsOrchestrator::execute()`:

```rust
pub async fn execute(&self, prompt: &str) -> Result<WorkflowResult> {
    // ... existing phases ...

    // NEW PHASE
    self.phase_controller.transition(WorkflowPhase::MyPhase);
    let my_result = self.run_my_skill_phase(&my_input).await?;

    // ... continue workflow ...
}
```

### 4. Add Phase Variant

Update `WorkflowPhase` enum:

```rust
pub enum WorkflowPhase {
    Planning,
    TaskGeneration,
    Implementation,
    MyPhase,  // New phase
    Merging,
    Documentation,
    Complete,
}
```

### 5. Update Phase Transitions

Update `WorkflowState::valid_transitions()`:

```rust
fn valid_transitions(&self) -> Vec<WorkflowPhase> {
    match self.current_phase {
        // ... existing transitions ...
        WorkflowPhase::Implementation => vec![WorkflowPhase::MyPhase],
        WorkflowPhase::MyPhase => vec![WorkflowPhase::Merging],
        // ...
    }
}
```

---

## Best Practices

### Skill Integration

1. **Let skills do the work** - Don't reimplement what skills already do well
2. **Monitor, don't micromanage** - Trust skill output, intervene only on errors
3. **Preserve artifacts** - Skills generate valuable artifacts, save them

### AI-as-Human Responses

1. **Be decisive** - Pick sensible defaults, don't hedge
2. **Be concise** - One-word answers when possible
3. **Be consistent** - Use context to maintain consistency
4. **Escalate appropriately** - Some questions need real humans

### Error Handling

1. **Retry transient errors** - Network issues, rate limits
2. **Log skill output** - Helps debug failures
3. **Graceful degradation** - Continue workflow if optional phase fails

---

## Troubleshooting

### Skill Not Responding

**Symptoms:** Agent appears stuck, no output
**Solution:** Check tmux session directly: `zen attach <agent_id>`

### Wrong Answers from AI-as-Human

**Symptoms:** Skill goes in unexpected direction
**Solution:** Review context accumulation, add escalation patterns

### Missing Artifacts

**Symptoms:** Phase completes but artifacts not found
**Solution:** Check worktree path, verify skill output locations

### Phase Transition Errors

**Symptoms:** "Invalid phase transition" error
**Solution:** Review `WorkflowState::valid_transitions()`, ensure phases are connected
