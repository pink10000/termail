extern crate imap;

use super::{Backend, Error};
use crate::auth::Credentials;
use crate::config::BackendConfig;
use crate::cli::command::{Command, CommandResult};
use crate::core::{email::{EmailMessage, EmailSender}, label::Label};
use async_trait::async_trait;
use lettre::{Transport, Message, SmtpTransport};
use tempfile::NamedTempFile;
use std::io::Write;
use crate::plugins::plugins::PluginManager;

pub struct GreenmailBackend {
    host: String,
    port: u16,
    _ssl: bool, // TODO: remove this once we have a proper SSL implementation
    credentials: Credentials,
    editor: String,
}

impl GreenmailBackend {
    pub fn new(config: &BackendConfig, editor: String) -> Self {
        let credentials = config.auth_credentials.clone()
            .expect("Greenmail backend requires credentials in configuration");
        
        Self {
            host: config.host.clone(),
            port: config.port,
            _ssl: config.ssl,
            credentials,
            editor,
        }
    }
}

impl GreenmailBackend {
    fn fetch_inbox_emails(&self, count: usize) -> Result<Vec<EmailMessage>, Error> {
        let domain = self.host.as_str();
        
        // For local testing with self-signed certificates, we need to accept invalid certs
        // while still maintaining TLS encryption
        let tls = native_tls::TlsConnector::builder()
            .danger_accept_invalid_certs(true)
            .danger_accept_invalid_hostnames(true)
            .build()
            .unwrap();
    
        // we pass in the domain twice to check that the server's TLS
        // certificate is valid for the domain we're connecting to.
        let client = imap::connect((domain, self.port), domain, &tls).unwrap();
    
        // the client we have here is unauthenticated.
        // to do anything useful with the e-mails, we need to log in
        let mut imap_session = client
            .login(&self.credentials.username, &self.credentials.password)
            .map_err(|e| e.0)?;
    
        // we want to fetch the first email in the INBOX mailbox
        imap_session.select("INBOX")?;
    
        // fetch message number 1 in this mailbox, along with its RFC822 field.
        // RFC 822 dictates the format of the body of e-mails
        let fetch_range = if count == 1 {
            "1".to_string()
        } else {
            format!("1:{count}")
        };
        
        let messages = imap_session.fetch(fetch_range.as_str(), "RFC822")?;
        let emails = messages.iter()
            .map(|message| self.parse_email_message(message))
            .collect::<Result<Vec<EmailMessage>, Error>>()?;
    
        // be nice to the server and log out
        imap_session.logout()?;
    
        Ok(emails)
    }

    fn list_labels(&self) -> Result<Vec<Label>, Error> {
        eprintln!("unimplemented!");
        return Err(Error::Unimplemented {
            backend: "greenmail".to_string(),
            feature: "list_labels".to_string(),
        });
    }  

    /// Greenmail (or the library?) parses emails in a weird way. This method provides a layer to our
    /// `EmailMessage` type api.
    fn parse_email_message(&self, message: &imap::types::Fetch) -> Result<EmailMessage, Error> {
        let body = message.body().unwrap_or(&[]);
        let body_str = std::str::from_utf8(body)
            .unwrap_or("(invalid utf-8)")
            .to_string();

        let mut output = EmailMessage::new();

        // need to split body_str into headers and body
        let (headers, body) = body_str.split_once("\r\n\r\n").unwrap();
        for header in headers.lines() {
            let (name, value) = header.split_once(": ").unwrap();
            match name {
                "Subject" => output.subject = value.to_string(),
                "To" => output.to = value.to_string(),
                "From" => output.from = EmailSender::from(value.to_string()),
                "Received" => {
                    output.date = value.split_once(";").unwrap().1.trim().to_string();
                },
                _ => (),
            }
        }

        output.body = body.to_string();
        Ok(output)
    }


    /// Opens the provided editor (e.g., vim, code) to allow the user to edit the email draft.
    /// Prefills the email with any available information (to, subject, body) from cli and writes it as template to a temporary file.
    /// After the user edits the email and exits the editor, the function reads the updated content and returns the modified `EmailMessage`.
    fn edit_email_with_prefill(editor: &str, mut draft: EmailMessage) -> std::io::Result<EmailMessage> {
        
        // Create a new temp file to be used by editor
        // File gets deleted once out of scope
        let mut temp_file = NamedTempFile::new()?;

        // Write draft information into temp file
        writeln!(temp_file, "To: {}", draft.to)?;
        writeln!(temp_file, "Subject: {}", draft.subject)?;
        writeln!(temp_file, "Body:\n{}", draft.body)?;

        // Get temp file path        
        let temp_file_path = temp_file.path().to_owned();

        // Create command to run editor with path as arg
        let mut command = std::process::Command::new(editor);
        if editor.contains("code") {
            // Add wait arg for vscode to ensure file is saved before returning
            command.arg("--wait").arg(&temp_file_path);
        }
        else {
            command.arg(&temp_file_path);
        }

        // Run the editor and check if it was successful
        let status = command.status()?;
        if !status.success() {
            eprintln!("Editor failed with status: {:?}", status);
            return Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                "Editor failed",
            ));
        }

        // After the user exits the editor, read contents of temp file
        let contents = std::fs::read_to_string(&temp_file_path)?;
        let mut in_body = false;
        let mut body_lines = Vec::new();

        // Iterate through the lines of the file and parse the email fields
        // Evertyhing after Body: goes into body_lines
        for line in contents.lines() {
            if in_body {
                body_lines.push(line);
            } else if line.starts_with("To:") {
                draft.to = line["To:".len()..].trim().to_string();
            } else if line.starts_with("Subject:") {
                draft.subject = line["Subject:".len()..].trim().to_string();
            } else if line.starts_with("Body:") {
                in_body = true;
                body_lines.push(line["Body:".len()..].trim());
            }
        }
        draft.body = body_lines.join("\n");
        Ok(draft)
    }

    /// Send an email using the `lettre` library.
    fn send_email(&self, draft: &EmailMessage) -> Result<CommandResult, Error> {
        // Build the email message
        let email = Message::builder()
            .from("GreenMailTester <greenmail@domain.tester>".parse().unwrap())
            .to(draft.to.parse().unwrap())
            .subject(draft.subject.clone())
            .body(draft.body.clone())
            .unwrap();

        // Create an SMTP transport (for local testing)
        let mailer = SmtpTransport::builder_dangerous("127.0.0.1")
            .port(1025)
            .build();

        // Send the email
        match mailer.send(&email) {
            Ok(_) => {
                println!("Email sent successfully.");
                Ok(CommandResult::Empty)
            },
            Err(e) => {
                eprintln!("Failed to send email: {}", e);
                Err(Error::Connection(e.to_string()))
            },
        }
    }

}

#[async_trait]
impl Backend for GreenmailBackend {
    fn needs_oauth(&self) -> bool {
        false 
    }

    async fn do_command(&self, cmd: Command, _plugin_manager: Option<&mut PluginManager>) -> Result<CommandResult, Error> {
        match cmd {
            Command::FetchInbox { count } => {
                let emails = self.fetch_inbox_emails(count)?;
                if emails.is_empty() {
                    Ok(CommandResult::Empty)
                } else if count == 1 {
                    Ok(CommandResult::Email(emails.into_iter().next().unwrap()))
                } else {
                    Ok(CommandResult::Emails(emails))
                }
            },
            Command::ListLabels => {
                let labels = self.list_labels()?;
                Ok(CommandResult::Labels(labels))
            }
            Command::SendEmail { to, subject, body } => {
                let mut draft = EmailMessage::new();
                draft.to = to.unwrap_or_default();
                draft.subject = subject.unwrap_or_default();
                draft.body = body.unwrap_or_default();

                let draft = if draft.to.is_empty() || draft.subject.is_empty() || draft.body.is_empty() {
                    Self::edit_email_with_prefill(&self.editor, draft)?
                } else {
                    draft
                };

                if draft.to.is_empty() {
                    return Err(Error::InvalidInput("To field cannot be empty".to_string()));
                }

                self.send_email(&draft)
            }
            Command::SyncFromCloud => {

                println!("sync from lcoud called");

                Ok(CommandResult::Empty)
            }
            Command::ViewMailbox { count } => {

                println!("view mailbox called, count: {:?}", count);

                Ok(CommandResult::Empty)
            }
            Command::Null => Ok(CommandResult::Empty)
        }
    }

    /// Defines which commands require authentication to the Greenmail service.
    fn requires_authentication(&self, cmd: &Command) -> Option<bool> {
        match cmd {
            Command::SyncFromCloud => Some(true),
            Command::ViewMailbox { count: _ } => Some(false),
            Command::SendEmail { to: _, subject: _, body: _ } => Some(true),
            // Command::FetchInbox { count: _ } => None, // TODO: deprecate fetch inbox for greenmail backend
            Command::ListLabels => Some(false),
            Command::Null => Some(false),
            _ => None
        }
    }
}