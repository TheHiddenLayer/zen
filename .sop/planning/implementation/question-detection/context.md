# Question Detection Context

## Task Summary
Implement robust question detection patterns in `src/orchestration/detection.rs` to identify when an agent is asking for user input, enabling the AI-as-Human proxy to respond appropriately.

## Requirements

### Technical Requirements (from task file)
1. Create `src/orchestration/detection.rs` with:
   - `is_question(text: &str) -> bool`
   - `extract_question(text: &str) -> Option<String>`
   - `is_waiting_for_input(output: &str) -> bool`

2. Implement detection patterns:
   - Direct questions (ending with `?`)
   - Numbered options ("1. Option A\n2. Option B")
   - Yes/No prompts
   - Input prompts ("Enter", "Please provide", "What is")

3. Detect waiting state (cursor position, prompt patterns)
4. Integrate with `AgentOutput::Question` variant

### Acceptance Criteria
1. Direct Question Detection: "Which database should we use?" → `is_question()` returns true
2. Numbered Options Detection: "Choose an option:\n1. PostgreSQL\n2. MySQL" → true
3. Non-Question Output: "Creating design document..." → false
4. Question Extraction: Extracts just the question text from surrounding context
5. Waiting State Detection: Detects when agent is waiting at a prompt

## Existing Patterns

### Current Question Detection (pool.rs:105-133)
The existing `AgentOutput::parse()` already has basic question detection:
- Question mark at end of line
- Common question phrases: "do you want", "would you like", "should i", "shall i", etc.

### Current Detection Patterns
```rust
// Question patterns (existing)
- Line ending with `?`
- "do you want", "would you like", "should i", "shall i"
- "can i", "may i", "please confirm", "please select"
- "choose one", "select an option", "enter your", "type your"

// Completion patterns (existing)
- "task completed", "successfully completed", "all tests pass"
- "build successful", "implementation complete"
- "done!", "finished!", "complete!", "✓ all done", "✅ done"

// Error patterns (existing)
- "error:", "failed:", "failure:", "fatal:", "panic:"
- "exception:", "could not", "cannot ", "unable to"
- "❌", "✗"
```

## Design Decisions

### Module Organization
Create a dedicated `detection.rs` module that can be used by both:
- `AgentOutput::parse()` in pool.rs (refactor to use detection)
- `AIHumanProxy` for question identification
- `SkillsOrchestrator` monitor loop

### Function Design
1. `is_question(text: &str) -> bool` - Check if text contains a question
2. `extract_question(text: &str) -> Option<String>` - Extract clean question text
3. `is_waiting_for_input(output: &str) -> bool` - Detect prompt/waiting state

### Integration Approach
Update `AgentOutput::contains_question_pattern()` to use the new `detection::is_question()` function to centralize question detection logic.

## Dependencies
- AgentHandle.read_output() from Step 4 ✅
- monitor_agent_output() from Step 6 ✅

## Implementation Path
1. Create `src/orchestration/detection.rs`
2. Implement core detection functions with regex patterns
3. Add comprehensive tests
4. Update `pool.rs` to use the new detection module
5. Export from `orchestration/mod.rs`
