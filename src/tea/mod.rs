//! The Elm Architecture (TEA) implementation for Zen TUI.
//!
//! This module provides a clean separation of concerns:
//! - `Model`: Pure application state
//! - `Message`: Inputs to the update function
//! - `Command`: Outputs (side effects) from the update function
//! - `update`: Pure function that transforms state

pub mod command;
pub mod message;
pub mod model;
pub mod update;

pub use command::Command;
pub use message::Message;
pub use model::{InputKind, Mode, Model, Notification, NotificationLevel, PromptState};
pub use update::update;
