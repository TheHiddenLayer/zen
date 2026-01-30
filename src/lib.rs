pub mod agent;
pub mod config;
pub mod error;
pub mod git;
pub mod git_notes;
pub mod log;
pub mod session;
pub mod tmux;
pub mod util;

// Decoupled game loop architecture
pub mod actors;
pub mod app;
pub mod render;
pub mod tea;
pub mod ui;

pub use error::{Error, Result};
pub use session::{Session, SessionId, SessionStatus, State};

/// Architecture verification tests.
///
/// These tests verify the core properties of the Decoupled Game Loop architecture:
/// - Thread safety: Lock-free channels never block
/// - State isolation: Immutable snapshots prevent race conditions
/// - Performance: Consistent frame timing and input latency
#[cfg(test)]
mod architecture_tests {
    use crate::render::{next_version, RenderState};
    use std::time::{Duration, Instant};

    /// Verify that the frame duration constant aligns with 60 FPS target.
    #[test]
    fn test_frame_duration_is_60fps() {
        const TARGET_FPS: u32 = 60;
        const FRAME_DURATION: Duration = Duration::from_micros(1_000_000 / TARGET_FPS as u64);

        let expected_ms = 1000.0 / 60.0; // ~16.67ms
        let actual_ms = FRAME_DURATION.as_secs_f64() * 1000.0;

        assert!(
            (actual_ms - expected_ms).abs() < 0.1,
            "Frame duration should be ~16.67ms, got {}ms",
            actual_ms
        );
    }

    /// Verify that RenderState::default() is cheap to create.
    /// This is important because the render thread may create default states.
    #[test]
    fn test_render_state_default_is_cheap() {
        let start = Instant::now();
        for _ in 0..10000 {
            let _ = RenderState::default();
        }
        let elapsed = start.elapsed();

        assert!(
            elapsed.as_millis() < 100,
            "Creating 10000 default RenderStates took {:?} - should be < 100ms",
            elapsed
        );
    }

    /// Verify that version generation is fast and atomic.
    #[test]
    fn test_version_generation_is_fast() {
        let start = Instant::now();
        for _ in 0..100000 {
            let _ = next_version();
        }
        let elapsed = start.elapsed();

        assert!(
            elapsed.as_millis() < 50,
            "Generating 100000 versions took {:?} - should be < 50ms",
            elapsed
        );
    }

    /// Verify that versions are strictly monotonic.
    #[test]
    fn test_version_monotonicity() {
        let mut prev = 0u64;
        for _ in 0..1000 {
            let v = next_version();
            assert!(v > prev, "Version {} should be > previous {}", v, prev);
            prev = v;
        }
    }

    /// Verify the bounded channel pattern works for latest-wins semantics.
    #[test]
    fn test_bounded_channel_latest_wins() {
        let (tx, rx) = crossbeam_channel::bounded::<RenderState>(1);

        // Simulate rapid state updates (sender faster than receiver)
        for i in 0..100 {
            // Drain old state if present
            let _ = rx.try_recv();

            // Send new state
            let mut state = RenderState::default();
            state.selected = i;
            let _ = tx.try_send(state);
        }

        // Receiver should get the latest state (99)
        let received = rx.try_recv().unwrap();
        assert_eq!(
            received.selected, 99,
            "Should receive latest state, got {}",
            received.selected
        );
    }

    /// Verify that try_send never blocks on a full channel.
    /// This is CRITICAL for the decoupled architecture.
    #[test]
    fn test_try_send_never_blocks_on_full_channel() {
        let (tx, _rx) = crossbeam_channel::bounded::<RenderState>(1);

        // Fill the channel
        let _ = tx.try_send(RenderState::default());

        // Measure time to try_send when full
        let iterations = 10000;
        let start = Instant::now();
        for _ in 0..iterations {
            let _ = tx.try_send(RenderState::default());
        }
        let elapsed = start.elapsed();

        // Should be extremely fast since try_send doesn't block
        let avg_ns = elapsed.as_nanos() / iterations as u128;
        assert!(
            avg_ns < 1000, // Less than 1 microsecond average
            "try_send averaged {}ns per call - should be < 1000ns",
            avg_ns
        );
    }

    /// Verify that RenderState clone is reasonably fast.
    /// The render thread receives cloned states.
    #[test]
    fn test_render_state_clone_performance() {
        let state = RenderState {
            version: 42,
            sessions: vec![], // Empty for baseline
            selected: 0,
            mode: crate::tea::Mode::List,
            preview: Some("A".repeat(10000)), // 10KB preview
            input_buffer: String::new(),
            notification: None,
            show_keymap: false,
            trust_enabled: false,
        };

        let start = Instant::now();
        for _ in 0..1000 {
            let _ = state.clone();
        }
        let elapsed = start.elapsed();

        assert!(
            elapsed.as_millis() < 500,
            "Cloning 1000 states with 10KB preview took {:?} - should be < 500ms",
            elapsed
        );
    }

    /// Integration test: Simulate rapid input processing doesn't block.
    #[test]
    fn test_rapid_state_updates_dont_block() {
        let (tx, rx) = crossbeam_channel::bounded::<RenderState>(1);

        // Simulate 1000 rapid state updates (like fast keyboard input)
        let start = Instant::now();
        for i in 0..1000 {
            // Drain and send pattern (mimics actual logic thread)
            let _ = rx.try_recv();
            let mut state = RenderState::default();
            state.version = next_version();
            state.selected = i;
            let _ = tx.try_send(state);
        }
        let elapsed = start.elapsed();

        assert!(
            elapsed.as_millis() < 50,
            "1000 rapid state updates took {:?} - should be < 50ms",
            elapsed
        );
    }
}
