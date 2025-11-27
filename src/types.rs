// This file defines the types for email messages and command results.

use serde::{Deserialize, Serialize};
use clap::Subcommand;
use google_gmail1::api::LabelColor;

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

    // SyncFromCloud,

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

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub enum MimeType {
    #[default]
    TextPlain,
    TextHtml,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct EmailSender {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub email: String,
}

// This allows you to do: EmailSender::from("Bob <bob@gmail.com>".to_string())
impl From<String> for EmailSender {
    fn from(value: String) -> Self {
        if let (Some(left), Some(right)) = (value.find("<"), value.rfind(">")) {
            if left < right {
                let name_field = value[..left].trim().to_string();
                let email_field = value[left + 1..right].to_string();

                return EmailSender {
                    name: if name_field.is_empty() { None } else { Some(name_field) },
                    email: email_field
                }
            }
        }

        // Fallback
        EmailSender { name: None, email: value }
    }
}

impl std::fmt::Display for EmailSender {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let content = match &self.name {
            Some(name) => name.clone(),
            None => self.email.clone()
        };
        let displayed_text = if let Some(p) = f.precision() {
             format!("{:.1$}", content, p) 
        } else {
             content
        };
        f.pad(&displayed_text)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmailMessage {
    pub id: String,
    pub subject: String,
    pub from: EmailSender,
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
            from: EmailSender::default(),
            to: String::new(),
            date: String::new(),
            body: String::new(),
            mime_type: Default::default(),
        }
    }
}

/// The `google_gmail1::api::Label` has its own Label type, but we're wrapping 
/// it in our own type for consistency.
/// 
/// We reuse some of the fields from the `google_gmail1::api::Label` type, but not all of them.
/// We reuse the `LabelColor` enum from the `google_gmail1::api::LabelColor` type.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Label {
    pub color: Option<LabelColor>,
    /// The immutable ID of the label.
    pub id: Option<String>,
    /// The total number of messages with the label.
    #[serde(rename = "messagesTotal")]
    pub messages_total: Option<usize>,
    /// The number of unread messages with the label.
    #[serde(rename = "messagesUnread")]
    pub messages_unread: Option<usize>,
    /// The display name of the label.
    pub name: Option<String>,
}

impl Label {
    pub fn new() -> Self {
        Self {
            color: None,
            id: None,
            messages_total: None,
            messages_unread: None,
            name: None,
        }
    }
}

impl std::fmt::Display for Label {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Name: {:?}\n\tColor: {:?}\n\tID: {:?}\n\tMessages Total: {:?}\n\tMessages Unread: {:?}",
            self.name.as_ref(),
            self.color.as_ref(),
            self.id.as_ref(),
            self.messages_total.as_ref(),
            self.messages_unread.as_ref()
        )
    }
}