use super::{Backend, Error};
use crate::config::BackendConfig;
use crate::plugins::events::Hook;
use crate::types::{Command, CommandResult, EmailMessage, EmailSender, Label, MimeType};
use std::io::Write;
use google_gmail1::{Gmail, hyper_rustls, hyper_util, yup_oauth2, api::Message};
use tempfile::NamedTempFile;
use yup_oauth2::{InstalledFlowAuthenticator, InstalledFlowReturnMethod};
use async_trait::async_trait;
use hyper_rustls::HttpsConnector;
use futures::future;
use crate::plugins::plugins::{PluginManager};
use crate::maildir::MaildirManager;

type GmailHub = Gmail<HttpsConnector<hyper_util::client::legacy::connect::HttpConnector>>;
pub struct GmailBackend {
    oauth2_client_secret_file: Option<String>,
    hub: Option<Box<GmailHub>>,
    filter_labels: Option<Vec<String>>,
    editor: String,
    maildir_manager: Option<MaildirManager>,
}

impl GmailBackend {
    pub fn new(config: &BackendConfig, editor: String) -> Self {
        Self {
            oauth2_client_secret_file: config.oauth2_client_secret_file.clone(),
            hub: None,
            filter_labels: config.filter_labels.clone(),
            editor,
            maildir_manager: Some(MaildirManager::new(config.maildir_path.clone()).unwrap()),
        }
    }

    /// Fetches the inbox emails from the Gmail backend.
    /// 
    /// There is a chance that you will be rate limited by Gmail if you fetch too 
    /// many emails at once. 
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
        if message_results.iter().any(|result| result.is_err()) {
            return Err(Error::Connection("Rate limited by Gmail".to_string()));
        }
        
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
                        from: EmailSender::from(get_header("From")),
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

    async fn list_labels(&self) -> Result<Vec<Label>, Error> {
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
        let output = detailed_labels.iter().map(|label| Label {
            color: label.color.clone(),
            id: label.id.clone(),
            messages_total: label.messages_total.map(|x| x as usize),
            messages_unread: label.messages_unread.map(|x| x as usize),
            name: label.name.clone(),
        }).collect::<Vec<Label>>();
        
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

    async fn do_command(&self, cmd: Command, plugin_manager: Option<&mut PluginManager>) -> Result<CommandResult, Error> {        
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

                let mut draft = if draft.to.is_empty() || draft.subject.is_empty() || draft.body.is_empty() {
                    Self::edit_email_with_prefill(&self.editor, draft)?
                } else {
                    draft
                };

                if draft.to.is_empty() {
                    return Err(Error::InvalidInput("To field cannot be empty".to_string()));
                }

                // Plugin hook-point: Hook::BeforeSend
                if let Some(plugin_manager) = plugin_manager {
                    let updated_body = plugin_manager.dispatch(
                        Hook::BeforeSend.to_wit_event(draft.body.clone())
                    ).await?;
                    draft.body = updated_body;
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
            Command::SyncFromCloud => {

                println!("Sync From Cloud Gmail called");
                
                let last_sync_id = self.maildir_manager.as_ref().unwrap().get_last_sync_id();
                println!("last sync id: {:?}", last_sync_id);

                if last_sync_id == 0 {
                    println!("FULL SYNC HAPPENING");
                    // full_sync()
                    
                } else {
                    
                    let result = self.hub.as_ref().unwrap()
                        .users()
                        .history_list("me")
                        .start_history_id(last_sync_id)
                        .doit()
                        .await;

                    match result {
                        Ok(result) => {
                            println!("history list result: {:?}", result.1);
                            println!("INCREMENTAL SYNC HAPPENING");
                            // incremental_sync()
                        }
                        Err(e) => {
                            if e.to_string().contains("404") {
                                // smart_sync()
                                // TODO worry about this later
                                // idea is get every email in cloud mailbox but only get the minimal info
                                // compare gmail message id with maildir message id using map stored in sync_state.json
                                // if gmail message id is not in the map, then save the email to maildir
                                // if gmail message id is in the map, then compare the email content
                                // if the email content is different, then save the email to maildir
                                // if the email content is the same, then do not save the email to maildir
                                // save the map to sync_state.json
                                
                                // if gmail deleted the email from cloud, then delete the email from maildir
                                // if there are emails in maildir that are not in the cloud then delete email from maildir

                                println!("SMART SYNC HAPPENING");
                            } else {
                                return Err(Error::Connection(format!("Failed to fetch history: {}", e)));
                            }
                        }
                    }
                    
                }

                // let last_sync_id = self.maildir_manager.as_ref().unwrap().get_last_sync_id();
                // println!("last sync id: {:?}", last_sync_id);

                // let profile_result = self.hub.as_ref().unwrap()
                //     .users()
                //     .get_profile("me")
                //     .doit()
                //     .await
                //     .map_err(|e| Error::Connection(format!("Failed to get profile: {}", e)))?;
                
                // let current_history_id = profile_result.1.history_id
                //     .ok_or_else(|| Error::Connection("No historyId in profile".to_string()))?;
                
                // println!("Current history ID: {:?}", current_history_id);
                
                // let result = self.hub.as_ref().unwrap()
                //     .users()
                //     .history_list("me")
                //     .start_history_id(2902946)
                //     .doit()
                //     .await
                //     .map_err(|e| Error::Connection(format!("Failed to fetch history: {}", e)))?;

                // println!("history list result: {:?}", result.1);
// ------
                // let result = self.hub.as_ref().unwrap()
                //     .users()
                //     .messages_list("me")
                //     .max_results(1)
                //     // .page_token("03683800523264572113")
                //     .doit()
                //     .await
                //     .map_err(|e| Error::Connection(format!("Failed to fetch inbox: {}", e)))?;
                
                // let messages: Vec<Message> = result.1.messages.unwrap_or_default();
                // println!("emails: {:?}", messages);
                // println!("messages count: {:?}", messages.len());

                // let next_page_token = result.1.next_page_token;
                // println!("next page token: {:?}", next_page_token);

                // let message_id = messages.first().unwrap().id.clone().unwrap();

                // let message_response = self.hub.as_ref().unwrap()
                //     .users()
                //     .messages_get("me", message_id.as_str())
                //     .format("raw")
                //     .doit()
                //     .await
                //     .map_err(|e| Error::Connection(format!("Failed to fetch message_id ({}): {}", message_id, e)));
                        
                // let message = message_response.map(|resp| (message_id, resp.1)).unwrap();
                // println!("message: {:?}", message);


                // self.maildir_manager.as_ref().unwrap().save_message(message.1, "cur".to_string()).unwrap();
// -----------

                // let res = self.maildir_manager.as_ref().unwrap().save_message(messages.first().unwrap().clone(), "cur".to_string());
                // if res.is_err() {
                //     return Err(Error::Connection(format!("Failed to save message: {}", res.err().unwrap())));
                // } else {
                //     println!("Message saved successfully");
                // }

                // let message_id = <std::option::Option<std::string::String> as Clone>::clone(&messages.first().unwrap().id).unwrap();
                // println!("emails: {:?}", messages);

                // let message_response = self.hub.as_ref().unwrap()
                //     .users()
                //     .messages_get("me", message_id.as_str())
                //     .format("minimal")
                //     .doit()
                //     .await
                //     .map_err(|e| Error::Connection(format!("Failed to fetch message_id ({}): {}", message_id, e)));
                        
                // let message = message_response.map(|resp| (message_id, resp.1)).unwrap();
                // println!("message: {:?}", message);


                Ok(CommandResult::Empty)
            }
            Command::Null => Ok(CommandResult::Empty)
        }
    }
}

