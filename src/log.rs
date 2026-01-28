//! Structured logging for debugging zen.
//!
//! Log levels:
//! - ERROR: Critical failures that prevent operations from completing
//! - WARN: Unexpected conditions that are recoverable
//! - INFO: High-level operation notifications (startup, shutdown, session events)
//! - DEBUG: Detailed operation traces (useful for debugging)
//! - TRACE: Very detailed traces (tmux output, internal state)
//!
//! Debug mode can be enabled with `--debug` flag or `ZEN_DEBUG=1` env var.

use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::OnceLock;

static LOG_PATH: OnceLock<PathBuf> = OnceLock::new();
static DEBUG_ENABLED: AtomicBool = AtomicBool::new(false);
static LOG_LEVEL: AtomicU8 = AtomicU8::new(LogLevel::Info as u8);

/// Log levels for filtering messages.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum LogLevel {
    Error = 0,
    Warn = 1,
    Info = 2,
    Debug = 3,
    Trace = 4,
}

impl LogLevel {
    pub fn as_str(&self) -> &'static str {
        match self {
            LogLevel::Error => "ERROR",
            LogLevel::Warn => "WARN",
            LogLevel::Info => "INFO",
            LogLevel::Debug => "DEBUG",
            LogLevel::Trace => "TRACE",
        }
    }

    fn from_u8(v: u8) -> Self {
        match v {
            0 => LogLevel::Error,
            1 => LogLevel::Warn,
            2 => LogLevel::Info,
            3 => LogLevel::Debug,
            _ => LogLevel::Trace,
        }
    }
}

/// Initialize logging to ~/.zen/zen.log
pub fn init() {
    init_with_debug(false);
}

/// Initialize logging with explicit debug mode setting.
pub fn init_with_debug(debug: bool) {
    // Check env var for debug mode
    let env_debug = std::env::var("ZEN_DEBUG")
        .map(|v| v == "1" || v.to_lowercase() == "true")
        .unwrap_or(false);

    let debug_enabled = debug || env_debug;
    DEBUG_ENABLED.store(debug_enabled, Ordering::SeqCst);

    // Set log level based on debug mode
    let level = if debug_enabled {
        LogLevel::Debug
    } else {
        LogLevel::Info
    };
    LOG_LEVEL.store(level as u8, Ordering::SeqCst);

    if let Some(zen_dir) = dirs::home_dir().map(|h| h.join(".zen")) {
        let _ = std::fs::create_dir_all(&zen_dir);
        let path = zen_dir.join("zen.log");
        // Truncate file on startup
        let _ = std::fs::write(&path, "");
        LOG_PATH.set(path).ok();
    }
}

/// Check if debug mode is enabled.
pub fn is_debug() -> bool {
    DEBUG_ENABLED.load(Ordering::Relaxed)
}

/// Set the minimum log level for output.
pub fn set_level(level: LogLevel) {
    LOG_LEVEL.store(level as u8, Ordering::SeqCst);
}

/// Get the current log level.
pub fn get_level() -> LogLevel {
    LogLevel::from_u8(LOG_LEVEL.load(Ordering::Relaxed))
}

/// Log a message at the specified level.
pub fn log_at(level: LogLevel, msg: &str) {
    let current_level = LogLevel::from_u8(LOG_LEVEL.load(Ordering::Relaxed));
    if level > current_level {
        return;
    }

    if let Some(path) = LOG_PATH.get() {
        if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) {
            let timestamp = chrono::Local::now().format("%H:%M:%S%.3f");
            let _ = writeln!(file, "[{}] [{}] {}", timestamp, level.as_str(), msg);
        }
    }
}

/// Log a message at INFO level (default, always logged).
pub fn log(msg: &str) {
    log_at(LogLevel::Info, msg);
}

/// Log a message at ERROR level.
pub fn error(msg: &str) {
    log_at(LogLevel::Error, msg);
}

/// Log a message at WARN level.
pub fn warn(msg: &str) {
    log_at(LogLevel::Warn, msg);
}

/// Log a message at INFO level.
pub fn info(msg: &str) {
    log_at(LogLevel::Info, msg);
}

/// Log a message at DEBUG level (only in debug mode).
pub fn debug(msg: &str) {
    log_at(LogLevel::Debug, msg);
}

/// Log a message at TRACE level (very verbose).
pub fn trace(msg: &str) {
    log_at(LogLevel::Trace, msg);
}

/// Log macro for INFO level (convenience, backward compatible).
#[macro_export]
macro_rules! zlog {
    ($($arg:tt)*) => {
        $crate::log::log(&format!($($arg)*))
    };
}

/// Log macro for ERROR level.
#[macro_export]
macro_rules! zlog_error {
    ($($arg:tt)*) => {
        $crate::log::error(&format!($($arg)*))
    };
}

/// Log macro for WARN level.
#[macro_export]
macro_rules! zlog_warn {
    ($($arg:tt)*) => {
        $crate::log::warn(&format!($($arg)*))
    };
}

/// Log macro for DEBUG level (only logs when debug mode is enabled).
#[macro_export]
macro_rules! zlog_debug {
    ($($arg:tt)*) => {
        $crate::log::debug(&format!($($arg)*))
    };
}

/// Log macro for TRACE level (very verbose, only in debug mode with trace level).
#[macro_export]
macro_rules! zlog_trace {
    ($($arg:tt)*) => {
        $crate::log::trace(&format!($($arg)*))
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_log_level_ordering() {
        assert!(LogLevel::Error < LogLevel::Warn);
        assert!(LogLevel::Warn < LogLevel::Info);
        assert!(LogLevel::Info < LogLevel::Debug);
        assert!(LogLevel::Debug < LogLevel::Trace);
    }

    #[test]
    fn test_log_level_as_str() {
        assert_eq!(LogLevel::Error.as_str(), "ERROR");
        assert_eq!(LogLevel::Warn.as_str(), "WARN");
        assert_eq!(LogLevel::Info.as_str(), "INFO");
        assert_eq!(LogLevel::Debug.as_str(), "DEBUG");
        assert_eq!(LogLevel::Trace.as_str(), "TRACE");
    }

    #[test]
    fn test_log_level_from_u8() {
        assert_eq!(LogLevel::from_u8(0), LogLevel::Error);
        assert_eq!(LogLevel::from_u8(1), LogLevel::Warn);
        assert_eq!(LogLevel::from_u8(2), LogLevel::Info);
        assert_eq!(LogLevel::from_u8(3), LogLevel::Debug);
        assert_eq!(LogLevel::from_u8(4), LogLevel::Trace);
        assert_eq!(LogLevel::from_u8(255), LogLevel::Trace); // Out of range defaults to Trace
    }
}
