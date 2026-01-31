# Claude Code CLI Integration Research

**Date**: 2026-01-30
**Purpose**: Document how to programmatically integrate Claude Code CLI with the Zen orchestrator

---

## Executive Summary

Claude Code (now using the Claude Agent SDK) provides multiple interfaces for programmatic integration:
1. **CLI with headless mode** (`-p` flag) for scripting and automation
2. **TypeScript SDK** (`@anthropic-ai/claude-code` on npm)
3. **Python SDK** (`claude-code-sdk` on PyPI)

The CLI's headless mode is the simplest integration path, while the SDKs offer full programmatic control for more sophisticated orchestration scenarios.

---

## 1. Spawning Claude Code Programmatically

### 1.1 Basic Command Structure

```bash
# Interactive mode
claude

# Headless/non-interactive mode
claude -p "prompt text"

# With piped input
cat file.txt | claude -p "analyze this"

# With specific model
claude -p "prompt" --model sonnet
claude -p "prompt" --model opus
claude -p "prompt" --model claude-sonnet-4-20250514
```

### 1.2 Key Command-Line Arguments

| Flag | Purpose | Example |
|------|---------|---------|
| `-p, --print` | Headless mode - non-interactive execution | `claude -p "review code"` |
| `--output-format` | Output format: `text`, `json`, `stream-json` | `--output-format json` |
| `--model` | Select Claude model | `--model opus` |
| `--add-dir` | Add working directories | `--add-dir ../lib ../apps` |
| `--continue, -c` | Continue most recent session | `claude -c` |
| `--resume, -r` | Resume specific session by ID | `claude -r abc123` |
| `--append-system-prompt` | Add to system instructions | `--append-system-prompt "Be concise"` |
| `--dangerously-skip-permissions` | Skip permission checks (use with caution) | For trusted automation only |
| `--agents` | Define custom subagents (JSON) | `--agents '{"name": {...}}'` |

### 1.3 Installation

```bash
# Install globally via npm
npm install -g @anthropic-ai/claude-code

# Verify installation
claude --version
```

---

## 2. Headless/Non-Interactive Mode

### 2.1 The `-p` Flag

The `-p` or `--print` flag enables headless mode for automation scenarios:
- No interactive CLI interface
- Executes task and returns result to stdout
- Perfect for CI/CD, pre-commit hooks, build scripts

**Key Characteristics**:
- Does NOT persist state between invocations by default
- User-invoked skills (like `/commit`) are NOT available in `-p` mode
- Built-in slash commands are only available in interactive mode
- Describe the task you want instead of using slash commands

### 2.2 Output Formats

Three output modes are available via `--output-format`:

#### Text (default)
```bash
claude -p "explain this code" < file.js
# Returns plain text to stdout
```

#### JSON (single object at completion)
```bash
claude -p "review code" --output-format json
```

**JSON Structure**:
```json
{
  "type": "result",
  "subtype": "success",
  "total_cost_usd": 0.003,
  "duration_ms": 1234,
  "num_turns": 6,
  "result": "Response text...",
  "session_id": "abc123"
}
```

#### Stream JSON (newline-delimited JSON)
```bash
claude -p "analyze codebase" --output-format stream-json
```

Emits multiple JSON objects as the agent progresses (newline-delimited). Best for real-time monitoring.

### 2.3 Programmatic Usage Patterns

```bash
# Single task execution
claude -p "lint all Python files" --output-format json > result.json

# With custom system prompt
cat pr.diff | claude -p "review for security issues" \
  --append-system-prompt "Focus on SQL injection and XSS"

# Parsing output with jq
claude -p "list all TODO comments" --output-format json | jq '.result'

# Error handling pattern
if ! claude -p "run tests" --output-format json > output.json; then
  echo "Claude execution failed with exit code $?"
  exit 1
fi
```

---

## 3. Detecting Completion and Errors

### 3.1 Exit Codes

**Standard Behavior**:
- Exit code `0`: Success
- Exit code `1`: Error/failure
- Exit code `≠ 0`: Non-zero exit indicates error

**Error Detection**:
```bash
claude -p "task" --output-format json
EXIT_CODE=$?

if [ $EXIT_CODE -eq 0 ]; then
  echo "Task completed successfully"
else
  echo "Task failed with exit code $EXIT_CODE"
fi
```

### 3.2 JSON Output Parsing

When using `--output-format json`, parse the response to determine status:

```bash
# Extract session ID
SESSION_ID=$(jq -r '.session_id' < result.json)

# Check for success
SUBTYPE=$(jq -r '.subtype' < result.json)
if [ "$SUBTYPE" = "success" ]; then
  echo "Task completed successfully"
fi

# Get cost and duration
COST=$(jq -r '.total_cost_usd' < result.json)
DURATION=$(jq -r '.duration_ms' < result.json)
```

### 3.3 Known Issues and Limitations

**Exit Code 1 Errors**:
- Common error: "Claude Code process exited with code 1"
- May occur with authentication issues or hook failures
- Check stderr for meaningful error messages

**Hook Exit Codes**:
- PostToolUse hooks: Non-zero exit codes should be non-blocking, but currently may block execution
- Hook exit code 1 can prevent Claude from continuing

**Premature Termination**:
- Claude may terminate before completing all todos in some cases
- Implement safeguards to detect incomplete work

### 3.4 Intelligent Exit Detection (Ralph Framework Pattern)

For autonomous loops, implement sophisticated exit detection:

**Dual-Condition Exit Gate**:
- Requires BOTH completion indicators AND explicit EXIT_SIGNAL
- Prevents premature exit

**Circuit Breaker Thresholds**:
```bash
CB_NO_PROGRESS_THRESHOLD=3      # Open circuit after 3 loops with no file changes
CB_SAME_ERROR_THRESHOLD=5        # Open circuit after 5 loops with repeated errors
CB_OUTPUT_DECLINE_THRESHOLD=70%  # Open circuit if output declines by >70%
```

**Rate Limiting**:
- Prevent runaway loops
- Add delays between iterations

---

## 4. Output Parsing Strategies

### 4.1 Completion Detection

**JSON Mode**:
```javascript
const result = JSON.parse(output);

// Check completion status
if (result.subtype === 'success') {
  console.log('Task completed successfully');
  console.log('Session ID:', result.session_id);
  console.log('Cost:', result.total_cost_usd);
  console.log('Duration:', result.duration_ms, 'ms');
  console.log('Turns:', result.num_turns);
}
```

**Stream JSON Mode**:
```javascript
// Process newline-delimited JSON
const lines = output.split('\n').filter(Boolean);
for (const line of lines) {
  const message = JSON.parse(line);

  if (message.type === 'result') {
    // Final result
    console.log('Completed:', message.result);
  } else if (message.type === 'tool_use') {
    // Tool invocation in progress
    console.log('Tool:', message.name);
  }
}
```

### 4.2 Error Detection in Output

```bash
# Check for errors in JSON output
ERROR_COUNT=$(jq -r '.errors | length' < result.json 2>/dev/null || echo "0")

if [ "$ERROR_COUNT" -gt 0 ]; then
  echo "Errors detected:"
  jq -r '.errors[]' < result.json
fi

# Parse stderr for runtime errors
claude -p "task" 2> errors.log
if [ -s errors.log ]; then
  echo "Runtime errors occurred:"
  cat errors.log
fi
```

### 4.3 Progress Monitoring

For long-running tasks with `stream-json`:

```python
import subprocess
import json

proc = subprocess.Popen(
    ['claude', '-p', 'complex task', '--output-format', 'stream-json'],
    stdout=subprocess.PIPE,
    stderr=subprocess.PIPE,
    text=True
)

for line in proc.stdout:
    message = json.loads(line)

    if message.get('type') == 'tool_use':
        print(f"Using tool: {message.get('name')}")
    elif message.get('type') == 'text':
        print(f"Output: {message.get('content')}")
    elif message.get('type') == 'result':
        print(f"Completed: {message.get('result')}")

proc.wait()
exit_code = proc.returncode
```

---

## 5. Session Management

### 5.1 Session Basics

**Session Storage**:
- Sessions are stored in `~/.claude/` directory
- Each session is saved as line-delimited JSON
- Sessions can be resumed from any directory with session ID

**Session ID Capture**:
```bash
# Get session ID from JSON output
SESSION_ID=$(claude -p "task" --output-format json | jq -r '.session_id')
echo "Session ID: $SESSION_ID"
```

### 5.2 Resuming Sessions

**Continue Most Recent**:
```bash
# Resume most recent conversation
claude -c "continue with the previous task"
claude --continue "what's next?"
```

**Resume Specific Session**:
```bash
# Resume by session ID
claude -r abc123 "finish the implementation"
claude --resume abc123 "what was the status?"
```

**Interactive Session Picker**:
```bash
# Shows list of recent sessions to choose from
claude --resume
```

### 5.3 Session Persistence (CLI)

The CLI automatically saves sessions:
- Each invocation creates/updates session file in `~/.claude/`
- Sessions persist across directory changes
- Use `--resume` with session ID to continue from anywhere

### 5.4 Programmatic Session Management (SDK)

**TypeScript SDK**:
```typescript
import { createClaudeAgent } from '@anthropic-ai/claude-code';

// Start new session
const agent = createClaudeAgent({ apiKey: process.env.ANTHROPIC_API_KEY });
const stream = agent.query('initial task');

let sessionId: string;

for await (const message of stream) {
  if (message.type === 'system' && message.subtype === 'init') {
    sessionId = message.session_id;
    console.log('Session ID:', sessionId);
    // Save sessionId to database/file for later
  }
}

// Resume session later
const resumedStream = agent.query('continue task', {
  resume: savedSessionId
});

// Fork a session (new ID, same state)
const forkedStream = agent.query('try alternative approach', {
  resume: savedSessionId,
  forkSession: true
});
```

**Python SDK**:
```python
from claude_code_sdk import ClaudeAgent

# Start new session
agent = ClaudeAgent(api_key=os.environ['ANTHROPIC_API_KEY'])

async for message in agent.query('initial task'):
    if message['type'] == 'system' and message['subtype'] == 'init':
        session_id = message['session_id']
        print(f'Session ID: {session_id}')
        # Save session_id for later

# Resume session later
async for message in agent.query('continue task', resume=session_id):
    process_message(message)

# Fork session
async for message in agent.query('alternative', resume=session_id, fork_session=True):
    process_message(message)
```

### 5.5 Known Session Issues

**Session ID Changes on Resume**:
- Bug: When resuming via `--resume`, the session_id may change to a new UUID
- Workaround: Track both original and resumed session IDs
- Impact: Hooks receive new session_id, not original

**Session Isolation**:
- Sessions maintain conversation history but not working directory state
- File changes persist, but Claude's context is from the session
- Use `--add-dir` to ensure correct working directories when resuming

---

## 6. Advanced Integration Features

### 6.1 Subagent Orchestration

**Custom Subagents** (via `--agents` flag):
```bash
claude -p "task" --agents '{
  "reviewer": {
    "model": "claude-opus-4-5",
    "system_prompt": "You are a code reviewer",
    "max_turns": 5
  },
  "tester": {
    "model": "claude-sonnet-4",
    "system_prompt": "You write comprehensive tests",
    "max_turns": 3
  }
}'
```

**Built-in Subagents**:
- Claude automatically uses Task tool and Explore tool
- These spawn independent sub-agents that report back
- Enable parallel execution and task delegation

**Parallel Execution Pattern**:
```bash
# Run multiple Claude instances in parallel
claude -p "implement feature A" --output-format json > result_a.json &
PID_A=$!

claude -p "implement feature B" --output-format json > result_b.json &
PID_B=$!

# Wait for both to complete
wait $PID_A $PID_B

# Analyze results
jq -s '.' result_a.json result_b.json
```

### 6.2 Custom Commands

**Purpose**: Reusable prompt templates for repeated workflows

**Setup**:
1. Create `.claude/commands/` directory
2. Add `.md` files with command names
3. Use `$ARGUMENTS`, `$1`, `$2`, etc. for parameters

**Example**: `.claude/commands/review.md`
```markdown
Review the following code for security vulnerabilities,
focusing on SQL injection, XSS, and authentication issues.

File to review: $1

$ARGUMENTS
```

**Usage**:
```bash
# In interactive mode
/review auth.py

# In headless mode (describe instead)
claude -p "review auth.py for security issues"
```

**Note**: Custom slash commands only work in interactive mode. In `-p` mode, describe the task directly.

### 6.3 Hooks for Event-Driven Automation

**Hook Types**:
- Pre-prompt hooks: Run before Claude processes prompt
- Post-tool-use hooks: Run after tool execution
- Pre-response hooks: Run before Claude sends response

**Configuration** (`.claude/hooks.json`):
```json
{
  "hooks": [
    {
      "name": "format-on-write",
      "trigger": "post-tool-use",
      "condition": {
        "tool": "Write",
        "file_pattern": "**/*.py"
      },
      "command": "black $FILE_PATH"
    },
    {
      "name": "run-tests",
      "trigger": "post-tool-use",
      "condition": {
        "tool": "Edit",
        "file_pattern": "**/src/**"
      },
      "command": "npm test"
    }
  ]
}
```

**Exit Code Behavior**:
- Exit code 0: Success, continue
- Exit code 1: Should be non-blocking, but currently may block
- Other codes: Non-blocking error, stderr shown to user

### 6.4 Model Context Protocol (MCP)

**Purpose**: Extend Claude Code with custom tools and data sources

**Integration**:
- Connect to MCP servers for internal tools, databases, APIs
- Servers provide additional capabilities beyond built-in tools
- Available in both interactive and headless modes

**Configuration**: `.claude/mcp-servers.json`
```json
{
  "mcpServers": {
    "internal-db": {
      "command": "node",
      "args": ["./mcp-servers/database.js"],
      "env": {
        "DB_CONNECTION_STRING": "..."
      }
    }
  }
}
```

### 6.5 Permission Management

**Permission Modes**:
- Interactive: Claude asks for permission (default)
- Automatic: Auto-approve whitelisted commands
- Restricted: Block blacklisted commands

**Configuration**: `.claude/settings.json`
```json
{
  "permissions": {
    "whitelist": [
      "git status",
      "git diff",
      "npm test",
      "pytest"
    ],
    "blacklist": [
      "rm -rf",
      "sudo",
      "curl | sh"
    ]
  }
}
```

**Bypass Permissions** (for trusted automation):
```bash
claude -p "task" --dangerously-skip-permissions
```

⚠️ **Use with extreme caution** - only in sandboxed/trusted environments

---

## 7. SDK-Based Integration

### 7.1 TypeScript SDK

**Installation**:
```bash
npm install @anthropic-ai/claude-code
```

**Basic Usage**:
```typescript
import { createClaudeAgent } from '@anthropic-ai/claude-code';

const agent = createClaudeAgent({
  apiKey: process.env.ANTHROPIC_API_KEY,
  model: 'claude-sonnet-4',
});

// Query returns async iterator
const stream = agent.query('Review this codebase for bugs');

for await (const message of stream) {
  if (message.type === 'text') {
    console.log(message.content);
  } else if (message.type === 'tool_use') {
    console.log(`Tool: ${message.name}`);
  } else if (message.type === 'result') {
    console.log(`Completed: ${message.result}`);
  }
}
```

**V2 Interface (Simplified)**:
```typescript
// New send/receive pattern
const agent = createClaudeAgentV2({ apiKey: '...' });

// Send prompt
await agent.send('Implement authentication');

// Receive messages
for await (const message of agent.receive()) {
  console.log(message);
}

// Multi-turn conversation is easier with V2
await agent.send('Now add tests');
for await (const message of agent.receive()) {
  // ...
}
```

**Features**:
- Full programmatic control
- Session management (resume, fork)
- Custom tool definitions
- Streaming responses
- Error handling

### 7.2 Python SDK

**Installation**:
```bash
pip install claude-code-sdk
```

**Basic Usage**:
```python
import os
from claude_code_sdk import ClaudeAgent

agent = ClaudeAgent(
    api_key=os.environ['ANTHROPIC_API_KEY'],
    model='claude-sonnet-4'
)

# Query returns async iterator
async for message in agent.query('Analyze this project structure'):
    if message['type'] == 'text':
        print(message['content'])
    elif message['type'] == 'tool_use':
        print(f"Tool: {message['name']}")
    elif message['type'] == 'result':
        print(f"Completed: {message['result']}")
```

**Session Management**:
```python
# Start session and capture ID
session_id = None
async for message in agent.query('Start task'):
    if message['type'] == 'system' and message['subtype'] == 'init':
        session_id = message['session_id']
        save_session_id(session_id)

# Resume later
async for message in agent.query('Continue task', resume=session_id):
    process(message)
```

**Features**:
- Native async/await support
- Type hints for better IDE support
- Same capabilities as TypeScript SDK
- Each query() call is independent unless resumed

### 7.3 SDK vs CLI Comparison

| Feature | CLI (`-p` mode) | TypeScript/Python SDK |
|---------|-----------------|----------------------|
| Ease of use | ⭐⭐⭐⭐⭐ Simple | ⭐⭐⭐ Requires coding |
| Control | ⭐⭐⭐ Limited | ⭐⭐⭐⭐⭐ Full control |
| Streaming | ⭐⭐⭐⭐ stream-json | ⭐⭐⭐⭐⭐ Native async |
| Session mgmt | ⭐⭐⭐⭐ Auto-saved | ⭐⭐⭐⭐⭐ Manual control |
| Error handling | ⭐⭐⭐ Exit codes | ⭐⭐⭐⭐⭐ Exceptions |
| Parallelization | ⭐⭐⭐ Shell scripting | ⭐⭐⭐⭐⭐ Native async |
| Custom tools | ⭐⭐⭐ Via MCP | ⭐⭐⭐⭐⭐ Direct API |
| Best for | Scripts, CI/CD | Complex orchestration |

### 7.4 Authentication

**API Key** (all interfaces):
```bash
export ANTHROPIC_API_KEY='sk-ant-...'
```

**Alternative Providers**:
```bash
# Amazon Bedrock
export CLAUDE_CODE_USE_BEDROCK=1

# Google Vertex AI
export CLAUDE_CODE_USE_VERTEX=1

# Microsoft Foundry
export CLAUDE_CODE_USE_FOUNDRY=1
```

---

## 8. Integration Patterns for Zen Orchestrator

### 8.1 Recommended Approach

For Zen orchestrator, recommend **hybrid approach**:

1. **CLI (`-p` mode) for simple delegations**:
   - Single-task executions
   - File-scoped operations
   - When streaming not needed

2. **TypeScript/Python SDK for complex orchestration**:
   - Multi-turn conversations
   - Parallel execution
   - Session forking and state management
   - Custom tool integration

### 8.2 Example: Simple Task Delegation

```python
import subprocess
import json

def delegate_to_claude(task: str, context_files: list[str]) -> dict:
    """
    Delegate a task to Claude Code CLI in headless mode.

    Args:
        task: Natural language task description
        context_files: List of file paths for context

    Returns:
        Dict with result, session_id, cost, etc.
    """
    cmd = [
        'claude',
        '-p', task,
        '--output-format', 'json',
        '--model', 'claude-sonnet-4'
    ]

    # Add context files to working directories
    if context_files:
        dirs = list(set(os.path.dirname(f) for f in context_files))
        for dir in dirs:
            cmd.extend(['--add-dir', dir])

    # Execute
    result = subprocess.run(
        cmd,
        capture_output=True,
        text=True,
        timeout=600  # 10 minute timeout
    )

    if result.returncode != 0:
        raise RuntimeError(f"Claude failed: {result.stderr}")

    return json.loads(result.stdout)

# Usage
result = delegate_to_claude(
    task="Review auth.py for security vulnerabilities",
    context_files=["src/auth.py", "tests/test_auth.py"]
)

print(f"Session ID: {result['session_id']}")
print(f"Cost: ${result['total_cost_usd']}")
print(f"Result: {result['result']}")
```

### 8.3 Example: Multi-Turn Orchestration

```python
from claude_code_sdk import ClaudeAgent
import asyncio

class ZenClaude:
    """Wrapper for Claude Agent with Zen-specific patterns."""

    def __init__(self, api_key: str):
        self.agent = ClaudeAgent(api_key=api_key)
        self.session_id = None

    async def start_task(self, task: str) -> str:
        """Start a new task, capturing session ID."""
        async for message in self.agent.query(task):
            if message['type'] == 'system' and message['subtype'] == 'init':
                self.session_id = message['session_id']
            elif message['type'] == 'result':
                return message['result']

    async def continue_task(self, followup: str) -> str:
        """Continue the current task."""
        if not self.session_id:
            raise RuntimeError("No active session")

        async for message in self.agent.query(followup, resume=self.session_id):
            if message['type'] == 'result':
                return message['result']

    async def fork_and_explore(self, alternative: str) -> str:
        """Fork session to explore alternative approach."""
        if not self.session_id:
            raise RuntimeError("No active session")

        async for message in self.agent.query(
            alternative,
            resume=self.session_id,
            fork_session=True
        ):
            if message['type'] == 'result':
                return message['result']

# Usage
async def main():
    zen = ZenClaude(api_key=os.environ['ANTHROPIC_API_KEY'])

    # Start task
    result1 = await zen.start_task("Implement authentication module")
    print("Initial:", result1)

    # Continue with follow-up
    result2 = await zen.continue_task("Now add rate limiting")
    print("Follow-up:", result2)

    # Fork to explore alternative
    result3 = await zen.fork_and_explore("Try JWT instead of sessions")
    print("Alternative:", result3)

asyncio.run(main())
```

### 8.4 Example: Parallel Execution

```python
import asyncio
from claude_code_sdk import ClaudeAgent

async def parallel_delegation(tasks: list[dict]) -> list[dict]:
    """
    Execute multiple Claude tasks in parallel.

    Args:
        tasks: List of dicts with 'prompt' and 'context' keys

    Returns:
        List of results in same order as tasks
    """
    agent = ClaudeAgent(api_key=os.environ['ANTHROPIC_API_KEY'])

    async def execute_task(task: dict) -> dict:
        result = None
        session_id = None
        cost = 0

        async for message in agent.query(task['prompt']):
            if message['type'] == 'system' and message['subtype'] == 'init':
                session_id = message['session_id']
            elif message['type'] == 'result':
                result = message['result']
                cost = message.get('total_cost_usd', 0)

        return {
            'prompt': task['prompt'],
            'result': result,
            'session_id': session_id,
            'cost': cost
        }

    # Execute all tasks in parallel
    results = await asyncio.gather(*[
        execute_task(task) for task in tasks
    ])

    return results

# Usage
tasks = [
    {'prompt': 'Review module A for bugs'},
    {'prompt': 'Write tests for module B'},
    {'prompt': 'Optimize module C for performance'},
]

results = asyncio.run(parallel_delegation(tasks))
for r in results:
    print(f"{r['prompt']}: {r['result'][:100]}... (${r['cost']})")
```

### 8.5 Error Handling and Retry Logic

```python
import time
from typing import Optional

class ClaudeExecutor:
    """Robust Claude execution with retries and error handling."""

    def __init__(self, max_retries: int = 3, backoff: float = 2.0):
        self.max_retries = max_retries
        self.backoff = backoff

    def execute_with_retry(
        self,
        task: str,
        timeout: int = 600
    ) -> Optional[dict]:
        """Execute task with exponential backoff retry."""

        for attempt in range(self.max_retries):
            try:
                result = subprocess.run(
                    ['claude', '-p', task, '--output-format', 'json'],
                    capture_output=True,
                    text=True,
                    timeout=timeout
                )

                if result.returncode == 0:
                    return json.loads(result.stdout)

                # Handle known error codes
                if result.returncode == 1:
                    error_msg = result.stderr

                    # Don't retry on certain errors
                    if 'authentication' in error_msg.lower():
                        raise RuntimeError(f"Auth error: {error_msg}")

                    # Retry on transient errors
                    if attempt < self.max_retries - 1:
                        wait = self.backoff ** attempt
                        print(f"Attempt {attempt + 1} failed, retrying in {wait}s...")
                        time.sleep(wait)
                        continue

                raise RuntimeError(f"Failed after {attempt + 1} attempts: {result.stderr}")

            except subprocess.TimeoutExpired:
                if attempt < self.max_retries - 1:
                    print(f"Timeout on attempt {attempt + 1}, retrying...")
                    continue
                raise RuntimeError(f"Task timed out after {timeout}s")

        return None

# Usage
executor = ClaudeExecutor(max_retries=3, backoff=2.0)
result = executor.execute_with_retry("Complex task")
```

### 8.6 Monitoring and Observability

```python
import logging
from datetime import datetime

class ClaudeMonitor:
    """Monitor Claude executions for observability."""

    def __init__(self, log_file: str = 'claude_executions.log'):
        logging.basicConfig(
            filename=log_file,
            level=logging.INFO,
            format='%(asctime)s - %(levelname)s - %(message)s'
        )
        self.logger = logging.getLogger(__name__)

    def log_execution(self, task: str, result: dict):
        """Log execution details."""
        self.logger.info({
            'timestamp': datetime.now().isoformat(),
            'task': task,
            'session_id': result.get('session_id'),
            'cost_usd': result.get('total_cost_usd'),
            'duration_ms': result.get('duration_ms'),
            'num_turns': result.get('num_turns'),
            'success': result.get('subtype') == 'success'
        })

    def log_error(self, task: str, error: str):
        """Log execution errors."""
        self.logger.error({
            'timestamp': datetime.now().isoformat(),
            'task': task,
            'error': error
        })

    async def execute_and_monitor(self, agent, task: str):
        """Execute task with monitoring."""
        start_time = time.time()

        try:
            async for message in agent.query(task):
                if message['type'] == 'result':
                    duration = time.time() - start_time
                    self.log_execution(task, {
                        **message,
                        'duration_ms': duration * 1000
                    })
                    return message['result']
        except Exception as e:
            self.log_error(task, str(e))
            raise

# Usage
monitor = ClaudeMonitor()
result = await monitor.execute_and_monitor(agent, "Complex task")
```

---

## 9. Best Practices for Zen Integration

### 9.1 Task Delegation

✅ **DO**:
- Use descriptive, specific prompts
- Provide relevant context files via `--add-dir`
- Set appropriate timeouts for long-running tasks
- Capture session IDs for multi-turn workflows
- Use JSON output for programmatic parsing
- Implement retry logic with exponential backoff

❌ **DON'T**:
- Use slash commands in `-p` mode (not available)
- Assume sessions persist without explicit resume
- Ignore exit codes and errors
- Run unbounded loops without circuit breakers
- Skip cost/usage tracking

### 9.2 Resource Management

**Cost Tracking**:
```python
class CostTracker:
    def __init__(self):
        self.total_cost = 0.0
        self.executions = []

    def track(self, result: dict):
        cost = result.get('total_cost_usd', 0)
        self.total_cost += cost
        self.executions.append({
            'session_id': result.get('session_id'),
            'cost': cost,
            'duration': result.get('duration_ms')
        })

    def report(self):
        print(f"Total cost: ${self.total_cost:.4f}")
        print(f"Executions: {len(self.executions)}")
        print(f"Avg cost: ${self.total_cost / len(self.executions):.4f}")
```

**Rate Limiting**:
```python
from time import time, sleep

class RateLimiter:
    def __init__(self, max_calls_per_minute: int = 10):
        self.max_calls = max_calls_per_minute
        self.calls = []

    def wait_if_needed(self):
        now = time()
        # Remove calls older than 1 minute
        self.calls = [c for c in self.calls if now - c < 60]

        if len(self.calls) >= self.max_calls:
            sleep_time = 60 - (now - self.calls[0])
            if sleep_time > 0:
                sleep(sleep_time)
                self.calls = []

        self.calls.append(now)

# Usage
limiter = RateLimiter(max_calls_per_minute=10)
limiter.wait_if_needed()
result = delegate_to_claude(task)
```

### 9.3 Error Recovery

**Circuit Breaker Pattern**:
```python
class CircuitBreaker:
    def __init__(
        self,
        failure_threshold: int = 3,
        timeout: int = 60
    ):
        self.failure_threshold = failure_threshold
        self.timeout = timeout
        self.failures = 0
        self.last_failure_time = None
        self.state = 'closed'  # closed, open, half-open

    def call(self, func, *args, **kwargs):
        if self.state == 'open':
            if time.time() - self.last_failure_time > self.timeout:
                self.state = 'half-open'
            else:
                raise RuntimeError("Circuit breaker is open")

        try:
            result = func(*args, **kwargs)
            self.on_success()
            return result
        except Exception as e:
            self.on_failure()
            raise

    def on_success(self):
        self.failures = 0
        self.state = 'closed'

    def on_failure(self):
        self.failures += 1
        self.last_failure_time = time.time()

        if self.failures >= self.failure_threshold:
            self.state = 'open'

# Usage
breaker = CircuitBreaker(failure_threshold=3, timeout=60)
result = breaker.call(delegate_to_claude, task)
```

### 9.4 Testing Integration

```python
import pytest
from unittest.mock import patch, MagicMock

def test_claude_delegation():
    """Test Claude task delegation."""
    with patch('subprocess.run') as mock_run:
        # Mock successful execution
        mock_run.return_value = MagicMock(
            returncode=0,
            stdout=json.dumps({
                'session_id': 'test-123',
                'result': 'Task completed',
                'subtype': 'success',
                'total_cost_usd': 0.001
            })
        )

        result = delegate_to_claude("Test task", [])

        assert result['session_id'] == 'test-123'
        assert result['subtype'] == 'success'
        mock_run.assert_called_once()

def test_claude_error_handling():
    """Test error handling for failed executions."""
    with patch('subprocess.run') as mock_run:
        # Mock failed execution
        mock_run.return_value = MagicMock(
            returncode=1,
            stderr='Error: something went wrong'
        )

        with pytest.raises(RuntimeError, match="Claude failed"):
            delegate_to_claude("Test task", [])
```

---

## 10. Troubleshooting Guide

### 10.1 Common Issues

**Issue: "Claude Code process exited with code 1"**
- Check API key: `echo $ANTHROPIC_API_KEY`
- Verify installation: `claude --version`
- Try diagnostic: `claude /doctor`
- Check stderr for details

**Issue: Session doesn't resume correctly**
- Verify session ID is captured correctly
- Check that session file exists in `~/.claude/`
- Note: Session ID may change on resume (known bug)
- Use `--add-dir` to ensure correct working directories

**Issue: Output not parsing as JSON**
- Ensure `--output-format json` is specified
- Check for stderr mixed with stdout
- Verify command completed successfully (exit code 0)

**Issue: Task times out**
- Increase timeout parameter
- Check if task is too complex
- Consider breaking into smaller subtasks

**Issue: Hooks blocking execution**
- Check hook exit codes (should be 0 for success)
- Review hook configuration in `.claude/hooks.json`
- Disable problematic hooks temporarily

### 10.2 Debugging Commands

```bash
# Check Claude version
claude --version

# Run diagnostics
claude /doctor

# Test with simple task
claude -p "echo hello" --output-format json

# View recent sessions
ls -la ~/.claude/

# Check session file
cat ~/.claude/sessions/SESSION_ID.jsonl

# Test with verbose output
claude -p "task" --output-format stream-json 2>&1 | tee debug.log
```

### 10.3 Reset and Reinstall

```bash
# Complete reset
npm uninstall -g @anthropic-ai/claude-code
rm -rf ~/.claude.json ~/.claude/
npm cache clean --force
npm install -g @anthropic-ai/claude-code

# Verify
claude --version
```

---

## 11. Additional Resources

### Official Documentation
- [CLI Reference](https://code.claude.com/docs/en/cli-reference)
- [Headless Mode Guide](https://code.claude.com/docs/en/headless)
- [Agent SDK Overview](https://platform.claude.com/docs/en/agent-sdk/overview)
- [TypeScript SDK Reference](https://platform.claude.com/docs/en/agent-sdk/typescript)
- [Python SDK Reference](https://platform.claude.com/docs/en/agent-sdk/python)
- [Session Management](https://platform.claude.com/docs/en/agent-sdk/sessions)
- [Best Practices](https://www.anthropic.com/engineering/claude-code-best-practices)

### Community Resources
- [Shipyard Claude Code Cheatsheet](https://shipyard.build/blog/claude-code-cheat-sheet/)
- [Builder.io Guide](https://www.builder.io/blog/claude-code)
- [Awesome Claude Code](https://github.com/hesreallyhim/awesome-claude-code)
- [Claude Code Cheat Sheet (GitHub)](https://github.com/Njengah/claude-code-cheat-sheet)

### Third-Party Tools
- [Clawd - Autonomous Orchestrator](https://github.com/HMilbradt/clawd)
- [Ralph - Autonomous Development Loop](https://github.com/frankbria/ralph-claude-code)
- [Claude Flow - Non-Interactive Mode](https://github.com/ruvnet/claude-flow)

### Examples and Guides
- [Headless Automation Guide](https://lilys.ai/en/notes/claude-code-20251028/building-headless-automation-claude-code)
- [CI/CD Integration](https://angelo-lima.fr/en/claude-code-cicd-headless-en/)
- [Batch Processing](https://smartscope.blog/en/generative-ai/claude/claude-code-batch-processing/)
- [Tutorial Center](https://www.claudecode101.com/en/tutorial/advanced/headless-mode)

---

## 12. Recommendations for Zen

### Phase 1: CLI Integration (MVP)
1. Start with simple CLI-based delegation (`-p` mode)
2. Use `--output-format json` for structured responses
3. Implement basic error handling and retry logic
4. Track sessions for multi-turn workflows
5. Add cost tracking and monitoring

### Phase 2: Advanced Orchestration
1. Migrate to SDK (TypeScript or Python) for complex workflows
2. Implement parallel execution for independent tasks
3. Add session forking for exploration
4. Build circuit breakers and rate limiting
5. Integrate with Zen's state management

### Phase 3: Production Hardening
1. Add comprehensive error recovery
2. Implement detailed monitoring and observability
3. Add integration tests
4. Document patterns for Zen developers
5. Create reusable templates and patterns

### Key Integration Points
- **Task delegation**: Use CLI for simple tasks, SDK for complex
- **State management**: Leverage session IDs and Zen's state store
- **Error handling**: Combine Claude's exit codes with Zen's error system
- **Monitoring**: Track costs, durations, success rates
- **Parallelization**: Use SDK's async capabilities for parallel work

---

## Sources

- [CLI reference - Claude Code Docs](https://code.claude.com/docs/en/cli-reference)
- [Run Claude Code programmatically - Claude Code Docs](https://code.claude.com/docs/en/headless)
- [Agent SDK overview - Claude API Docs](https://platform.claude.com/docs/en/agent-sdk/overview)
- [Session Management - Claude API Docs](https://platform.claude.com/docs/en/agent-sdk/sessions)
- [Agent SDK reference - TypeScript - Claude API Docs](https://platform.claude.com/docs/en/agent-sdk/typescript)
- [Agent SDK reference - Python - Claude API Docs](https://platform.claude.com/docs/en/agent-sdk/python)
- [Shipyard | Claude Code CLI Cheatsheet](https://shipyard.build/blog/claude-code-cheat-sheet/)
- [Claude Code: Best practices for agentic coding](https://www.anthropic.com/engineering/claude-code-best-practices)
- [Troubleshooting - Claude Code Docs](https://code.claude.com/docs/en/troubleshooting)
- [GitHub - anthropics/claude-code](https://github.com/anthropics/claude-code)
- [GitHub - HMilbradt/clawd](https://github.com/HMilbradt/clawd)
- [GitHub - frankbria/ralph-claude-code](https://github.com/frankbria/ralph-claude-code)
- [Is Claude Code available via API?](https://milvus.io/ai-quick-reference/is-claude-code-available-via-api)
- [ClaudeCode Tutorial Center](https://www.claudecode101.com/en/tutorial/advanced/headless-mode)
- [Building Headless Automation with Claude Code](https://lilys.ai/en/notes/claude-code-20251028/building-headless-automation-claude-code)
