/// This file defines our custom error type for backend operations.

use std::error::Error as StdError;
use std::fmt;

/// Custom error type for backend operations
#[derive(Debug)]
pub enum Error {
    /// Feature not implemented for this backend
    Unimplemented { backend: String, feature: String },
    
    /// IMAP protocol error
    Imap(imap::Error),
    
    /// Connection error
    Connection(String),
    
    /// Authentication error
    Authentication(String),
    
    /// Parse error
    Parse(String),
    
    /// Generic error with message
    Other(String),

    /// Config error
    Config(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Unimplemented { backend, feature } => {
                write!(f, "Feature '{}' is not implemented for backend '{}'", feature, backend)
            }
            Error::Imap(e) => write!(f, "IMAP error: {}", e),
            Error::Connection(msg) => write!(f, "Connection error: {}", msg),
            Error::Authentication(msg) => write!(f, "Authentication error: {}", msg),
            Error::Parse(msg) => write!(f, "Parse error: {}", msg),
            Error::Config(msg) => write!(f, "Config error: {}", msg),
            Error::Other(msg) => write!(f, "{}", msg),
        }
    }
}

impl StdError for Error {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match self {
            Error::Imap(e) => Some(e),
            _ => None,
        }
    }
}

// Automatically convert imap::Error to our Error type
impl From<imap::Error> for Error {
    fn from(err: imap::Error) -> Self {
        Error::Imap(err)
    }
}