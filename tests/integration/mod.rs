//! Integration test suite for Zen v2.
//!
//! These tests exercise the full workflow from prompt to completion,
//! including parallel execution and conflict resolution. They verify
//! that all components work together correctly.
//!
//! # Test Categories
//!
//! - `workflow_e2e`: Full workflow execution tests
//! - `parallel_agents`: Parallel execution correctness
//! - `conflict_resolution`: Merge conflict handling
//! - `recovery`: Health monitor and recovery tests
//! - `performance`: Performance benchmarks and thresholds
//!
//! # CI Compatibility
//!
//! These tests use mock Claude responses and do not make actual API calls,
//! making them safe to run in CI environments.

mod fixtures;

mod workflow_e2e;
mod parallel_agents;
mod conflict_resolution;
mod recovery;
mod performance;
