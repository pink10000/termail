extern crate imap;

pub mod greenmail;
pub mod gmail;
use crate::error::Error;
use crate::config::BackendConfig;
use crate::cli::command::{Command, CommandResult};
use async_trait::async_trait;
use crate::plugins::plugins::PluginManager;
use std::fmt;

#[async_trait]
pub trait Backend: Send {    
    /// Check if this backend requires OAuth2 authentication
    fn needs_oauth(&self) -> bool;

    /// Perform authentication (if needed). This is a sync wrapper that may spawn async tasks.
    /// Returns Ok(()) if authentication succeeded or wasn't needed.
    async fn authenticate(&mut self) -> Result<(), Error> {
        Ok(())
    }

    /// Execute a command and return a structured result
    /// 
    /// The plugin_manager is optional - only pass it for commands that need plugin dispatch
    async fn do_command(&self, cmd: Command, plugin_manager: Option<&mut PluginManager>) -> Result<CommandResult, Error>;

    /// Check if a particular command requires authentication
    /// 
    /// This function WILL NOT authenticate the backend and `authenticate()` should be called after.
    fn requires_authentication(&self, cmd: &Command) -> Option<bool>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BackendType {
    #[serde(rename = "greenmail")]
    GreenMail,
    #[serde(rename = "gmail")]
    Gmail,
}

impl std::str::FromStr for BackendType {
    type Err = String;
    
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "greenmail" => Ok(BackendType::GreenMail),
            "gmail" => Ok(BackendType::Gmail),
            // this will need a way to list all available backends without having to hardcode them here
            _ => Err(format!("Invalid backend: {}. Available backends are: greenmail, gmail", s)),
        }
    }
}

impl fmt::Display for BackendType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BackendType::GreenMail => write!(f, "greenmail"),
            BackendType::Gmail => write!(f, "gmail"),
        }
    }
}

impl BackendType {
    /// Get a trait object for this backend, initialized with its configuration
    pub fn get_backend(&self, config: &BackendConfig, editor: &str) -> Box<dyn Backend> {
        match self {
            BackendType::GreenMail => Box::new(greenmail::GreenmailBackend::new(config, editor.to_string())),
            BackendType::Gmail => Box::new(gmail::GmailBackend::new(config, editor.to_string())),
        }
    }
}

