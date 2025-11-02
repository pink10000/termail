use super::{Backend, Command, Error};
use crate::config::BackendConfig;
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

    async fn fetch_inbox_top_async(&self) -> Result<Option<String>, Error> {
        println!("Fetching inbox top");
        let result = self.hub.as_ref().unwrap()
            .users()
            .messages_list("me")
            .max_results(1)
            .doit()
            .await
            .map_err(|e| Error::Connection(format!("Failed to fetch inbox top: {}", e)))?;
        let messages: Vec<Message> = result.1.messages.unwrap();

        // I think theres a better way to write this?
        if messages.is_empty() {
            println!("No Messages Found");
            return Ok(None)
        }
        
        let message_id = messages.first().unwrap().id.as_ref().unwrap();
        println!("Message ID: {}", message_id);

        let top_message_response = self.hub.as_ref().unwrap()
            .users()
            .messages_get("me", message_id)
            .format("full")
            .doit()
            .await
            .map_err(|e| Error::Connection(format!("Failed to fetch inbox top: {}", e)))?;
        
        let top_message: Message = top_message_response.1;

        let mut output = String::new();
                
        if let Some(payload) = &top_message.payload {
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

                output.push_str(&format!("Subject: {}\n", subject));
                output.push_str(&format!("From: {}\n", from));
                output.push_str(&format!("To: {}\n", to));
                output.push_str(&format!("Date: {}\n", date));

                // need some kind of way to handle different mime types for email 
                // not all emails are plain text

                if let Some(parts) = &payload.parts {
                    for part in parts {
                        if let Some(body) = &part.body {
                            let body_data = body.data.as_ref().unwrap();
                            output.push_str(std::str::from_utf8(&body_data).unwrap());
                        }
                    }   
                }   
            }
        }

        Ok(Some(output))
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

    fn check_command_support(&self, cmd: &Command) -> Result<bool, Error> {
        match cmd {
            Command::FetchInbox { count } => Ok(*count > 0),
            _ => Ok(false),
        }
    }

    fn do_command(&self, cmd: Command) -> Result<Option<String>, Error> {
        match cmd {
            Command::FetchInbox { count } => {
                if count == 1 {
                    self.fetch_inbox_top()
                } else {
                    Err(Error::Unimplemented {
                        backend: "gmail".to_string(),
                        feature: "fetch_inbox_top_n".to_string(),
                    })
                }
            }
        }
    }

    fn fetch_inbox_top(&self) -> Result<Option<String>, Error> {
        let rt = tokio::runtime::Runtime::new()
            .map_err(|e| Error::Config(format!("Failed to create tokio runtime: {}", e)))?;
        
        rt.block_on(self.fetch_inbox_top_async())
    }

    fn fetch_inbox_top_n(&self, _n: usize) -> Result<Vec<String>, Error> {
        Err(Error::Unimplemented {
            backend: "gmail".to_string(),
            feature: "fetch_inbox_top_n".to_string(),
        })
    }
}

