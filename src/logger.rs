// Logger module for termail
//
// This module provides structured logging using the tracing crate.
// In TUI mode, logs are written to a file to avoid corrupting the terminal UI.
// In CLI mode, logs go to both stdout and the file for immediate feedback.

use crate::error::Error;
use std::path::PathBuf;
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

/// Converts verbosity count to log level string
fn verbosity_to_level(verbosity: u8) -> &'static str {
    match verbosity {
        0 => "error",  // No -v flags: only errors
        1 => "info",   // -v: info and above
        2 => "debug",  // -vv: debug and above
        _ => "trace",  // -vvv or more: everything
    }
}

/// Initialize the tracing logger with appropriate output based on mode
pub fn init_logger(is_tui: bool, verbosity: u8, log_path: PathBuf) -> Result<(), Error> {
    let log_level = verbosity_to_level(verbosity);

    // Create the log directory if it doesn't exist
    if let Some(parent) = log_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| {
            Error::Other(format!("Failed to create log directory: {}", e))
        })?;
    }

    // Create the file appender for writing logs to disk
    let file_appender = tracing_appender::rolling::never(
        log_path.parent().unwrap_or(&PathBuf::from(".")),
        log_path.file_name().unwrap_or(std::ffi::OsStr::new("termail.log")),
    );

    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(log_level));

    if is_tui {
        // TUI mode: Only log to file to avoid corrupting the terminal UI
        tracing_subscriber::registry()
            .with(env_filter)
            .with(fmt::layer()
                .with_writer(file_appender)
                .with_ansi(false)  // No ANSI colors in log files
                .with_target(true)
                .with_thread_ids(false)
                .with_line_number(true))
            .init();
    } else {
        // CLI mode: Log to both stdout and file for immediate feedback
        let stdout_layer = fmt::layer()
            .with_writer(std::io::stdout)
            .with_ansi(true)  // Colors for terminal
            .with_target(false)
            .with_line_number(false);

        let file_layer = fmt::layer()
            .with_writer(file_appender)
            .with_ansi(false)  // No colors in log file
            .with_target(true)
            .with_line_number(true);

        tracing_subscriber::registry()
            .with(env_filter)
            .with(stdout_layer)
            .with(file_layer)
            .init();
    }

    tracing::info!("Logger initialized with level: {}", log_level);
    tracing::debug!("Log file: {:?}", log_path);

    Ok(())
}

