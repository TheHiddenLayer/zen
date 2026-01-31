# ConversationContext Tracking - Implementation Plan

## Test Scenarios

### 1. Decision Extraction - Naming Decisions
- **Input**: Q: "What should we name the module?" A: "user_auth"
- **Expected**: decisions["naming"] contains "user_auth"
- **Variations**: "name the", "what name", "naming convention"

### 2. Decision Extraction - Technology Choices
- **Input**: Q: "Which database?" A: "PostgreSQL"
- **Expected**: decisions["database"] contains "PostgreSQL"
- **Variations**: database, framework, library choices

### 3. Decision Extraction - Architectural Decisions
- **Input**: Q: "What pattern should we use?" A: "Repository pattern"
- **Expected**: decisions["architecture"] contains "Repository pattern"
- **Variations**: pattern, approach, structure

### 4. Context Summary with Decisions
- **Input**: 3 Q&A pairs with various decisions
- **Expected**: summary() returns formatted string with all Q&A

### 5. Empty Context Handling
- **Input**: New ConversationContext
- **Expected**: summary() returns empty string, decisions is empty

### 6. Multiple Decisions
- **Input**: Record multiple decisions of different types
- **Expected**: All decisions tracked independently

### 7. Decision Override
- **Input**: Two naming questions with different answers
- **Expected**: Later answer overwrites earlier (latest decision wins)

## Implementation Plan

### Step 1: Add HashMap import
Add `use std::collections::HashMap;` if not present

### Step 2: Add decisions field to ConversationContext
```rust
decisions: HashMap<String, String>,
```

### Step 3: Update Default/new implementations
Initialize decisions as empty HashMap

### Step 4: Implement decision extraction in record()
- Check question for naming keywords -> store in "naming"
- Check question for database keywords -> store in "database"
- Check question for framework/library keywords -> store in "technology"
- Check question for pattern/architecture keywords -> store in "architecture"

### Step 5: Add decisions accessor
```rust
pub fn decisions(&self) -> &HashMap<String, String>
```

### Step 6: Update Clone derive (HashMap is Clone)

## Acceptance Criteria Mapping

| Criteria | Test Scenario |
|----------|---------------|
| Q&A Recording | Existing tests (already pass) |
| Decision Extraction | Scenarios 1-3 |
| Context Summary | Scenario 4 |
| Consistent Answers | Scenario 6 (decisions preserved) |
| Empty Context | Scenario 5 |
