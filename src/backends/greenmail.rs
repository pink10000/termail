extern crate imap;

use super::{Backend, Error};
use crate::auth::Credentials;
use crate::config::BackendConfig;
use crate::types::{Command, CommandResult, EmailMessage};

pub struct GreenmailBackend {
    host: String,
    port: u16,
    _ssl: bool, // TODO: remove this once we have a proper SSL implementation
    credentials: Credentials,
}

impl GreenmailBackend {
    pub fn new(config: &BackendConfig) -> Self {
        let credentials = config.auth_credentials.clone()
            .expect("Greenmail backend requires credentials in configuration");
        
        Self {
            host: config.host.clone(),
            port: config.port,
            _ssl: config.ssl,
            credentials,
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
                "From" => output.from = value.to_string(),
                "Received" => {
                    output.date = value.split_once(";").unwrap().1.trim().to_string();
                },
                _ => (),
            }
        }

        output.body = body.to_string();
        Ok(output)
    }
}

impl Backend for GreenmailBackend {
    fn needs_oauth(&self) -> bool {
        false 
    }

    fn do_command(&self, cmd: Command) -> Result<CommandResult, Error> {
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
            }
            Command::SendEmail { to: _to, subject: _subject, body: _body } => {
                // TODO: Implement email sending
                Err(Error::Unimplemented {
                    backend: "greenmail".to_string(),
                    feature: "send_email".to_string(),
                })
            }
        }
    }
}