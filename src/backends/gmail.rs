use super::{Backend, Error};
use crate::types::Command;
use crate::config::BackendConfig;
use crate::types::{CommandResult, EmailMessage, MimeType};
use google_gmail1::{Gmail, hyper_rustls, hyper_util, yup_oauth2, api::Message};
use yup_oauth2::{InstalledFlowAuthenticator, InstalledFlowReturnMethod};

use hyper_rustls::HttpsConnector;

type GmailHub = Gmail<HttpsConnector<hyper_util::client::legacy::connect::HttpConnector>>;
pub struct GmailBackend {
    oauth2_client_secret_file: Option<String>,
    hub: Option<Box<GmailHub>>,
}

impl GmailBackend {
    pub fn new(config: &BackendConfig) -> Self {
        Self {
            oauth2_client_secret_file: config.oauth2_client_secret_file.clone(),
            hub: None
        }
    }

    async fn authenticate_async(&mut self) -> Result<(), Error> {
        let secret_file = self.oauth2_client_secret_file.as_ref()
            .ok_or_else(|| Error::Config(
                "No OAuth2 client secret file configured for Gmail backend".to_string()
            ))?;

        let secret = yup_oauth2::read_application_secret(secret_file)
            .await
            .map_err(|e| Error::Config(format!("Failed to read OAuth2 secret file: {}", e)))?;

        // Set up the OAuth2 authenticator with installed flow (opens browser)
        let scopes = &[
            "https://www.googleapis.com/auth/gmail.readonly",
            "https://www.googleapis.com/auth/gmail.addons.current.message.readonly"
        ];
        
        let auth = InstalledFlowAuthenticator::builder(secret,InstalledFlowReturnMethod::HTTPRedirect)
            .persist_tokens_to_disk("tokencache.json")
            .build()
            .await
            .map_err(|e| Error::Config(format!("Failed to build authenticator: {}", e)))?;
        auth.token(scopes).await.map_err(|e| Error::Config(format!("Failed to get token: {}", e)))?;
        
        let https = hyper_rustls::HttpsConnectorBuilder::new()
            .with_native_roots()
            .map_err(|e| Error::Config(format!("Failed to load native roots: {}", e)))?
            .https_or_http()
            .enable_http1()
            .build();

        let client = hyper_util::client::legacy::Client::builder(
            hyper_util::rt::TokioExecutor::new()
        ).build(https);

        self.hub = Some(Box::new(Gmail::new(client, auth)));
        Ok(())
    }

    async fn fetch_inbox_emails_async(&self, count: usize) -> Result<Vec<EmailMessage>, Error> {
        let result = self.hub.as_ref().unwrap()
            .users()
            .messages_list("me")
            .max_results(count as u32)
            .doit()
            .await
            .map_err(|e| Error::Connection(format!("Failed to fetch inbox: {}", e)))?;
        
        let messages: Vec<Message> = result.1.messages.unwrap_or_default();

        if messages.is_empty() {
            println!("No Messages Found");
            return Ok(Vec::new())
        }
        
        let mut emails = Vec::new();
        
        for message in messages {
            let message_id = message.id.as_ref().unwrap();
            println!("Fetching message: {}", message_id);

            let message_response = self.hub.as_ref().unwrap()
                .users()
                .messages_get("me", message_id)
                .format("full")
                .doit()
                .await
                .map_err(|e| Error::Connection(format!("Failed to fetch message {}: {}", message_id, e)))?;
            
            let full_message: Message = message_response.1;
            
            if let Some(payload) = &full_message.payload {
                if let Some(headers) = &payload.headers {
                    let mut subject = String::new();
                    let mut from = String::new();
                    let mut to = String::new();
                    let mut date = String::new();
                    
                    for header in headers {
                        let name = header.name.as_ref().unwrap();
                        let value = header.value.as_ref().unwrap();

                        match name.as_str() {
                            "Subject" => subject = value.to_string(),
                            "From" => from = value.to_string(),
                            "To" => to = value.to_string(),
                            "Date" => date = value.to_string(),
                            _ => (),
                        }
                    }

                    let mut body = String::new();
                    let mut mime_type: MimeType = Default::default();
                    
                    // Extract body from parts
                    if let Some(parts) = &payload.parts {
                        for part in parts {
                            if let Some(part_body) = &part.body {
                                if let Some(data) = &part_body.data {
                                    if let Ok(text) = std::str::from_utf8(data) {
                                        body.push_str(text);
                                    }
                                }
                            }
                            
                            // Determine mime type from part
                            if let Some(part_mime_type) = &part.mime_type {
                                if part_mime_type.contains("html") {
                                    mime_type = MimeType::TextHtml;
                                }
                            }
                        }
                    }
                    
                    let email = EmailMessage {
                        id: message_id.clone(),
                        subject,
                        from,
                        to,
                        date,
                        body,
                        mime_type,
                    };
                    
                    emails.push(email);
                }
            }
        }

        Ok(emails)
    }
}

impl Backend for GmailBackend {
    fn needs_oauth(&self) -> bool {
        true
    }

    fn authenticate(&mut self) -> Result<(), Error> {
        let rt = tokio::runtime::Runtime::new()
            .map_err(|e| Error::Config(format!("Failed to create tokio runtime: {}", e)))?;
        
        rt.block_on(self.authenticate_async())
    }

    fn do_command(&self, cmd: Command) -> Result<CommandResult, Error> {
        let rt = tokio::runtime::Runtime::new()
            .map_err(|e| Error::Config(format!("Failed to create tokio runtime: {}", e)))?;
        
        match cmd {
            Command::FetchInbox { count } => {
                let emails = rt.block_on(self.fetch_inbox_emails_async(count))?;
                if emails.is_empty() {
                    Ok(CommandResult::Empty)
                } else if count == 1 {
                    Ok(CommandResult::Email(emails.into_iter().next().unwrap()))
                } else {
                    Ok(CommandResult::Emails(emails))
                }
            }
            Command::SendEmail { to: _to, subject: _subject, body: _body } => {
                // TODO: Implement email sending via Gmail API
                Err(Error::Unimplemented {
                    backend: "gmail".to_string(),
                    feature: "send_email".to_string(),
                })
            }
        }
    }
}

