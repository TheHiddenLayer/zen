# Context: ClaudeHeadless Executor

## Overview

This task creates the `ClaudeHeadless` struct that executes Claude Code in headless mode (`-p` flag) with JSON output parsing, enabling programmatic interaction with the AI agent.

## Requirements

### Functional Requirements

1. **Binary Detection** - Detect Claude Code binary path using `which` crate
2. **Successful Execution** - Execute Claude with prompt and working directory, return structured response
3. **JSON Parsing** - Parse JSON output format extracting session_id, result, and cost
4. **Error Handling** - Handle non-zero exit codes and return appropriate Error variant
5. **Timeout Handling** - Support configurable timeout for long-running operations

### Acceptance Criteria

1. `ClaudeHeadless::new()` detects the binary path correctly
2. `execute(prompt, cwd)` returns `ClaudeResponse` with Success result
3. JSON response is correctly parsed (session_id, result, cost)
4. Error responses return `ClaudeResponse` with Error result
5. Timeout exceeded returns appropriate timeout error

## Technical Specifications

### From Research (claude-code-integration.md)

**Headless Mode**:
- Use `claude -p "prompt" --output-format json`
- Exit code 0 = success, non-zero = error

**JSON Response Structure**:
```json
{
  "type": "result",
  "subtype": "success",  // or "error"
  "total_cost_usd": 0.003,
  "duration_ms": 1234,
  "num_turns": 6,
  "result": "Response text...",
  "session_id": "abc123"
}
```

### Structs to Implement

```rust
pub struct ClaudeHeadless {
    binary: PathBuf,
    output_format: String,  // "json"
}

pub struct ClaudeResponse {
    pub session_id: Option<String>,
    pub result: ResultType,
    pub cost_usd: Option<f64>,
}

pub enum ResultType {
    Success { output: String },
    Error { message: String },
}
```

### Methods to Implement

- `ClaudeHeadless::new() -> Result<Self>` - Detect binary, create instance
- `ClaudeHeadless::execute(&self, prompt: &str, cwd: &Path) -> Result<ClaudeResponse>` - Execute and parse

## Existing Patterns

### From `pool.rs`

- Uses `tokio::process::Command` pattern is NOT used yet (uses std::process::Command)
- Error handling via `crate::error::{Error, Result}`
- Module exports via `mod.rs`

### From `Cargo.toml`

Dependencies available:
- `tokio` with `process` feature
- `serde`, `serde_json` for JSON parsing
- `which = "8"` for binary detection
- `thiserror = "2"` for error types

### From `error.rs`

Error pattern:
```rust
#[derive(Error, Debug)]
pub enum Error {
    #[error("...")]
    Variant { field: Type },
}
```

## Implementation Path

1. Create `src/orchestration/claude.rs`
2. Add module to `src/orchestration/mod.rs`
3. Add error variants to `src/error.rs` if needed
4. Write tests following existing patterns

## Dependencies

- `tokio::process` - async command execution
- `serde`, `serde_json` - JSON parsing
- `which` - binary detection
- `std::time::Duration` - timeout handling
