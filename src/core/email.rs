use serde::{Deserialize, Serialize};
use crate::error::Error;

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
        let content = self.display_name();        
        if let Some(p) = f.precision() {
            // Truncate to precision 'p' then pad
            f.pad(&format!("{:.1$}", content, p))
        } else {
            f.pad(content)
        }
    }
}

impl EmailSender {
    /// Returns just the name (or email fallback).
    pub fn display_name(&self) -> &str {
        self.name.as_deref().unwrap_or(&self.email)
    }

    pub fn formatted_email(&self) -> String {
        format!("<{}>", self.email)
    }

    /// Returns the standard "Name <email>" format.
    pub fn full_string(&self) -> String {
        match &self.name {
            Some(name) => format!("{} <{}>", name, self.email),
            None => self.email.clone(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmailAttachment {
    pub filename: String,
    pub content_type: String,
    pub data: Vec<u8>,
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
    pub email_attachments: Vec<EmailAttachment>,
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
            email_attachments: Vec::new(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.to.is_empty() && self.subject.is_empty() && self.body.is_empty()
    }

    pub fn is_partially_empty(&self) -> bool {
        self.to.is_empty() || self.subject.is_empty() || self.body.is_empty()
    }

    // fn to_email_content(&self) -> String {
    //     format!(
    //         "To: {}\r\nSubject: {}\r\nContent-Type: text/plain; charset=UTF-8\r\n\r\n{}",
    //         self.to, self.subject, self.body
    //     )
    // }

    pub fn to_lettre_email(&self) -> Result<lettre::Message, Error> {
        lettre::Message::builder()
            .from("me@localhost".parse().unwrap()) // Gmail ignores this and uses the authenticated user
            .to(self.to.parse().unwrap())
            .subject(self.subject.clone())
            .header(lettre::message::header::ContentType::TEXT_PLAIN)
            .body(self.body.clone())
            .map_err(|e: lettre::error::Error| Error::Other(format!("Failed to build email: {}", e)))
    }
}