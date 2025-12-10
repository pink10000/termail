use super::{Backend, Error};
use crate::config::BackendConfig;
use crate::plugins::events::Hook;
use crate::cli::command::{Command, CommandResult};
use crate::core::{email::{EmailMessage, EmailSender, MimeType}, label::Label, editor::Editor};
use std::collections::{HashMap, HashSet};
use google_gmail1::{Gmail, hyper_rustls, hyper_util, yup_oauth2, api::Message};
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
    maildir_manager: MaildirManager,
}

impl GmailBackend {
    pub fn new(config: &BackendConfig, editor: String) -> Self {
        Self {
            oauth2_client_secret_file: config.oauth2_client_secret_file.clone(),
            hub: None,
            filter_labels: config.filter_labels.clone(),
            editor,
            maildir_manager: MaildirManager::new(config.maildir_path.clone()).unwrap_or_else(|e| {
                eprintln!("Failed to create maildir manager: {}", e);
                std::process::exit(1);
            }),
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
                        email_attachments: Vec::new(),
                    });
                }
                Err(e) => eprintln!("Failed to fetch message: {}", e),
            }
        }
        Ok(emails)
    }

    /// Views emails from the local maildir (reads from synced emails).
    /// 
    /// Emails are read from the maildir directory where they were synced from Gmail.
    async fn view_mailbox(&self, count: usize) -> Result<Vec<EmailMessage>, Error> {
        // Read emails from maildir
        let emails = self.maildir_manager.list_emails(count)?;
        
        if emails.is_empty() {
            return Ok(Vec::new());
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

    async fn incremental_sync(&self, last_sync_id: u64) -> Result<(), Error> {

        let result = self.hub.as_ref().unwrap()
            .users()
            .history_list("me")
            .start_history_id(last_sync_id)
            .doit()
            .await;


        if let Err(e) = result {
            if e.to_string().contains("404") {
                // means that not enough history is available, so we need to do a smart sync
                return self.smart_sync().await;
            } else {
                return Err(Error::Connection(format!("Failed to fetch history: {}", e)));
            }
        }

        let curr_history_id = result.as_ref().unwrap().1.history_id.unwrap();

        let sync_state_path = self.maildir_manager.get_sync_state_path();
        let mut sync_state = MaildirManager::load_sync_state_from_file(&sync_state_path)?;

        // iterate thru all the history records starting at last_sync_id
        // make sure to go to all pages

        let history_records = self.hub.as_ref().unwrap()
            .users()
            .history_list("me")
            .start_history_id(last_sync_id)
            .doit()
            .await
            .map_err(|e| Error::Connection(format!("Failed to fetch history: {}", e)))?;

        if history_records.1.history.is_none() {
            return Ok(());
        }

        // create a map of message id to action that was taken and we overwrite if there are multiple actions for the same message since records are in chronological order
        let mut message_id_to_action: HashMap<String, String> = HashMap::new();


        for history_record in history_records.1.history.unwrap() {
            if history_record.labels_added.is_some() {

                // if record was added Trash label then we delete from maildir
                // if record was added Unread label then we move to new in maildir
                for label in history_record.labels_added.unwrap() {

                    let labels = label.label_ids.unwrap();

                    if labels.contains(&"UNREAD".to_string()) {

                        let gmail_id = label.message.unwrap().id.unwrap();
                        message_id_to_action.insert(gmail_id.to_string(), "move_to_new".to_string());
                    } else if labels.contains(&"TRASH".to_string()) {
                        
                        let gmail_id = label.message.unwrap().id.unwrap();
                        message_id_to_action.insert(gmail_id.to_string(), "delete".to_string());
                    }
                }
            } else if history_record.labels_removed.is_some() {

                // if record was removed Unread label then we move to cur in maildir
                for label in history_record.labels_removed.unwrap() {
                    let labels = label.label_ids.unwrap();
                    if labels.contains(&"UNREAD".to_string()) {
                        
                        let gmail_id = label.message.unwrap().id.unwrap();
                        message_id_to_action.insert(gmail_id.to_string(), "move_to_cur".to_string());
                    }
                }

                

            } else if history_record.messages_added.is_some() {
                // if record has message added then we need to put in maildir dir based on label
                for message in history_record.messages_added.unwrap() {
                    let gmail_id = message.message.unwrap().id.unwrap();
                    message_id_to_action.insert(gmail_id.to_string(), "move_to_new".to_string());
                }
            } 

        
        }

        // do the right thing based on the action
        for (message_id, action) in message_id_to_action.iter() {
            if action == "delete" {
                // get maildir id from map
                let maildir_id = sync_state.message_id_to_maildir_id.get(message_id).unwrap();
                // delete message from maildir using maildir_id
                self.maildir_manager.delete_message(maildir_id.clone()).unwrap();
                // update sync state by removing message_id from map
                sync_state.message_id_to_maildir_id.remove(message_id);
            } else if action == "move_to_new" {
                // TODO: fix this syncing issue where the image doesn't get pulled from cloud
                // get maildir id from map
                if let Some(maildir_id) = sync_state.message_id_to_maildir_id.get(message_id) {
                    self.maildir_manager.maildir_move_new_to_cur(&maildir_id).unwrap();
                } else {
                    eprintln!("Message id not found in sync state: {}", message_id);
                }
            } else if action == "move_to_cur" {
                // get maildir id from map
                let maildir_id = sync_state.message_id_to_maildir_id.get(message_id).unwrap();
                // move message to cur in maildir
                let new_maildir_id = self.maildir_manager.maildir_move_cur_to_new(&maildir_id).unwrap();
                // update sync state by removing message_id from map and adding new maildir_id
                sync_state.message_id_to_maildir_id.remove(message_id);
                sync_state.message_id_to_maildir_id.insert(message_id.clone(), new_maildir_id);
            }
        }
            
        // update last sync id
        sync_state.last_sync_id = curr_history_id;
        MaildirManager::save_sync_state_to_file(&sync_state_path, &sync_state)?;


        Ok(())
    }

    async fn smart_sync(&self) -> Result<(), Error> {

        // Get all current gmail message ids
        let mut all_gmail_ids: HashSet<String> = HashSet::new();
        let mut page_token: Option<String> = None;
        
        loop {
            // build request
            let mut request = self.hub.as_ref().unwrap()
                .users()
                .messages_list("me")
                .add_label_ids("INBOX")
                .max_results(500);
            
            // add page token if it exists
            if let Some(token) = page_token {
                request = request.page_token(&token);
            }
            
            // send request
            let result = request.doit().await
                .map_err(|e| Error::Connection(format!("Failed to list messages: {}", e)))?;
            
            // add messages to set
            if let Some(messages) = result.1.messages {
                for msg in messages {
                    match msg.id {
                        Some(id) => all_gmail_ids.insert(id),
                        None => false,
                    };
                }
            }
            
            // update page token and break if no more pages
            page_token = result.1.next_page_token;
            if page_token.is_none() {
                break;
            }
        }
        // Get all current maildir message ids
        let sync_state_path = self.maildir_manager.get_sync_state_path();
        let sync_state = MaildirManager::load_sync_state_from_file(&sync_state_path)?;
        let local_ids: HashSet<String> = sync_state.message_id_to_maildir_id.keys().cloned().collect();
    
        // Find differences
        let to_add_ids = &all_gmail_ids - &local_ids;
        let to_delete_ids = &local_ids - &all_gmail_ids;
        let to_update_ids = &all_gmail_ids & &local_ids;

        // println!("to_add_ids size: {:?}", to_add_ids.len());
        // println!("to_delete_ids size: {:?}", to_delete_ids.len());
        // println!("to_update_ids size: {:?}", to_update_ids.len());

        // Downlaod new messages
        let mut sync_updates_to_add: Vec<(String, String)> = Vec::new();

        for id in to_add_ids {
            let message_response = self.hub.as_ref().unwrap()
                .users()
                .messages_get("me", id.as_str())
                .format("raw")
                .doit()
                .await
                .map_err(|e| Error::Connection(format!("Failed to fetch message: {}", e)));
            
            match message_response {
                Ok(message) => {
                    
                    // Save message to correct maildir subdirectory
                    let maildir_id: String;
                        if message.1.label_ids.clone().unwrap_or_default().contains(&"UNREAD".to_string()) {
                            maildir_id = self.maildir_manager.save_message(&message.1, "new".to_string()).unwrap();
                        } else {
                            maildir_id = self.maildir_manager.save_message(&message.1, "cur".to_string()).unwrap();
                        } 
                    
                    // Add to sync updates to add
                    sync_updates_to_add.push((id.clone(), maildir_id));
                    
                }
                Err(e) => {
                    return Err(Error::Connection(format!("Failed to fetch message: {}", e)));
                }
            }
        }
        // update sync state with new messages
        self.update_sync_state(&sync_updates_to_add).unwrap();
        
        // Take care of deleted messages
        // maildir deletes messages based on maildir_id so we need to get the maildir_id from the sync state
        let mut sync_state = MaildirManager::load_sync_state_from_file(&sync_state_path)?;
        for gmail_id in to_delete_ids {
            let maildir_id = sync_state.message_id_to_maildir_id.get(&gmail_id).unwrap();
            self.maildir_manager.delete_message(maildir_id.clone()).unwrap();
            sync_state.message_id_to_maildir_id.remove(&gmail_id);
        }
        MaildirManager::save_sync_state_to_file(&sync_state_path, &sync_state)?;
        
        // Update existing messagse if needed
        let mut sync_state = MaildirManager::load_sync_state_from_file(&sync_state_path)?;

        for gmail_id in to_update_ids {
            // if message was updated (read or unread) then we need to update the message in the maildir
            let metadata_response = self.hub.as_ref().unwrap()
                .users()
                .messages_get("me", gmail_id.as_str())
                .format("metadata")
                .doit()
                .await
                .map_err(|e| Error::Connection(format!("Failed to fetch message: {}", e)));

            // get maildir id form gmail id
            let maildir_id = sync_state.message_id_to_maildir_id.get(&gmail_id).unwrap();

            // figure out if message is read or unread
            let is_read = !metadata_response.unwrap().1.label_ids.clone().unwrap_or_default().contains(&"UNREAD".to_string());

            // need to see which directory our message is in and if different from cloud label then we need to move it
            let maildir_directory = self.maildir_manager.get_message_directory(&maildir_id).unwrap();
            
            if !is_read && maildir_directory == "cur" {
                // if not read in cloud but read locally then move message to new in maildir
                let new_maildir_id = self.maildir_manager.maildir_move_cur_to_new(&maildir_id).unwrap();
                sync_state.message_id_to_maildir_id.remove(&gmail_id);
                sync_state.message_id_to_maildir_id.insert(gmail_id.clone(), new_maildir_id);

            } else if is_read && maildir_directory == "new" {
                // if read in cloud but in new then move message to cur in maildir
                self.maildir_manager.maildir_move_new_to_cur(&maildir_id).unwrap();
            }
        }

        // Update last_sync_id and sync sate
        let profile_result = self.hub.as_ref().unwrap()
            .users()
            .get_profile("me")
            .doit()
            .await
            .map_err(|e| Error::Connection(format!("Failed to get profile: {}", e)))?;
        let last_sync_id = profile_result.1.history_id.unwrap();
        sync_state.last_sync_id = last_sync_id;
        MaildirManager::save_sync_state_to_file(&sync_state_path, &sync_state)?;

        Ok(())
    }

    async fn full_sync(&self) -> Result<(), Error> {
        // TODO: can later get progress to show easily later
        let mut page_token: Option<String> = None;

        let mut updates = Vec::new();

        loop {
            // build request
            let mut request = self.hub.as_ref().unwrap()
                .users()
                .messages_list("me")
                .add_label_ids("INBOX")
                .max_results(500);
            
            // add page token if it exists
            if let Some(token) = page_token {
                request = request.page_token(&token);
            }
            
            // send request
            let result = request.doit().await
                .map_err(|e| Error::Connection(format!("Failed to fetch messages: {}", e)))?;
            
            // update page token
            page_token = result.1.next_page_token;
            
            let messages: Vec<Message> = result.1.messages.unwrap_or_default();

            // iterate through messages
            for message in messages {
                
                // fetch message
                let message_response = self.hub.as_ref().unwrap()
                    .users()
                    .messages_get("me", message.id.unwrap().as_str())
                    .format("raw")
                    .doit()
                    .await
                    .map_err(|e| Error::Connection(format!("Failed to fetch message: {}", e)));

                match message_response {
                    Ok(message) => {

                        // Save message to correct maildir subdirectory
                        // message will either have label READ or UNREAD
                        let maildir_id: String;
                        if message.1.label_ids.clone().unwrap_or_default().contains(&"UNREAD".to_string()) {
                            maildir_id = self.maildir_manager.save_message(&message.1, "new".to_string()).unwrap();
                        } else {
                            maildir_id = self.maildir_manager.save_message(&message.1, "cur".to_string()).unwrap();
                        } 
                        
                        updates.push((message.1.id.unwrap().clone(), maildir_id));

                    }
                    Err(e) => {
                        return Err(Error::Connection(format!("Failed to fetch message: {}", e)));
                    }
                }

            }
            // update sync state
            self.update_sync_state(&updates).unwrap();

            // break if no more pages
            if page_token.is_none() {
                break;
            }
        }

        Ok(())
    }

    fn update_sync_state(&self, updates: &[(String, String)]) -> Result<(), Error> {

        let sync_state_path = self.maildir_manager.get_sync_state_path();
        
        // load sync state from file
        let mut sync_state = MaildirManager::load_sync_state_from_file(&sync_state_path)?;
        // update sync state
        for (message_id, maildir_id) in updates {
            sync_state.message_id_to_maildir_id.insert(message_id.clone(), maildir_id.clone());
        }

        // save sync state to file
        MaildirManager::save_sync_state_to_file(&sync_state_path, &sync_state)?;

        Ok(())
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
            // TODO: deprecate fetch inbox for gmail backend
            // - Breaks the sync model
            // - Risks rate limiting
            // - Doesn't persist emails
            // - Duplicates functionality
            // Command::FetchInbox { count: _ } => {
            //     return Err(Error::Other("FetchInbox is deprecated for Gmail backend. Use 'sync-from-cloud' to download emails to maildir, then 'view-mailbox' to view them.".to_string()));
            // },
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
                let mut draft = EmailMessage {
                    to: to.unwrap_or_default(),
                    subject: subject.unwrap_or_default(),
                    body: body.unwrap_or_default(),
                    ..EmailMessage::new()
                };

                if draft.is_partially_empty() {
                    let result = Editor::open(&self.editor, draft)?;
                    draft = result;
                }

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

                let email = draft.to_lettre_email()?;
                let raw_bytes = email.formatted();

                let _result = self.hub.as_ref().unwrap()
                    .users()
                    .messages_send(google_gmail1::api::Message::default(), "me") // See documentation of this method for Gmail's API docs.
                    .upload(
                        std::io::Cursor::new(raw_bytes), 
                        "message/rfc822".parse().unwrap()
                    )
                    .await
                    .map_err(|e| Error::Connection(format!("Failed to send email: {}", e)))?;

                // println!("Email sent successfully! Message ID: {:?}", result.1.id);

                Ok(CommandResult::Empty)
            }
            Command::SyncFromCloud => {
                
                let last_sync_id = self.maildir_manager.get_last_sync_id();
                println!("Last sync id: {:?}", last_sync_id);

                if last_sync_id == 0 && !self.maildir_manager.has_synced_emails()? {
                    println!("Last sync id is 0 and no emails have been synced yet, doing full sync");
                    self.full_sync().await?;
                    println!("Full sync completed");
                } else {
                    println!("Incrementing sync from last sync id: {:?}", last_sync_id);
                    self.incremental_sync(last_sync_id).await?;                    
                }

                Ok(CommandResult::Empty)
            },
            Command::ViewMailbox { count } => {
                let emails = self.view_mailbox(count).await.unwrap();
                // filter emails to the ones that only have image attachments
                let filtered_emails: Vec<EmailMessage> = emails.into_iter()
                    .filter(|email| email.get_image_attachments().is_empty())
                    .collect();
                if filtered_emails.is_empty() {
                    Ok(CommandResult::Empty)
                } else if count == 1 {
                    Ok(CommandResult::Email(filtered_emails.into_iter().next().unwrap()))
                } else {
                    Ok(CommandResult::Emails(filtered_emails))
                }
            },
            Command::Null => Ok(CommandResult::Empty)
        }
    }

    /// Defines which commands require authentication to the Gmail service.
    fn requires_authentication(&self, cmd: &Command) -> Option<bool> {
        match cmd {
            Command::SyncFromCloud => Some(true),
            Command::ViewMailbox { count: _ } => Some(false),
            Command::SendEmail { to: _, subject: _, body: _ } => Some(true),
            // Command::FetchInbox { count: _ } => None, // TODO: deprecate fetch inbox for gmail backend
            Command::ListLabels => Some(true),
            Command::Null => Some(false),
            _ => None
        }
    }
}

