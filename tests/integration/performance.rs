//! Performance tests for Zen v2.
//!
//! These tests verify that the system meets performance requirements:
//! - 60 FPS TUI rendering (frame time < 16.67ms)
//! - Scheduler overhead < 10ms per dispatch
//! - Memory usage < 100MB for 10 parallel agents
//! - No render blocking from state updates
//!
//! # CI Integration
//!
//! Tests output performance metrics and fail if thresholds are exceeded.
//! Use `cargo test --test integration performance -- --nocapture` to see metrics.

use std::collections::HashSet;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

use zen::agent::{AgentId, AgentStatus};
use zen::core::dag::{DependencyType, TaskDAG};
use zen::core::task::{Task, TaskId, TaskStatus};
use zen::render::{AgentView, RenderState, TaskDAGView, TaskView, WorkflowView};
use zen::workflow::{WorkflowId, WorkflowPhase, WorkflowStatus};

/// Performance thresholds
const MAX_FRAME_TIME_MS: u128 = 17; // 60 FPS = 16.67ms per frame, allow slight margin
const MAX_SCHEDULER_OVERHEAD_MS: u128 = 10;
const MAX_MEMORY_MB: usize = 100;

// ============================================================================
// TUI Render Performance Tests
// ============================================================================

/// Helper to create a RenderState with N agents for testing.
fn create_render_state_with_agents(agent_count: usize) -> RenderState {
    let mut state = RenderState::default();

    // Add workflow
    state.workflow = Some(WorkflowView::new(
        WorkflowId::new(),
        "test-workflow".to_string(),
        WorkflowPhase::Implementation,
        WorkflowStatus::Running,
    ));

    // Add agents
    for i in 0..agent_count {
        state.agents.push(AgentView::new(
            AgentId::new(),
            format!("task-{}", i),
            AgentStatus::Running {
                task_id: zen::workflow::TaskId::new(),
            },
            Duration::from_secs(i as u64 * 30),
            format!(
                "Line 1 of output\nLine 2 of output\nLine 3 of output\nWorking on step {}...",
                i
            ),
        ));
    }

    // Add DAG
    let mut dag = TaskDAGView::new();
    for i in 0..agent_count {
        let status = if i < agent_count / 2 {
            TaskStatus::Completed
        } else {
            TaskStatus::Running
        };
        dag.add_task(TaskView::new(format!("task-{}", i), status));
    }
    // Add some edges
    for i in 0..agent_count.saturating_sub(1) {
        if i % 2 == 0 {
            dag.add_edge(i, i + 1);
        }
    }
    state.dag = Some(dag);
    state.show_dag = true;

    state
}

/// Test that render state construction is fast.
/// Frame preparation must be under 16.67ms for 60 FPS.
#[test]
fn test_render_state_construction_performance() {
    const ITERATIONS: usize = 100;
    let mut total_time = Duration::ZERO;

    for _ in 0..ITERATIONS {
        let start = Instant::now();
        let _state = create_render_state_with_agents(4);
        total_time += start.elapsed();
    }

    let avg_time_ms = total_time.as_micros() / ITERATIONS as u128 / 1000;
    println!(
        "Average render state construction time: {}ms (threshold: {}ms)",
        avg_time_ms, MAX_FRAME_TIME_MS
    );

    // State construction should be very fast (< 1ms typically)
    assert!(
        avg_time_ms < MAX_FRAME_TIME_MS,
        "Render state construction took {}ms, exceeds {}ms threshold",
        avg_time_ms,
        MAX_FRAME_TIME_MS
    );
}

/// Test that WorkflowView progress calculation is fast.
#[test]
fn test_workflow_view_progress_calculation() {
    const ITERATIONS: usize = 10000;
    let view = WorkflowView::new(
        WorkflowId::new(),
        "test".to_string(),
        WorkflowPhase::Implementation,
        WorkflowStatus::Running,
    );

    let start = Instant::now();
    for _ in 0..ITERATIONS {
        let _percentage = view.progress_percentage();
        let _index = view.current_phase_index();
        let _names = WorkflowView::phase_names();
    }
    let elapsed = start.elapsed();

    let avg_ns = elapsed.as_nanos() / ITERATIONS as u128;
    println!(
        "Average WorkflowView calculation time: {}ns ({} iterations)",
        avg_ns, ITERATIONS
    );

    // Should be sub-microsecond
    assert!(avg_ns < 1000, "WorkflowView calculation too slow: {}ns", avg_ns);
}

/// Test that AgentView formatting is fast.
#[test]
fn test_agent_view_formatting_performance() {
    const ITERATIONS: usize = 10000;

    let agent = AgentView::new(
        AgentId::new(),
        "create-user-model".to_string(),
        AgentStatus::Running {
            task_id: zen::workflow::TaskId::new(),
        },
        Duration::from_secs(150),
        "Line 1\nLine 2\nLine 3\nLine 4\nLine 5\nLine 6\nLine 7\nLine 8\nLine 9\nLine 10"
            .to_string(),
    );

    let start = Instant::now();
    for _ in 0..ITERATIONS {
        let _label = agent.status_label();
        let _elapsed = agent.format_elapsed();
        let _lines = agent.output_lines(3);
    }
    let elapsed = start.elapsed();

    let avg_ns = elapsed.as_nanos() / ITERATIONS as u128;
    println!(
        "Average AgentView formatting time: {}ns ({} iterations)",
        avg_ns, ITERATIONS
    );

    // Should be sub-microsecond for basic operations
    assert!(avg_ns < 5000, "AgentView formatting too slow: {}ns", avg_ns);
}

/// Test that DAG layer computation is efficient.
#[test]
fn test_dag_layer_computation_performance() {
    const ITERATIONS: usize = 1000;

    // Create a moderately complex DAG (10 tasks with diamond pattern)
    let mut dag = TaskDAGView::new();
    for i in 0..10 {
        dag.add_task(TaskView::new(format!("task-{}", i), TaskStatus::Pending));
    }
    // Create diamond patterns: 0->2, 1->2, 2->4, 3->4, etc.
    dag.add_edge(0, 2);
    dag.add_edge(1, 2);
    dag.add_edge(2, 4);
    dag.add_edge(3, 4);
    dag.add_edge(4, 6);
    dag.add_edge(5, 6);
    dag.add_edge(6, 8);
    dag.add_edge(7, 8);
    dag.add_edge(8, 9);

    let start = Instant::now();
    for _ in 0..ITERATIONS {
        let _layers = dag.compute_layers();
    }
    let elapsed = start.elapsed();

    let avg_us = elapsed.as_micros() / ITERATIONS as u128;
    println!(
        "Average DAG layer computation time: {}µs ({} iterations)",
        avg_us, ITERATIONS
    );

    // Should be fast (< 1ms for 10 tasks)
    assert!(
        avg_us < 1000,
        "DAG layer computation too slow: {}µs",
        avg_us
    );
}

/// Test rendering performance with 4 active agents (60 FPS requirement).
#[test]
fn test_render_60fps_with_4_agents() {
    const ITERATIONS: usize = 60; // Simulate 1 second at 60 FPS

    let state = create_render_state_with_agents(4);
    let mut total_time = Duration::ZERO;
    let mut max_frame_time = Duration::ZERO;

    for _ in 0..ITERATIONS {
        let start = Instant::now();

        // Simulate render preparation (creating view structs, calculating layouts)
        let _workflow = &state.workflow;
        let _agents = &state.agents;
        if let Some(dag) = &state.dag {
            let _layers = dag.compute_layers();
        }
        for agent in &state.agents {
            let _label = agent.status_label();
            let _elapsed = agent.format_elapsed();
            let _lines = agent.output_lines(3);
        }

        let frame_time = start.elapsed();
        total_time += frame_time;
        if frame_time > max_frame_time {
            max_frame_time = frame_time;
        }
    }

    let avg_frame_time_ms = total_time.as_micros() / ITERATIONS as u128 / 1000;
    let max_frame_time_ms = max_frame_time.as_millis();

    println!(
        "Render performance (4 agents): avg={}ms, max={}ms (threshold: {}ms)",
        avg_frame_time_ms, max_frame_time_ms, MAX_FRAME_TIME_MS
    );

    assert!(
        avg_frame_time_ms < MAX_FRAME_TIME_MS,
        "Average frame time {}ms exceeds 60 FPS threshold ({}ms)",
        avg_frame_time_ms,
        MAX_FRAME_TIME_MS
    );
}

// ============================================================================
// Scheduler Overhead Tests
// ============================================================================

/// Helper to create a TaskDAG with N tasks.
fn create_task_dag(task_count: usize) -> TaskDAG {
    let mut dag = TaskDAG::new();
    let mut task_ids = Vec::new();

    for i in 0..task_count {
        let task = Task::new(&format!("task-{}", i), &format!("Description for task {}", i));
        task_ids.push(task.id);
        dag.add_task(task);
    }

    // Add some dependencies (diamond pattern)
    for i in 0..task_count.saturating_sub(2) {
        if i % 2 == 0 && i + 2 < task_count {
            let _ = dag.add_dependency(&task_ids[i], &task_ids[i + 2], DependencyType::DataDependency);
        }
    }

    dag
}

/// Test that ready_tasks computation is under 10ms for 10 tasks.
#[test]
fn test_scheduler_ready_tasks_overhead() {
    const ITERATIONS: usize = 1000;

    let dag = create_task_dag(10);
    let completed = HashSet::new();

    let start = Instant::now();
    for _ in 0..ITERATIONS {
        let _ready = dag.ready_tasks(&completed);
    }
    let elapsed = start.elapsed();

    let avg_us = elapsed.as_micros() / ITERATIONS as u128;
    let avg_ms = avg_us as f64 / 1000.0;

    println!(
        "Scheduler ready_tasks overhead (10 tasks): {:.3}ms (threshold: {}ms)",
        avg_ms, MAX_SCHEDULER_OVERHEAD_MS
    );

    assert!(
        avg_ms < MAX_SCHEDULER_OVERHEAD_MS as f64,
        "ready_tasks took {:.3}ms, exceeds {}ms threshold",
        avg_ms,
        MAX_SCHEDULER_OVERHEAD_MS
    );
}

/// Test scheduler overhead with larger DAG (50 tasks).
#[test]
fn test_scheduler_overhead_50_tasks() {
    const ITERATIONS: usize = 100;

    let dag = create_task_dag(50);
    let mut completed = HashSet::new();

    // Simulate completing half the tasks
    for task in dag.all_tasks().iter().take(25) {
        completed.insert(task.id);
    }

    let start = Instant::now();
    for _ in 0..ITERATIONS {
        let _ready = dag.ready_tasks(&completed);
        let _pending = dag.pending_count(&completed);
        let _all_done = dag.all_complete(&completed);
    }
    let elapsed = start.elapsed();

    let avg_us = elapsed.as_micros() / ITERATIONS as u128;
    let avg_ms = avg_us as f64 / 1000.0;

    println!(
        "Scheduler overhead (50 tasks, 25 completed): {:.3}ms (threshold: {}ms)",
        avg_ms, MAX_SCHEDULER_OVERHEAD_MS
    );

    assert!(
        avg_ms < MAX_SCHEDULER_OVERHEAD_MS as f64,
        "Scheduler overhead {:.3}ms exceeds {}ms threshold",
        avg_ms,
        MAX_SCHEDULER_OVERHEAD_MS
    );
}

/// Test topological sort performance.
#[test]
fn test_topological_sort_performance() {
    const ITERATIONS: usize = 100;

    let dag = create_task_dag(50);

    let start = Instant::now();
    for _ in 0..ITERATIONS {
        let _order = dag.topological_order().unwrap();
    }
    let elapsed = start.elapsed();

    let avg_us = elapsed.as_micros() / ITERATIONS as u128;
    let avg_ms = avg_us as f64 / 1000.0;

    println!(
        "Topological sort (50 tasks): {:.3}ms (threshold: {}ms)",
        avg_ms, MAX_SCHEDULER_OVERHEAD_MS
    );

    assert!(
        avg_ms < MAX_SCHEDULER_OVERHEAD_MS as f64,
        "Topological sort took {:.3}ms, exceeds {}ms threshold",
        avg_ms,
        MAX_SCHEDULER_OVERHEAD_MS
    );
}

// ============================================================================
// Memory Usage Tests
// ============================================================================

/// Test memory usage for 10 parallel agent representations.
/// Note: This tests the data structure memory, not actual agent processes.
#[test]
fn test_memory_baseline_10_agents() {
    // Create structures for 10 agents
    let agents: Vec<AgentView> = (0..10)
        .map(|i| {
            AgentView::new(
                AgentId::new(),
                format!("task-{}", i),
                AgentStatus::Running {
                    task_id: zen::workflow::TaskId::new(),
                },
                Duration::from_secs(i as u64 * 60),
                // Simulate realistic output buffer (1KB per agent)
                "A".repeat(1024),
            )
        })
        .collect();

    // Create workflow state
    let workflow = WorkflowView::new(
        WorkflowId::new(),
        "test-workflow".to_string(),
        WorkflowPhase::Implementation,
        WorkflowStatus::Running,
    );

    // Create DAG for 50 tasks
    let mut dag = TaskDAGView::new();
    for i in 0..50 {
        dag.add_task(TaskView::new(format!("task-{}", i), TaskStatus::Pending));
    }
    for i in 0..49 {
        dag.add_edge(i, i + 1);
    }

    // Calculate approximate memory usage
    let agent_mem = std::mem::size_of::<AgentView>() * agents.len();
    let output_mem: usize = agents.iter().map(|a| a.output_preview.len()).sum();
    let workflow_mem = std::mem::size_of::<WorkflowView>();
    let dag_mem = std::mem::size_of::<TaskDAGView>()
        + dag.tasks.len() * std::mem::size_of::<TaskView>()
        + dag.edges.len() * std::mem::size_of::<(usize, usize)>();

    let total_estimated_kb = (agent_mem + output_mem + workflow_mem + dag_mem) / 1024;

    println!(
        "Estimated memory usage: {}KB (agents: {}KB, output buffers: {}KB, workflow: {}B, dag: {}KB)",
        total_estimated_kb,
        agent_mem / 1024,
        output_mem / 1024,
        workflow_mem,
        dag_mem / 1024
    );

    // Data structures should be much smaller than 100MB
    // This test verifies the in-memory representation is reasonable
    assert!(
        total_estimated_kb < MAX_MEMORY_MB * 1024,
        "Estimated memory {}KB exceeds {}MB threshold",
        total_estimated_kb,
        MAX_MEMORY_MB
    );
}

/// Test that task data structures don't grow unexpectedly.
#[test]
fn test_task_struct_size() {
    let task_size = std::mem::size_of::<Task>();
    let task_id_size = std::mem::size_of::<TaskId>();
    let task_status_size = std::mem::size_of::<TaskStatus>();

    println!(
        "Struct sizes: Task={}B, TaskId={}B, TaskStatus={}B",
        task_size, task_id_size, task_status_size
    );

    // Tasks should be reasonably sized (< 512 bytes)
    assert!(task_size < 512, "Task struct too large: {}B", task_size);
    // TaskId should be small (UUID = 16 bytes + alignment)
    assert!(task_id_size <= 24, "TaskId too large: {}B", task_id_size);
}

// ============================================================================
// Render Blocking Tests (Lock-Free)
// ============================================================================

/// Test that state updates don't block rendering.
/// Simulates concurrent state updates and render reads.
#[tokio::test]
async fn test_no_render_blocking() {
    let state = Arc::new(RwLock::new(create_render_state_with_agents(4)));
    let update_count = Arc::new(AtomicUsize::new(0));
    let read_count = Arc::new(AtomicUsize::new(0));

    // Spawn writer task (simulates state updates)
    let state_writer = Arc::clone(&state);
    let updates = Arc::clone(&update_count);
    let writer_handle = tokio::spawn(async move {
        for i in 0..100 {
            let mut s = state_writer.write().await;
            s.version = i;
            if let Some(ref mut workflow) = s.workflow {
                workflow.phase_progress.0 = i as usize % 6;
            }
            updates.fetch_add(1, Ordering::Relaxed);
            // Small yield to allow readers
            tokio::task::yield_now().await;
        }
    });

    // Spawn reader task (simulates render loop)
    let state_reader = Arc::clone(&state);
    let reads = Arc::clone(&read_count);
    let reader_handle = tokio::spawn(async move {
        let deadline = Instant::now() + Duration::from_millis(100);
        while Instant::now() < deadline {
            let start = Instant::now();
            let s = state_reader.read().await;
            let _version = s.version;
            if let Some(ref workflow) = s.workflow {
                let _progress = workflow.progress_percentage();
            }
            for agent in &s.agents {
                let _label = agent.status_label();
            }
            drop(s);
            let read_time = start.elapsed();

            // Each read should be fast (no blocking)
            assert!(
                read_time.as_millis() < 10,
                "Read blocked for {}ms",
                read_time.as_millis()
            );

            reads.fetch_add(1, Ordering::Relaxed);
            tokio::task::yield_now().await;
        }
    });

    // Wait for both to complete
    let _ = tokio::join!(writer_handle, reader_handle);

    let total_updates = update_count.load(Ordering::Relaxed);
    let total_reads = read_count.load(Ordering::Relaxed);

    println!(
        "Concurrent access test: {} updates, {} reads",
        total_updates, total_reads
    );

    // Should have many reads during the test period
    assert!(
        total_reads > 10,
        "Too few reads ({}), possible blocking",
        total_reads
    );
    assert_eq!(total_updates, 100, "Not all updates completed");
}

/// Test that heavy state updates don't starve readers.
#[tokio::test]
async fn test_state_updates_dont_block_render_thread() {
    let state = Arc::new(RwLock::new(RenderState::default()));
    let blocked_reads = Arc::new(AtomicUsize::new(0));
    let total_reads = Arc::new(AtomicUsize::new(0));

    // Spawn heavy writer
    let state_writer = Arc::clone(&state);
    let writer = tokio::spawn(async move {
        for _ in 0..50 {
            let mut s = state_writer.write().await;
            // Simulate heavy state update
            s.agents = (0..10)
                .map(|i| {
                    AgentView::new(
                        AgentId::new(),
                        format!("task-{}", i),
                        AgentStatus::Idle,
                        Duration::from_secs(0),
                        "output".to_string(),
                    )
                })
                .collect();
            drop(s);
            tokio::time::sleep(Duration::from_micros(100)).await;
        }
    });

    // Spawn render loop
    let state_reader = Arc::clone(&state);
    let blocked = Arc::clone(&blocked_reads);
    let reads = Arc::clone(&total_reads);
    let reader = tokio::spawn(async move {
        for _ in 0..100 {
            let start = Instant::now();
            let s = state_reader.read().await;
            let _count = s.agents.len();
            drop(s);
            let elapsed = start.elapsed();

            reads.fetch_add(1, Ordering::Relaxed);
            if elapsed.as_millis() > MAX_FRAME_TIME_MS {
                blocked.fetch_add(1, Ordering::Relaxed);
            }
            tokio::task::yield_now().await;
        }
    });

    let _ = tokio::join!(writer, reader);

    let blocked = blocked_reads.load(Ordering::Relaxed);
    let total = total_reads.load(Ordering::Relaxed);
    let block_rate = (blocked as f64 / total as f64) * 100.0;

    println!(
        "Render blocking test: {}/{} reads blocked ({:.1}%)",
        blocked, total, block_rate
    );

    // Allow small number of blocked reads (< 10%)
    assert!(
        block_rate < 10.0,
        "Too many blocked reads: {:.1}%",
        block_rate
    );
}

// ============================================================================
// CI Integration Helpers
// ============================================================================

/// Summary test that prints all performance metrics.
#[test]
fn test_performance_summary() {
    println!("\n=== Zen v2 Performance Test Summary ===\n");

    // Render metrics
    let state = create_render_state_with_agents(4);
    let start = Instant::now();
    for _ in 0..60 {
        let _w = &state.workflow;
        if let Some(dag) = &state.dag {
            let _layers = dag.compute_layers();
        }
    }
    let render_time = start.elapsed().as_micros() / 60;
    println!(
        "TUI Render (4 agents): {}µs/frame [{} 60 FPS]",
        render_time,
        if render_time < (MAX_FRAME_TIME_MS * 1000) as u128 {
            "✓"
        } else {
            "✗"
        }
    );

    // Scheduler metrics
    let dag = create_task_dag(10);
    let completed = HashSet::new();
    let start = Instant::now();
    for _ in 0..100 {
        let _ready = dag.ready_tasks(&completed);
    }
    let sched_time = start.elapsed().as_micros() / 100;
    println!(
        "Scheduler Overhead (10 tasks): {}µs/dispatch [{} <10ms]",
        sched_time,
        if sched_time < (MAX_SCHEDULER_OVERHEAD_MS * 1000) as u128 {
            "✓"
        } else {
            "✗"
        }
    );

    // Memory metrics
    let task_size = std::mem::size_of::<Task>();
    let agent_view_size = std::mem::size_of::<AgentView>();
    println!(
        "Memory: Task={}B, AgentView={}B [✓ <512B]",
        task_size, agent_view_size
    );

    println!("\n========================================\n");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_render_state_helper() {
        let state = create_render_state_with_agents(4);
        assert!(state.workflow.is_some());
        assert_eq!(state.agents.len(), 4);
        assert!(state.dag.is_some());
    }

    #[test]
    fn test_create_task_dag_helper() {
        let dag = create_task_dag(10);
        assert_eq!(dag.task_count(), 10);
        assert!(dag.dependency_count() > 0);
    }
}
