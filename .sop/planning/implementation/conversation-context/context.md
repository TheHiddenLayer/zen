# ConversationContext Tracking - Context Document

## Task Summary
Enhance `ConversationContext` struct to track Q&A history AND extracted decisions, enabling consistent AI-as-Human responses across a workflow.

## Requirements (from task file)

1. **Q&A Recording** - Record question/answer pairs to qa_history
2. **Decision Extraction** - Extract key decisions (naming, technology, architectural)
3. **Context Summary** - Format Q&A history for prompt inclusion
4. **Empty Context** - Handle empty context gracefully

## Technical Requirements

### Struct Enhancement
```rust
pub struct ConversationContext {
    qa_history: Vec<(String, String)>,
    decisions: HashMap<String, String>,  // NEW: Add decisions tracking
}
```

### Decision Categories to Extract
1. **Naming decisions** - When question contains "name" and answer provides a name
2. **Technology/library choices** - Database, framework, library choices
3. **Architectural decisions** - Patterns, approaches, structure choices

## Existing Implementation Analysis

The current `ConversationContext` in `src/orchestration/ai_human.rs`:
- Has `qa_history: Vec<(String, String)>` field
- Has `record()` that adds to qa_history (no decision extraction)
- Has `summary()` that formats Q&A pairs
- Has `is_empty()` and `len()` helpers

## Dependencies
- `std::collections::HashMap` (needs to be added)
- Existing `ConversationContext` struct

## Implementation Paths
- File: `src/orchestration/ai_human.rs`
- Add `decisions` field to struct
- Enhance `record()` to extract decisions
- Add accessor method for decisions
