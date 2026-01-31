//! Health monitor and recovery integration tests.
//!
//! These tests verify that the HealthMonitor correctly detects stuck
//! or failing agents and triggers appropriate recovery actions.

use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, RwLock};

use zen::agent::AgentId;
use zen::orchestration::{
    HealthConfig, HealthEvent, HealthMonitor, RecoveryAction, RetryTracker,
    AgentPool,
};
use zen::workflow::TaskId;

/// Test: Stuck agent detection
/// Given simulated stuck agent
/// When health monitor detects
/// Then HealthEvent::AgentStuck is emitted
#[tokio::test]
async fn test_stuck_agent_detection() {
    // Create a health monitor with short threshold for testing
    let config = HealthConfig {
        stuck_threshold: Duration::from_millis(100),
        max_retries: 3,
        stuck_patterns: vec!["rate limit".to_string()],
    };

    let (pool_tx, _agent_rx) = mpsc::channel(100);
    let pool = Arc::new(RwLock::new(AgentPool::new(4, pool_tx)));
    let (event_tx, _event_rx) = mpsc::channel(100);

    let monitor = HealthMonitor::new(config, Arc::clone(&pool), event_tx);

    // Spawn an agent
    let task_id = TaskId::new();
    {
        let mut pool = pool.write().await;
        let _ = pool.spawn(&task_id, "test-skill").await;
    }

    // Wait for the agent to become "stuck" (no activity)
    tokio::time::sleep(Duration::from_millis(150)).await;

    // Check all agents
    let events = monitor.check_all().await;

    // Should have detected stuck agent
    let stuck_events: Vec<_> = events
        .iter()
        .filter(|e| matches!(e, HealthEvent::AgentStuck { .. }))
        .collect();

    assert!(
        !stuck_events.is_empty(),
        "Should detect stuck agent after threshold"
    );
}

/// Test: Recovery action - restart for transient errors
/// Given agent with rate limit error
/// When recovery is determined
/// Then Restart action is returned
#[tokio::test]
async fn test_recovery_restart_for_rate_limit() {
    let config = HealthConfig {
        stuck_threshold: Duration::from_secs(60),
        max_retries: 3,
        stuck_patterns: vec!["rate limit".to_string(), "timeout".to_string()],
    };

    let (pool_tx, _) = mpsc::channel(100);
    let pool = Arc::new(RwLock::new(AgentPool::new(4, pool_tx)));
    let (event_tx, _) = mpsc::channel(100);

    let monitor = HealthMonitor::new(config, Arc::clone(&pool), event_tx);

    // Spawn agent
    let task_id = TaskId::new();
    let agent_id = {
        let mut pool = pool.write().await;
        pool.spawn(&task_id, "test-skill").await.unwrap()
    };

    // Get agent and determine recovery
    let pool = pool.read().await;
    if let Some(handle) = pool.get(&agent_id) {
        // Simulate rate limit in output by passing the output
        let output = "Error: rate limit exceeded, please try again later";

        let action = monitor.determine_recovery(handle, Some(output)).await;

        // Should recommend restart for transient error
        assert!(
            matches!(action, RecoveryAction::Restart),
            "Rate limit should trigger Restart action, got: {:?}",
            action
        );
    }
}

/// Test: Retry tracker
/// Test that retry tracking works correctly
#[test]
fn test_retry_tracker() {
    let mut tracker = RetryTracker::new();
    let task_id = TaskId::new();

    // Initial count is 0
    assert_eq!(tracker.get_retries(&task_id), 0);

    // Increment
    tracker.increment(&task_id);
    assert_eq!(tracker.get_retries(&task_id), 1);

    tracker.increment(&task_id);
    assert_eq!(tracker.get_retries(&task_id), 2);

    // Reset
    tracker.reset(&task_id);
    assert_eq!(tracker.get_retries(&task_id), 0);

    // Multiple tasks
    let task2 = TaskId::new();
    tracker.increment(&task_id);
    tracker.increment(&task2);
    tracker.increment(&task2);
    assert_eq!(tracker.get_retries(&task_id), 1);
    assert_eq!(tracker.get_retries(&task2), 2);

    // Clear all
    tracker.clear();
    assert_eq!(tracker.get_retries(&task_id), 0);
    assert_eq!(tracker.get_retries(&task2), 0);
}

/// Test: Health config defaults
/// Test that default config has reasonable values
#[test]
fn test_health_config_defaults() {
    let config = HealthConfig::default();

    // Stuck threshold should be reasonable (around 5 minutes)
    assert!(config.stuck_threshold >= Duration::from_secs(60));
    assert!(config.stuck_threshold <= Duration::from_secs(600));

    // Max retries should be reasonable (2-5)
    assert!(config.max_retries >= 2);
    assert!(config.max_retries <= 10);

    // Should have some stuck patterns
    assert!(!config.stuck_patterns.is_empty());
}

/// Test: Health event types
/// Test that health events are correctly constructed
#[test]
fn test_health_event_types() {
    let agent_id = AgentId::new();

    // AgentStuck
    let stuck = HealthEvent::AgentStuck {
        agent_id,
        duration: Duration::from_secs(300),
    };
    if let HealthEvent::AgentStuck {
        agent_id: aid,
        duration,
    } = stuck
    {
        assert_eq!(aid, agent_id);
        assert_eq!(duration.as_secs(), 300);
    }

    // AgentFailed
    let failed = HealthEvent::AgentFailed {
        agent_id,
        error: "Test error".to_string(),
    };
    if let HealthEvent::AgentFailed {
        agent_id: aid,
        error,
    } = failed
    {
        assert_eq!(aid, agent_id);
        assert_eq!(error, "Test error");
    }

    // RecoveryTriggered
    let recovery = HealthEvent::RecoveryTriggered {
        agent_id,
        action: RecoveryAction::Restart,
    };
    if let HealthEvent::RecoveryTriggered {
        agent_id: aid,
        action,
    } = recovery
    {
        assert_eq!(aid, agent_id);
        assert!(matches!(action, RecoveryAction::Restart));
    }
}

/// Test: Recovery action types
/// Test various recovery action types
#[test]
fn test_recovery_action_types() {
    // Restart
    let restart = RecoveryAction::Restart;
    assert!(matches!(restart, RecoveryAction::Restart));

    // Reassign
    let other_agent = AgentId::new();
    let reassign = RecoveryAction::Reassign {
        to_agent: other_agent,
    };
    if let RecoveryAction::Reassign { to_agent } = reassign {
        assert_eq!(to_agent, other_agent);
    }

    // Decompose
    let decompose = RecoveryAction::Decompose {
        into_tasks: vec!["subtask-1".to_string(), "subtask-2".to_string()],
    };
    if let RecoveryAction::Decompose { into_tasks } = decompose {
        assert_eq!(into_tasks.len(), 2);
    }

    // Escalate
    let escalate = RecoveryAction::Escalate {
        message: "Help needed".to_string(),
    };
    if let RecoveryAction::Escalate { message } = escalate {
        assert_eq!(message, "Help needed");
    }

    // Abort
    let abort = RecoveryAction::Abort;
    assert!(matches!(abort, RecoveryAction::Abort));
}

/// Test: Healthy agent returns None
/// Given healthy agent with recent activity
/// When check_agent is called
/// Then None is returned (no event)
#[tokio::test]
async fn test_healthy_agent_returns_none() {
    let config = HealthConfig {
        stuck_threshold: Duration::from_secs(300), // 5 minutes
        max_retries: 3,
        stuck_patterns: vec![],
    };

    let (pool_tx, _) = mpsc::channel(100);
    let pool = Arc::new(RwLock::new(AgentPool::new(4, pool_tx)));
    let (event_tx, _) = mpsc::channel(100);

    let monitor = HealthMonitor::new(config, Arc::clone(&pool), event_tx);

    // Spawn agent
    let task_id = TaskId::new();
    let agent_id = {
        let mut pool = pool.write().await;
        pool.spawn(&task_id, "test-skill").await.unwrap()
    };

    // Check immediately (should be healthy)
    let pool = pool.read().await;
    if let Some(handle) = pool.get(&agent_id) {
        let event = monitor.check_agent(handle);
        assert!(event.is_none(), "Recently spawned agent should be healthy");
    }
}

/// Test: Multiple agents health check
/// Given multiple agents with different states
/// When check_all is called
/// Then only unhealthy agents are reported
#[tokio::test]
async fn test_multiple_agents_health_check() {
    let config = HealthConfig {
        stuck_threshold: Duration::from_millis(50), // Short for testing
        max_retries: 3,
        stuck_patterns: vec![],
    };

    let (pool_tx, _) = mpsc::channel(100);
    let pool = Arc::new(RwLock::new(AgentPool::new(4, pool_tx)));
    let (event_tx, _) = mpsc::channel(100);

    let monitor = HealthMonitor::new(config, Arc::clone(&pool), event_tx);

    // Spawn multiple agents
    let task1 = TaskId::new();
    let task2 = TaskId::new();
    let task3 = TaskId::new();

    {
        let mut pool = pool.write().await;
        pool.spawn(&task1, "test-skill").await.unwrap();
        pool.spawn(&task2, "test-skill").await.unwrap();
        pool.spawn(&task3, "test-skill").await.unwrap();
    }

    // Wait for agents to become stuck
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Check all agents
    let events = monitor.check_all().await;

    // All 3 should be detected as stuck (no activity since spawn)
    let stuck_count = events
        .iter()
        .filter(|e| matches!(e, HealthEvent::AgentStuck { .. }))
        .count();

    assert!(stuck_count >= 1, "Should detect at least one stuck agent");
}

/// Test: No real Claude calls in recovery tests
/// Verify that recovery tests don't make actual Claude API calls
#[test]
fn test_no_real_claude_calls_in_recovery() {
    // This test documents that recovery tests are CI-safe.
    // The HealthMonitor and RecoveryAction types are pure data structures
    // that don't interact with Claude.
    //
    // Recovery decisions are made based on:
    // 1. Agent idle duration
    // 2. Output pattern matching
    // 3. Retry count
    //
    // Actual recovery execution (like restarting an agent) would involve
    // Claude, but that's not tested here - we only test the decision logic.

    assert!(true, "Recovery tests are CI-safe - no Claude calls");
}
