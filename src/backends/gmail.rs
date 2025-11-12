use super::{Backend, Error};
use crate::config::BackendConfig;
use crate::types;
use crate::types::{CommandResult, EmailMessage, MimeType, Command};
use std::io::Write;
use google_gmail1::api::{MessagePart, MessagePartBody, MessagePartHeader};
use google_gmail1::{Gmail, hyper_rustls, hyper_util, yup_oauth2, api::Message};
use tempfile::NamedTempFile;
use yup_oauth2::{InstalledFlowAuthenticator, InstalledFlowReturnMethod};
use async_trait::async_trait;
use hyper_rustls::HttpsConnector;
use futures::future;

type GmailHub = Gmail<HttpsConnector<hyper_util::client::legacy::connect::HttpConnector>>;
pub struct GmailBackend {
    oauth2_client_secret_file: Option<String>,
    hub: Option<Box<GmailHub>>,
    filter_labels: Option<Vec<String>>,
    editor: String,
}

impl GmailBackend {
    pub fn new(config: &BackendConfig, editor: String) -> Self {
        Self {
            oauth2_client_secret_file: config.oauth2_client_secret_file.clone(),
            hub: None,
            filter_labels: config.filter_labels.clone(),
            editor,
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
        
        let futures = messages.into_iter()
            .filter_map(|message| {
                message.id.map(|message_id| {
                    async move {
                        let message_response = self.hub.as_ref().unwrap()
                            .users()
                            .messages_get("me", message_id.as_str())
                            .format("full")
                            .doit()
                            .await
                            .map_err(|e| Error::Connection(format!("Failed to fetch message_id ({}): {}", message_id, e)));
                        
                        // Return the result (either Ok or Err) along with the message_id
                        message_response.map(|resp| (message_id, resp.1))
                    }
                })
            })
            .collect::<Vec<_>>();

        let message_results = future::join_all(futures).await;
        
        // We might be able to use an array here instead of a vector here in the future.
        let mut emails = Vec::new();
        for result in message_results {
            match result {
                Ok((message_id, message)) => {
                    let payload: google_gmail1::api::MessagePart = message.payload.unwrap();
                    let headers = payload.headers.unwrap();

                    // Helper function to extract header value by name
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
                        id: message_id, 
                        subject: get_header("Subject"),
                        from: get_header("From"),
                        to: get_header("To"),
                        date: get_header("Date"),
                        body,
                        mime_type,
                    });
                }
                Err(e) => eprintln!("Failed to fetch message: {}", e),
            }
        }
        Ok(emails)
    }

    async fn list_labels(&self) -> Result<Vec<types::Label>, Error> {
        let result = self.hub.as_ref().unwrap()
            .users()
            .labels_list("me")
            .doit()
            .await
            .map_err(|e| Error::Connection(format!("Failed to fetch labels: {}", e)))?;

        let partial_labels: Vec<google_gmail1::api::Label> = result.1.labels.unwrap();
        let futures = partial_labels.into_iter()
            .filter_map(|partial_label| {
                partial_label.id.map(|label_id| {
                    // Create an async task for each label_get request.
                    async move {
                        let result = self.hub.as_ref().unwrap()
                            .users()
                            .labels_get("me", &label_id)
                            .doit()
                            .await
                            .map_err(|e| Error::Connection(format!("Failed to fetch label {}: {}", label_id, e)));
                        result.unwrap().1
                    }
                })
            })
            .collect::<Vec<_>>();
        let detailed_labels: Vec<google_gmail1::api::Label> = future::join_all(futures).await;
        let output = detailed_labels.iter().map(|label| types::Label {
            color: label.color.clone(),
            id: label.id.clone(),
            messages_total: label.messages_total.map(|x| x as usize),
            messages_unread: label.messages_unread.map(|x| x as usize),
            name: label.name.clone(),
        }).collect::<Vec<types::Label>>();
        
        Ok(output)
    }

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
            "https://www.googleapis.com/auth/gmail.addons.current.message.readonly",
            "https://www.googleapis.com/auth/gmail.send",
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
            },
            Command::ListLabels => {
                let mut labels = self.list_labels().await.unwrap();
                if let Some(filter_labels) = self.filter_labels.as_ref() {
                    labels = labels.into_iter()
                        .filter(|label| !filter_labels
                            .contains(&label.name.as_ref().unwrap().to_string()))
                        .collect();
                }
                Ok(CommandResult::Labels(labels))
            },
            Command::SendEmail {to,subject, body } => {
                
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

                let email_content = format!(
                    "To: {}\r\nSubject: {}\r\nContent-Type: text/plain; charset=UTF-8\r\n\r\n{}",
                    draft.to, draft.subject, draft.body
                );

                let message = Message::default();

                let result = self.hub.as_ref().unwrap()
                    .users()
                    .messages_send(message, "me")
                    .upload(std::io::Cursor::new(email_content.as_bytes().to_vec()), "message/rfc822".parse().unwrap())
                    .await
                    .map_err(|e| Error::Connection(format!("Failed to send email: {}", e)))?;

                println!("Email sent successfully! Message ID: {:?}", result.1.id);

                Ok(CommandResult::Empty)
            }
        }
    }
}

