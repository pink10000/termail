// This file defines the types for email messages and command results.

use serde::{Deserialize, Serialize};
use clap::Subcommand;

/// We implement CLI commands via clap subcommands and validate backend compatibility at runtime.
#[derive(Subcommand, Debug, Clone)]
pub enum Command {
    /// Fetch inbox emails
    FetchInbox {
        /// Number of emails to fetch (default: 1)
        #[arg(default_value_t = 1)]
        count: usize,
    },
    
    /// Send an email (currently not implemented)
    SendEmail {
        #[arg(short, long)]
        to: String,
        #[arg(short, long)]
        subject: String,
        #[arg(short, long)]
        body: String,
    },
}

/// Result type for backend commands - can represent different types of outputs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CommandResult {
    /// A single email message
    Email(EmailMessage),
    /// Multiple email messages
    Emails(Vec<EmailMessage>),
    /// A success message
    Success(String),
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
            CommandResult::Empty => write!(f, "NO CONTENT"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub enum MimeType {
    #[default]
    TextPlain,
    TextHtml,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmailMessage {
    pub id: String,
    pub subject: String,
    pub from: String,
    pub to: String,
    pub date: String,
    pub body: String,
    pub mime_type: MimeType,
}

impl EmailMessage {
    pub fn new() -> Self {
        Self {
            id: String::new(),
            subject: String::new(),
            from: String::new(),
            to: String::new(),
            date: String::new(),
            body: String::new(),
            mime_type: Default::default(),
        }
    }
}