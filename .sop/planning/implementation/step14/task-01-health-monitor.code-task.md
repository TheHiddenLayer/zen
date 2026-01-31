# Task: Create Health Monitor

## Description
Create the HealthMonitor struct that detects stuck or failing agents by monitoring activity timestamps and output patterns.

## Background
Agents can get stuck in loops, hit rate limits, or fail silently. The HealthMonitor watches for these conditions and triggers recovery actions. This is essential for autonomous operation.

## Reference Documentation
**Required:**
- Design: .sop/planning/design/detailed-design.md (Section 4.4 Health Monitor)

**Note:** You MUST read the detailed design document before beginning implementation.

## Technical Requirements
1. Create `src/orchestration/health.rs` with:
   ```rust
   pub struct HealthMonitor {
       config: HealthConfig,
       agent_pool: Arc<RwLock<AgentPool>>,
       event_tx: mpsc::Sender<HealthEvent>,
   }

   pub struct HealthConfig {
       pub stuck_threshold: Duration,      // 5 minutes default
       pub max_retries: u32,               // 3 default
       pub stuck_patterns: Vec<String>,    // error patterns
   }

   pub enum HealthEvent {
       AgentStuck { agent_id: AgentId, duration: Duration },
       AgentFailed { agent_id: AgentId, error: String },
       RecoveryTriggered { agent_id: AgentId, action: RecoveryAction },
   }
   ```
2. Implement monitoring:
   - `check_all(&self) -> Vec<HealthEvent>`
   - `check_agent(&self, agent: &AgentHandle) -> Option<HealthEvent>`
3. Pattern matching for stuck detection

## Dependencies
- AgentPool from Step 4
- AgentHandle for activity timestamps

## Implementation Approach
1. Define HealthConfig with defaults
2. Define HealthEvent enum
3. Create HealthMonitor struct
4. Implement check_agent() examining last_activity
5. Implement pattern matching for error detection
6. Implement check_all() iterating all agents
7. Add tests for stuck and error detection

## Acceptance Criteria

1. **Stuck Detection**
   - Given agent with no activity for 5 minutes
   - When check_agent() is called
   - Then AgentStuck event is returned

2. **Pattern Detection**
   - Given agent output containing "rate limit"
   - When check_agent() examines output
   - Then stuck pattern is detected

3. **Healthy Agent**
   - Given agent with recent activity
   - When check_agent() is called
   - Then None is returned (no issues)

4. **Check All**
   - Given 3 agents, 1 stuck
   - When check_all() is called
   - Then 1 HealthEvent is returned

5. **Configurable Threshold**
   - Given config with stuck_threshold = 10 minutes
   - When checking agents
   - Then 10 minute threshold is used

## Metadata
- **Complexity**: Medium
- **Labels**: Health, Monitoring, Recovery, Agent
- **Required Skills**: Rust, async, pattern matching
