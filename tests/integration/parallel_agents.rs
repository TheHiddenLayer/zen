//! Parallel execution correctness tests.
//!
//! These tests verify that the scheduler correctly handles parallel
//! execution of multiple agents.

use std::collections::HashSet;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};

use zen::agent::AgentId;
use zen::core::dag::{DependencyType, TaskDAG};
use zen::core::task::{Task, TaskId, TaskStatus};
use zen::orchestration::{AgentPool, Scheduler, SchedulerEvent};

use crate::fixtures::{
    chain_dag, diamond_dag, independent_tasks, test_task,
    SchedulerHarness, TestRepo,
};

/// Test: Parallel Execution - 4 independent tasks
/// Given 4 independent tasks
/// When scheduler runs
/// Then 4 agents run concurrently
#[tokio::test]
async fn test_parallel_execution_four_agents() {
    let mut harness = SchedulerHarness::new(4);

    // Add 4 independent tasks
    let tasks = independent_tasks(4);
    for task in &tasks {
        harness.add_task(task.clone()).await;
    }

    // Dispatch should start all 4
    let dispatched = harness.scheduler.dispatch_ready_tasks().await.unwrap();
    assert_eq!(dispatched, 4, "All 4 tasks should be dispatched in parallel");

    // Active count should be 4
    assert_eq!(harness.scheduler.active_count(), 4, "4 agents should be active");

    // Collect all TaskStarted events
    let mut started_count = 0;
    for _ in 0..4 {
        if let Some(SchedulerEvent::TaskStarted { .. }) = harness.event_rx.recv().await {
            started_count += 1;
        }
    }
    assert_eq!(started_count, 4, "Should receive 4 TaskStarted events");
}

/// Test: Parallel execution respects capacity
/// Given 6 tasks and capacity of 3
/// When scheduler dispatches
/// Then only 3 run at a time
#[tokio::test]
async fn test_parallel_respects_capacity() {
    let mut harness = SchedulerHarness::new(3);

    // Add 6 independent tasks
    let tasks = independent_tasks(6);
    for task in &tasks {
        harness.add_task(task.clone()).await;
    }

    // First dispatch: 3 tasks
    let dispatched = harness.scheduler.dispatch_ready_tasks().await.unwrap();
    assert_eq!(dispatched, 3);
    assert_eq!(harness.scheduler.active_count(), 3);

    // Try to dispatch more - should be blocked by capacity
    let dispatched = harness.scheduler.dispatch_ready_tasks().await.unwrap();
    assert_eq!(dispatched, 0, "No more tasks should be dispatched while at capacity");
}

/// Test: Parallel execution releases capacity on completion
/// Given capacity reached and task completes
/// When completion is handled
/// Then capacity is freed for next task
#[tokio::test]
async fn test_parallel_releases_capacity() {
    let mut harness = SchedulerHarness::new(2);

    // Add 4 independent tasks
    let tasks = independent_tasks(4);
    for task in &tasks {
        harness.add_task(task.clone()).await;
    }

    // Dispatch first batch (2 tasks)
    harness.scheduler.dispatch_ready_tasks().await.unwrap();
    assert_eq!(harness.scheduler.active_count(), 2);

    // Get first agent ID
    let agent_id = match harness.event_rx.recv().await {
        Some(SchedulerEvent::TaskStarted { agent_id, .. }) => agent_id,
        _ => panic!("Expected TaskStarted"),
    };
    // Drain second event
    let _ = harness.event_rx.recv().await;

    // Complete first task
    harness
        .scheduler
        .handle_completion(agent_id, "commit".to_string())
        .await
        .unwrap();

    // Active count should drop
    assert_eq!(harness.scheduler.active_count(), 1);

    // Should be able to dispatch another task
    let dispatched = harness.scheduler.dispatch_ready_tasks().await.unwrap();
    assert_eq!(dispatched, 1, "Should dispatch one more task after completion");
}

/// Test: Independent tasks don't block each other
/// Given 3 independent tasks
/// When one fails
/// Then others can still complete
#[tokio::test]
async fn test_independent_tasks_isolation() {
    let mut harness = SchedulerHarness::new(3);

    let task_a = test_task("task-a");
    let task_b = test_task("task-b");
    let task_c = test_task("task-c");
    let id_a = task_a.id;
    let id_b = task_b.id;
    let id_c = task_c.id;

    harness.add_task(task_a).await;
    harness.add_task(task_b).await;
    harness.add_task(task_c).await;

    // Dispatch all
    harness.scheduler.dispatch_ready_tasks().await.unwrap();

    // Collect agents
    let mut agents = Vec::new();
    for _ in 0..3 {
        if let Some(SchedulerEvent::TaskStarted { agent_id, task_id }) = harness.event_rx.recv().await {
            agents.push((agent_id, task_id));
        }
    }

    // Fail first task
    harness
        .scheduler
        .handle_failure(agents[0].0, "Build failed".to_string())
        .await
        .unwrap();

    // Complete other two
    harness
        .scheduler
        .handle_completion(agents[1].0, "commit-b".to_string())
        .await
        .unwrap();
    harness
        .scheduler
        .handle_completion(agents[2].0, "commit-c".to_string())
        .await
        .unwrap();

    // 2 tasks should be completed
    assert_eq!(harness.scheduler.completed_count(), 2);

    // The failed task should not be in completed set
    let completed = harness.scheduler.completed();
    assert!(!completed.contains(&agents[0].1));
}

/// Test: Parallel execution with partial dependencies
/// Given A, B independent, C depends on A
/// When A and B start
/// Then C waits, B can complete independently
#[tokio::test]
async fn test_parallel_partial_dependencies() {
    let mut harness = SchedulerHarness::new(3);

    let task_a = test_task("task-a");
    let task_b = test_task("task-b");
    let task_c = test_task("task-c"); // Depends on A

    let id_a = task_a.id;
    let id_b = task_b.id;
    let id_c = task_c.id;

    harness.add_task(task_a).await;
    harness.add_task(task_b).await;
    harness.add_task(task_c).await;
    harness.add_dependency(&id_a, &id_c).await.unwrap();

    // Initially A and B are ready (C depends on A)
    let ready = harness.scheduler.get_ready_tasks().await;
    assert_eq!(ready.len(), 2);
    assert!(ready.contains(&id_a));
    assert!(ready.contains(&id_b));
    assert!(!ready.contains(&id_c));

    // Dispatch A and B
    harness.scheduler.dispatch_ready_tasks().await.unwrap();

    // Get agents
    let mut agents = Vec::new();
    for _ in 0..2 {
        if let Some(SchedulerEvent::TaskStarted { agent_id, task_id }) = harness.event_rx.recv().await {
            agents.push((agent_id, task_id));
        }
    }

    // Complete B first
    let b_agent = agents.iter().find(|(_, tid)| *tid == id_b).unwrap().0;
    harness
        .scheduler
        .handle_completion(b_agent, "commit-b".to_string())
        .await
        .unwrap();

    // C still not ready (waiting on A)
    let ready = harness.scheduler.get_ready_tasks().await;
    assert!(!ready.contains(&id_c));

    // Complete A
    let a_agent = agents.iter().find(|(_, tid)| *tid == id_a).unwrap().0;
    harness
        .scheduler
        .handle_completion(a_agent, "commit-a".to_string())
        .await
        .unwrap();

    // Now C is ready
    let ready = harness.scheduler.get_ready_tasks().await;
    assert!(ready.contains(&id_c));
}

/// Test: Multiple completion waves
/// Given chain A -> B -> C with capacity 1
/// When tasks complete sequentially
/// Then each completion triggers next task
#[tokio::test]
async fn test_completion_waves() {
    let mut harness = SchedulerHarness::new(1);

    let (dag, id_a, id_b, id_c) = chain_dag();
    *harness.dag.write().await = dag;

    // Wave 1: A
    harness.scheduler.dispatch_ready_tasks().await.unwrap();
    let agent_a = match harness.event_rx.recv().await {
        Some(SchedulerEvent::TaskStarted { agent_id, task_id }) => {
            assert_eq!(task_id, id_a);
            agent_id
        }
        _ => panic!("Expected TaskStarted for A"),
    };

    harness
        .scheduler
        .handle_completion(agent_a, "commit-a".to_string())
        .await
        .unwrap();

    // Wave 2: B becomes ready
    let ready = harness.scheduler.get_ready_tasks().await;
    assert_eq!(ready.len(), 1);
    assert!(ready.contains(&id_b));

    harness.scheduler.dispatch_ready_tasks().await.unwrap();

    // Drain events until we find TaskStarted for B
    let agent_b = loop {
        match harness.event_rx.recv().await {
            Some(SchedulerEvent::TaskStarted { agent_id, .. }) => break agent_id,
            Some(_) => continue, // Skip TaskCompleted events from A
            None => panic!("Expected TaskStarted for B"),
        }
    };

    harness
        .scheduler
        .handle_completion(agent_b, "commit-b".to_string())
        .await
        .unwrap();

    // Wave 3: C becomes ready
    let ready = harness.scheduler.get_ready_tasks().await;
    assert_eq!(ready.len(), 1);
    assert!(ready.contains(&id_c));

    harness.scheduler.dispatch_ready_tasks().await.unwrap();

    // Drain events until we find TaskStarted for C
    let agent_c = loop {
        match harness.event_rx.recv().await {
            Some(SchedulerEvent::TaskStarted { agent_id, .. }) => break agent_id,
            Some(_) => continue, // Skip TaskCompleted events from B
            None => panic!("Expected TaskStarted for C"),
        }
    };

    harness
        .scheduler
        .handle_completion(agent_c, "commit-c".to_string())
        .await
        .unwrap();

    // All complete
    assert!(harness.scheduler.all_complete().await);
}

/// Test: Parallel diamond pattern
/// Given diamond A, B -> C with capacity 2
/// When A and B run in parallel
/// Then C starts only after both complete
#[tokio::test]
async fn test_parallel_diamond_pattern() {
    let mut harness = SchedulerHarness::new(2);

    let (dag, id_a, id_b, id_c) = diamond_dag();
    *harness.dag.write().await = dag;

    // Dispatch A and B in parallel
    let dispatched = harness.scheduler.dispatch_ready_tasks().await.unwrap();
    assert_eq!(dispatched, 2, "A and B should start in parallel");

    // Get both agents
    let mut agents = Vec::new();
    for _ in 0..2 {
        if let Some(SchedulerEvent::TaskStarted { agent_id, task_id }) = harness.event_rx.recv().await {
            agents.push((agent_id, task_id));
        }
    }

    // C not ready
    let ready = harness.scheduler.get_ready_tasks().await;
    assert!(ready.is_empty());

    // Complete A
    let (agent_a, _) = agents.iter().find(|(_, tid)| *tid == id_a).copied().unwrap();
    harness
        .scheduler
        .handle_completion(agent_a, "commit-a".to_string())
        .await
        .unwrap();

    // C still not ready (needs B)
    let ready = harness.scheduler.get_ready_tasks().await;
    assert!(ready.is_empty());

    // Complete B
    let (agent_b, _) = agents.iter().find(|(_, tid)| *tid == id_b).copied().unwrap();
    harness
        .scheduler
        .handle_completion(agent_b, "commit-b".to_string())
        .await
        .unwrap();

    // Now C is ready
    let ready = harness.scheduler.get_ready_tasks().await;
    assert_eq!(ready.len(), 1);
    assert!(ready.contains(&id_c));
}

/// Test: Maximum parallelism utilization
/// Given many tasks and high capacity
/// When tasks are independent
/// Then scheduler uses maximum parallelism
#[tokio::test]
async fn test_maximum_parallelism() {
    let capacity = 8;
    let mut harness = SchedulerHarness::new(capacity);

    // Add 10 independent tasks
    let tasks = independent_tasks(10);
    for task in &tasks {
        harness.add_task(task.clone()).await;
    }

    // Should dispatch up to capacity
    let dispatched = harness.scheduler.dispatch_ready_tasks().await.unwrap();
    assert_eq!(dispatched, capacity);

    // Remaining tasks should be in pending
    let ready = harness.scheduler.get_ready_tasks().await;
    assert_eq!(ready.len(), 2); // 10 - 8 = 2 remaining ready but not dispatched
}

/// Test: Agent tracking consistency
/// Given multiple agents
/// When tracking their tasks
/// Then mapping is accurate
#[tokio::test]
async fn test_agent_task_mapping() {
    let mut harness = SchedulerHarness::new(3);

    let tasks = independent_tasks(3);
    let ids: Vec<_> = tasks.iter().map(|t| t.id).collect();
    for task in tasks {
        harness.add_task(task).await;
    }

    harness.scheduler.dispatch_ready_tasks().await.unwrap();

    // Collect agent-task mappings
    let mut mapping = Vec::new();
    for _ in 0..3 {
        if let Some(SchedulerEvent::TaskStarted { agent_id, task_id }) = harness.event_rx.recv().await {
            mapping.push((agent_id, task_id));
        }
    }

    // All 3 tasks should be assigned to different agents
    let unique_agents: HashSet<_> = mapping.iter().map(|(a, _)| *a).collect();
    let unique_tasks: HashSet<_> = mapping.iter().map(|(_, t)| *t).collect();

    assert_eq!(unique_agents.len(), 3, "Each task should have a unique agent");
    assert_eq!(unique_tasks.len(), 3, "All 3 tasks should be started");

    // Verify task IDs match what we added
    for (_, tid) in &mapping {
        assert!(ids.contains(tid));
    }
}

/// Test: Concurrent event emission
/// Given multiple concurrent operations
/// When events are emitted
/// Then all events are captured
#[tokio::test]
async fn test_concurrent_event_emission() {
    let mut harness = SchedulerHarness::new(5);

    // Add 5 tasks
    let tasks = independent_tasks(5);
    for task in tasks {
        harness.add_task(task).await;
    }

    // Dispatch all
    harness.scheduler.dispatch_ready_tasks().await.unwrap();

    // Collect all start events
    let mut events = Vec::new();
    for _ in 0..5 {
        events.push(harness.event_rx.recv().await.unwrap());
    }

    // All should be TaskStarted
    for event in &events {
        assert!(matches!(event, SchedulerEvent::TaskStarted { .. }));
    }

    // Complete all in rapid succession
    let agent_ids: Vec<_> = events
        .iter()
        .filter_map(|e| match e {
            SchedulerEvent::TaskStarted { agent_id, .. } => Some(*agent_id),
            _ => None,
        })
        .collect();

    for (i, agent_id) in agent_ids.iter().enumerate() {
        harness
            .scheduler
            .handle_completion(*agent_id, format!("commit-{}", i))
            .await
            .unwrap();
    }

    // Should have received 5 TaskCompleted events + progress events + AllTasksComplete
    let mut completed_count = 0;
    let mut all_complete_received = false;
    while let Some(event) = harness.next_event_timeout(100).await {
        match event {
            SchedulerEvent::TaskCompleted { .. } => completed_count += 1,
            SchedulerEvent::AllTasksComplete => all_complete_received = true,
            _ => {}
        }
    }

    assert_eq!(completed_count, 5);
    assert!(all_complete_received);
}
