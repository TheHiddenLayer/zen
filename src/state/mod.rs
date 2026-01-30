//! Git-native state management for Zen workflows.
//!
//! This module provides unified access to git refs, notes, and operations
//! for persisting workflow state directly in the git repository.

mod manager;

pub use manager::GitStateManager;
