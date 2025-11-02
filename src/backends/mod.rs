extern crate imap;

pub mod greenmail;
pub mod gmail;
use crate::error::Error;
use crate::config::BackendConfig;

use std::fmt;
use clap::Subcommand;

/// We implement CLI commands via clap subcommands and validate backend compatibility at runtime.
#[derive(Subcommand, Debug)]
pub enum Command {
    
    /// Fetch inbox
    FetchInbox {
        #[arg(short, long, default_value_t = 1)]
        count: usize,
    },
}

pub trait Backend {
    fn needs_oauth(&self) -> bool {
        false
    }

    /// Perform authentication (if needed). This is a sync wrapper that may spawn async tasks.
    /// Returns Ok(()) if authentication succeeded or wasn't needed.
    fn authenticate(&mut self) -> Result<(), Error> {
        Ok(())
    }

    fn check_command_support(&self, cmd: &Command) -> Result<bool, Error>;
    fn do_command(&self, cmd: Command) -> Result<Option<String>, Error>;
    fn fetch_inbox_top(&self) -> Result<Option<String>, Error>;
    fn fetch_inbox_top_n(&self, n: usize) -> Result<Vec<String>, Error>;
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
    /// Check if this backend type requires OAuth2 authentication
    pub fn needs_oauth(&self) -> bool {
        match self {
            BackendType::GreenMail => false,
            BackendType::Gmail => true,
        }
    }

    /// Get a trait object for this backend, initialized with its configuration
    pub fn get_backend(&self, config: &BackendConfig) -> Box<dyn Backend> {
        match self {
            BackendType::GreenMail => Box::new(greenmail::GreenmailBackend::new(config)),
            BackendType::Gmail => Box::new(gmail::GmailBackend::new(config)),
        }
    }
}

