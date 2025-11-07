use super::{Backend, Error};
use crate::types::Command;
use crate::config::BackendConfig;
use crate::types::{CommandResult, EmailMessage, MimeType};
use google_gmail1::{Gmail, hyper_rustls, hyper_util, yup_oauth2, api::Message};
use yup_oauth2::{InstalledFlowAuthenticator, InstalledFlowReturnMethod};
use async_trait::async_trait;
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

    async fn fetch_inbox_emails(&self, count: usize) -> Result<Vec<EmailMessage>, Error> {
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
            let message_response = self.hub.as_ref().unwrap()
                .users()
                .messages_get("me", message_id)
                .format("full")
                .doit()
                .await
                .map_err(|e| Error::Connection(format!("Failed to fetch message {}: {}", message_id, e)))?;
                        
            let payload: google_gmail1::api::MessagePart = message_response.1.payload.unwrap();
            let headers: Vec<google_gmail1::api::MessagePartHeader> = payload.headers.unwrap();

            // Helper function to extract header value by name
            // this is probably not maintainable; but it's cool!
            let get_header = |name: &str| -> String {
                headers.iter()
                    .find(|h| h.name.as_ref().map_or(false, |n| n == name))
                    .and_then(|h| h.value.as_ref())
                    .cloned()
                    .unwrap_or_default()
            };

            // Extract body and mime type from parts
            let (body, mime_type) = if let Some(parts) = &payload.parts {
                let mut body = String::new();
                let mut mime_type = Default::default();
                
                for part in parts {
                    if let Some(text) = part.body.as_ref()
                        .and_then(|b| b.data.as_ref())
                        .and_then(|data| std::str::from_utf8(data).ok())
                    {
                        body.push_str(text);
                    }
                    
                    if let Some(part_mime) = &part.mime_type {
                        if part_mime.contains("html") {
                            mime_type = MimeType::TextHtml;
                        }
                    }
                }
                
                (body, mime_type)
            } else {
                // fallback
                let body = payload.body.as_ref()
                    .and_then(|b| b.data.as_ref())
                    .and_then(|data| std::str::from_utf8(data).ok())
                    .unwrap_or("")
                    .to_string();
                (body, MimeType::TextPlain)
            };

            emails.push(EmailMessage { 
                id: message_id.to_string(), 
                subject: get_header("Subject"),
                from: get_header("From"),
                to: get_header("To"),
                date: get_header("Date"),
                body,
                mime_type,
            });       
        }
        Ok(emails)
    }
}

#[async_trait]
impl Backend for GmailBackend {
    fn needs_oauth(&self) -> bool {
        true
    }

    async fn authenticate(&mut self) -> Result<(), Error> {
        let secret_file = self.oauth2_client_secret_file.as_ref()
            .ok_or_else(|| Error::Config(
                "No OAuth2 client secret file configured for Gmail backend".to_string()
            ))?;

        let secret = yup_oauth2::read_application_secret(secret_file)
            .await
            .map_err(|e| Error::Config(format!("Failed to read OAuth2 secret file: {}", e)))?;

        // Set up the OAuth2 authenticator with installed flow (opens browser)
        // TODO: use a better way to get the scopes
        // Should be defined in the config file maybe?
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

    async fn do_command(&self, cmd: Command) -> Result<CommandResult, Error> {        
        match cmd {
            Command::FetchInbox { count } => {
                let emails = self.fetch_inbox_emails(count).await.unwrap();
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

