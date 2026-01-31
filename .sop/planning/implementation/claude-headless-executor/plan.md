# Plan: ClaudeHeadless Executor

## Test Scenarios

### 1. Binary Detection Tests

| Test | Input | Expected Output |
|------|-------|-----------------|
| `test_claude_headless_new_stores_binary_path` | N/A | `binary` field contains valid path |
| `test_claude_headless_default_output_format` | N/A | `output_format` is "json" |
| `test_claude_headless_with_custom_binary` | Custom path | Uses provided path |

### 2. Response Type Tests

| Test | Input | Expected Output |
|------|-------|-----------------|
| `test_result_type_success` | Success variant | Contains output string |
| `test_result_type_error` | Error variant | Contains message string |
| `test_claude_response_debug` | ClaudeResponse | Debug format works |
| `test_claude_response_clone` | ClaudeResponse | Clone works |

### 3. JSON Parsing Tests

| Test | Input | Expected Output |
|------|-------|-----------------|
| `test_parse_successful_json_response` | Valid success JSON | Success with output, session_id, cost |
| `test_parse_error_json_response` | Valid error JSON | Error with message |
| `test_parse_json_missing_session_id` | JSON without session_id | session_id is None |
| `test_parse_json_missing_cost` | JSON without cost | cost_usd is None |
| `test_parse_invalid_json` | Invalid JSON | Error result |

### 4. Timeout Tests

| Test | Input | Expected Output |
|------|-------|-----------------|
| `test_default_timeout` | N/A | DEFAULT_TIMEOUT_SECS is reasonable (600s) |
| `test_with_timeout` | Custom Duration | Uses provided timeout |

## Implementation Checklist

- [x] Create `src/orchestration/claude.rs`
- [x] Define `ResultType` enum
- [x] Define `ClaudeResponse` struct
- [x] Define `ClaudeHeadless` struct
- [x] Implement `ClaudeHeadless::new()`
- [x] Implement `ClaudeHeadless::with_binary()`
- [x] Implement `ClaudeHeadless::with_timeout()`
- [x] Implement helper `parse_json_response()`
- [x] Implement `ClaudeHeadless::execute()`
- [x] Add `ClaudeBinaryNotFound` error variant
- [x] Add `ClaudeExecutionFailed` error variant
- [x] Update `src/orchestration/mod.rs`
- [x] Write all tests
- [x] Run `cargo test claude` and verify passing

## File Changes

### New Files
- `src/orchestration/claude.rs`

### Modified Files
- `src/orchestration/mod.rs` - add `mod claude` and exports
- `src/error.rs` - add error variants

## Implementation Notes

1. Use `which::which("claude")` for binary detection
2. Use `tokio::process::Command` for async execution
3. Parse JSON with serde_json
4. Default timeout: 10 minutes (600 seconds)
5. Tests use mock JSON responses, not real Claude execution
