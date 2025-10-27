extern crate imap;

pub mod greenmail;
pub mod gmail;
pub mod error;
pub use error::Error;

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
    fn check_command_support(&self, cmd: &Command) -> Result<bool, Error>;
    fn do_command(&self, cmd: Command) -> Result<Option<String>, Error>;
    fn fetch_inbox_top(&self) -> Result<Option<String>, Error>;
    fn fetch_inbox_top_n(&self, n: usize) -> Result<Vec<String>, Error>;
}

#[derive(Debug, Clone, Copy)]
pub enum BackendType {
    GreenMail,
    Gmail,
}

// The trait std::str::FromStr is part of Rust stdlib to convert a string to a type.
// The cli parser `clap` uses this to trait to parse the backend.
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
    /// Get a trait object for this backend
    pub fn get_backend(&self) -> Box<dyn Backend> {
        match self {
            BackendType::GreenMail => Box::new(greenmail::GreenmailBackend),
            BackendType::Gmail => Box::new(gmail::GmailBackend),
        }
    }
}

