// This file defines the types for email messages and command results.

use clap::Subcommand;
use crate::core::{email::EmailMessage, label::Label};

/// We implement CLI commands via clap subcommands and validate backend compatibility at runtime.
#[derive(Subcommand, Debug, Clone)]
pub enum Command {
    /// Fetch inbox emails
    FetchInbox {
        /// Number of emails to fetch (default: 1)
        #[arg(default_value_t = 1)]
        count: usize,
    },

    /// Fetch the list of labels   
    ListLabels,
    
    /// Send an email (currently not implemented)
    SendEmail {
        #[arg(short, long)]
        to: Option<String>,
        #[arg(short, long)]
        subject:  Option<String>,
        #[arg(short, long)]
        body: Option<String>,
    },

    SyncFromCloud,

    /// View emails from local maildir
    ViewMailbox {
        /// Number of emails to view (default: 1)
        #[arg(default_value_t = 1)]
        count: usize,
    },

    /// Load a single email (with attachments) by id from the local maildir
    LoadEmail {
        /// Email (maildir) id to load
        email_id: String,
    },

    /// Null command (used for testing plugins))
    Null
}

/// Result type for backend commands - can represent different types of outputs
#[derive(Debug, Clone)]
pub enum CommandResult {
    /// A single email message
    Email(EmailMessage),
    /// Multiple email messages
    Emails(Vec<EmailMessage>),
    /// A success message
    Success(String),
    /// List Of Labels
    Labels(Vec<Label>),
    /// No content to return
    Empty,
}

impl std::fmt::Display for CommandResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CommandResult::Email(email) => {
                write!(f, "Subject: {}\nFrom: {}\nTo: {}\nDate: {}\n\n{}",
                    email.subject, email.from, email.to, email.date, email.body)
            }
            CommandResult::Emails(emails) => {
                if emails.is_empty() {
                    write!(f, "NO EMAILS FOUND")
                } else {
                    for (i, email) in emails.iter().enumerate() {
                        write!(f, "=== Email {} ===\n", i + 1)?;
                        write!(f, "Subject: {}\nFrom: {}\nTo: {}\nDate: {}\n\n{}\n\n",
                            email.subject, email.from, email.to, email.date, email.body)?;
                    }
                    Ok(())
                }
            }
            CommandResult::Success(msg) => write!(f, "{}", msg),
            CommandResult::Labels(labels) => write!(f, "{:?}", labels),
            CommandResult::Empty => write!(f, "NO CONTENT"),
        }
    }
}

